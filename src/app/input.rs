use super::{
    App, Pane,
    prompt::{Confirm, Field, Kind, Popup, Prompt},
};
use crate::keymap::{Action, Context, Lookup, notation::Key};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use std::time::{Duration, Instant};

const DOUBLE_CLICK: Duration = Duration::from_millis(450);

fn is_ctrl_c(event: &KeyEvent) -> bool {
    event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('c')
}

impl App {
    pub fn handle_key(&mut self, event: KeyEvent) -> Result<()> {
        self.needs_draw = true;
        if self.prompt.is_some() {
            return self.prompt_key(event);
        }
        if self.popup.is_some() {
            return self.popup_key(event);
        }
        if self.terminal_mode {
            return self.sequence(Context::Terminal, event);
        }
        if self.insert {
            if is_ctrl_c(&event) {
                return self.perform(Action::NormalMode, 1);
            }
            return self.sequence(Context::Insert, event);
        }
        if is_ctrl_c(&event) {
            self.pending.clear();
            self.count = None;
            self.info("Type :q and press Enter to quit envoke");
            return Ok(());
        }
        self.normal_key(event)
    }

    fn normal_key(&mut self, event: KeyEvent) -> Result<()> {
        let key = Key::from_event(event);
        if self.pending.is_empty()
            && let Some(digit) = key.typed_char().and_then(|c| c.to_digit(10))
            && (digit != 0 || self.count.is_some())
        {
            self.count = Some(self.count.unwrap_or(0).saturating_mul(10) + digit as usize);
            return Ok(());
        }
        self.pending.push(key);
        match self.keymap.lookup(&self.contexts(), &self.pending) {
            Lookup::Fire(action) => {
                self.pending.clear();
                let count = self.count.take().unwrap_or(1);
                self.perform(action, count)
            }
            Lookup::Pending => {
                self.pending_at = Instant::now();
                Ok(())
            }
            Lookup::Miss => {
                self.pending.clear();
                self.count = None;
                Ok(())
            }
        }
    }

    // INSERT and TERMINAL modes: mapped sequences fire, everything else is typed text or goes to the server.
    fn sequence(&mut self, ctx: Context, event: KeyEvent) -> Result<()> {
        self.pending.push(Key::from_event(event));
        match self.keymap.lookup(&[ctx], &self.pending) {
            Lookup::Fire(action) => {
                self.pending.clear();
                self.perform(action, 1)
            }
            Lookup::Pending => {
                self.pending_at = Instant::now();
                Ok(())
            }
            Lookup::Miss => {
                let keys = std::mem::take(&mut self.pending);
                for key in keys {
                    self.raw_key(ctx, key);
                }
                Ok(())
            }
        }
    }

    // A key sequence timed out: run it if it is complete on its own, otherwise pass the keys through.
    pub fn flush_pending(&mut self) -> Result<()> {
        let ctx = if self.terminal_mode {
            Some(Context::Terminal)
        } else if self.insert {
            Some(Context::Insert)
        } else {
            None
        };
        let contexts = match ctx {
            Some(ctx) => vec![ctx],
            None => self.contexts().to_vec(),
        };
        let keys = std::mem::take(&mut self.pending);
        if let Some(action) = self.keymap.exact(&contexts, &keys) {
            let count = self.count.take().unwrap_or(1);
            return self.perform(action, count);
        }
        self.count = None;
        if let Some(ctx) = ctx {
            for key in keys {
                self.raw_key(ctx, key);
            }
        }
        Ok(())
    }

    fn raw_key(&mut self, ctx: Context, key: Key) {
        if ctx == Context::Terminal {
            self.server.send_key(key.event());
            return;
        }
        let buffer = &mut self.buffer;
        if let Some(c) = key.typed_char() {
            buffer.insert_str(c.encode_utf8(&mut [0; 4]));
            return;
        }
        match key.code {
            KeyCode::Enter => buffer.insert_str("\n"),
            KeyCode::Backspace => buffer.backspace(),
            KeyCode::Delete => buffer.delete_forward(),
            KeyCode::Left => buffer.col = buffer.col.saturating_sub(1),
            KeyCode::Right => buffer.col += 1,
            KeyCode::Up => buffer.row = buffer.row.saturating_sub(1),
            KeyCode::Down => buffer.row += 1,
            KeyCode::Home => buffer.col = 0,
            KeyCode::End => buffer.col = usize::MAX,
            KeyCode::Tab | KeyCode::BackTab => {
                self.leave_insert();
                let next = if key.code == KeyCode::Tab {
                    Action::FocusNext
                } else {
                    Action::FocusPrev
                };
                let _ = self.perform(next, 1);
                return;
            }
            _ => {}
        }
        self.buffer.clamp(true);
    }

    fn prompt_key(&mut self, event: KeyEvent) -> Result<()> {
        let Some(mut prompt) = self.prompt.take() else {
            return Ok(());
        };
        if let Kind::Confirm(confirm) = prompt.kind {
            let yes = matches!(event.code, KeyCode::Char('y') | KeyCode::Char('Y'));
            if !yes {
                self.info("Cancelled");
                return Ok(());
            }
            return match confirm {
                Confirm::RemoveProject => self.remove_project(),
            };
        }
        let command = prompt.kind == Kind::Command;
        match event.code {
            KeyCode::Esc => return Ok(()),
            _ if is_ctrl_c(&event) => return Ok(()),
            KeyCode::Backspace if prompt.input.text.is_empty() && command => return Ok(()),
            KeyCode::Enter => return self.submit_prompt(prompt),
            KeyCode::Tab | KeyCode::BackTab if command => {
                self.complete(&mut prompt, event.code == KeyCode::Tab)
            }
            KeyCode::Up | KeyCode::Down if command && prompt.completion.active() => {
                self.complete(&mut prompt, event.code == KeyCode::Down)
            }
            KeyCode::Up | KeyCode::Down if command => {
                self.browse_history(&mut prompt, event.code == KeyCode::Up)
            }
            _ => {
                prompt.input.key(event);
                prompt.completion.clear();
            }
        }
        self.prompt = Some(prompt);
        Ok(())
    }

    fn submit_prompt(&mut self, prompt: Prompt) -> Result<()> {
        let text = prompt.input.text.trim().to_owned();
        let result = match &prompt.kind {
            Kind::Command => {
                if !text.is_empty() {
                    self.history.retain(|h| h != &text);
                    self.history.push(text.clone());
                }
                self.message = None;
                return self.run_command(&text);
            }
            Kind::NewEnv => self.create_env(&text),
            Kind::Confirm(_) => Ok(()),
        };
        // A failed file name stays in the prompt so it can be fixed instead of retyped.
        if result.is_err() {
            self.prompt = Some(prompt);
        }
        result
    }

    fn browse_history(&mut self, prompt: &mut Prompt, older: bool) {
        if self.history.is_empty() {
            return;
        }
        let last = self.history.len() - 1;
        let next = match (prompt.history, older) {
            (None, true) => Some(last),
            (None, false) => None,
            (Some(i), true) => Some(i.saturating_sub(1)),
            (Some(i), false) if i >= last => None,
            (Some(i), false) => Some(i + 1),
        };
        prompt.history = next;
        let text = next.map(|i| self.history[i].clone()).unwrap_or_default();
        prompt.input.set(text);
    }

    fn popup_key(&mut self, event: KeyEvent) -> Result<()> {
        let key = Key::from_event(event);
        let close = matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) || is_ctrl_c(&event);
        let len = self.selected().map_or(0, |p| p.scripts.len());
        match self.popup.as_mut() {
            Some(Popup::Help { scroll }) => match key.code {
                _ if close => self.popup = None,
                KeyCode::Char('j') | KeyCode::Down => *scroll += 1,
                KeyCode::Char('k') | KeyCode::Up => *scroll = scroll.saturating_sub(1),
                KeyCode::Char('d') | KeyCode::PageDown => *scroll += 10,
                KeyCode::Char('u') | KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                KeyCode::Char('g') | KeyCode::Home => *scroll = 0,
                // The renderer clamps this to the last page.
                KeyCode::Char('G') | KeyCode::End => *scroll = usize::MAX,
                _ => {}
            },
            Some(Popup::Scripts {
                selected,
                start_after,
            }) => match key.code {
                _ if close => self.popup = None,
                KeyCode::Char('j') | KeyCode::Down => {
                    *selected = (*selected + 1).min(len.saturating_sub(1))
                }
                KeyCode::Char('k') | KeyCode::Up => *selected = selected.saturating_sub(1),
                KeyCode::Enter => {
                    let (index, start) = (*selected, *start_after);
                    self.popup = None;
                    return self.choose_script(index, start);
                }
                _ => {}
            },
            Some(Popup::Project(_)) => return self.project_form_key(event),
            None => {}
        }
        Ok(())
    }

    // Name -> Folder -> save. In the folder field Tab completes and then walks through the matching folders.
    fn project_form_key(&mut self, event: KeyEvent) -> Result<()> {
        let Some(Popup::Project(form)) = self.popup.as_mut() else {
            return Ok(());
        };
        form.error = None;
        let listing = form.completion.active();
        let on_name = form.field == Field::Name;
        match event.code {
            KeyCode::Esc if listing => form.completion.clear(),
            KeyCode::Esc => self.popup = None,
            _ if is_ctrl_c(&event) => self.popup = None,
            KeyCode::Tab | KeyCode::Enter | KeyCode::Down if on_name => form.field = Field::Folder,
            KeyCode::Tab => form.complete(true),
            KeyCode::BackTab | KeyCode::Up if listing => form.complete(false),
            KeyCode::Down if listing => form.complete(true),
            KeyCode::BackTab | KeyCode::Up => form.field = Field::Name,
            // Enter on a highlighted folder only accepts it; the next Enter saves.
            KeyCode::Enter if form.completion.pick.is_some() => form.completion.clear(),
            KeyCode::Enter => {
                let (editing, name, folder) = (
                    form.editing,
                    form.name.text.clone(),
                    form.folder.text.clone(),
                );
                match self.submit_project(editing, &name, &folder) {
                    Ok(()) => self.popup = None,
                    Err(error) => {
                        if let Some(Popup::Project(form)) = self.popup.as_mut() {
                            form.field = if name.trim().is_empty() {
                                Field::Name
                            } else {
                                Field::Folder
                            };
                            form.error = Some(error.to_string());
                        }
                    }
                }
            }
            _ => {
                form.completion.clear();
                match form.field {
                    Field::Name => form.name.key(event),
                    Field::Folder => form.folder.key(event),
                }
            }
        }
        Ok(())
    }

    pub fn paste(&mut self, text: &str) {
        self.needs_draw = true;
        if let Some(prompt) = &mut self.prompt {
            prompt.input.insert(text);
        } else if let Some(Popup::Project(form)) = &mut self.popup {
            form.completion.clear();
            match form.field {
                Field::Name => form.name.insert(text),
                Field::Folder => form.folder.insert(text),
            }
        } else if self.terminal_mode {
            self.server.paste(text);
        } else if self.insert && self.buffer.path.is_some() {
            self.buffer.insert_str(text);
        }
    }

    pub fn mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if self.prompt.is_some() || self.popup.is_some() {
            return Ok(());
        }
        let point = Position::new(mouse.column, mouse.row);
        let areas = self.areas;
        let pane = [
            (Pane::Projects, areas.projects),
            (Pane::Env, areas.env),
            (Pane::Editor, areas.editor),
            (Pane::Server, areas.server),
        ]
        .into_iter()
        .find(|(_, r)| r.contains(point));
        let Some((pane, area)) = pane else {
            return Ok(());
        };
        let scroll = match mouse.kind {
            MouseEventKind::ScrollDown => Some(Action::Down),
            MouseEventKind::ScrollUp => Some(Action::Up),
            _ => None,
        };
        if let Some(action) = scroll {
            self.needs_draw = true;
            let previous = self.pane;
            self.pane = pane;
            let result = self.perform(action, if pane == Pane::Server { 3 } else { 1 });
            // The wheel scrolls whatever is under the pointer without moving the focus there.
            self.pane = previous;
            return result;
        }
        let MouseEventKind::Down(MouseButton::Left) = mouse.kind else {
            return Ok(());
        };
        self.needs_draw = true;
        let double = self.last_click.is_some_and(|(x, y, t)| {
            x == mouse.column && y == mouse.row && t.elapsed() < DOUBLE_CLICK
        });
        self.last_click = Some((mouse.column, mouse.row, Instant::now()));
        if self.pane != pane {
            self.focus(pane);
        }
        let Some(row) = content_row(area, mouse.row) else {
            return Ok(());
        };
        match pane {
            Pane::Projects => self.select_project(self.project_offset + row / 2),
            Pane::Env => {
                let target = self.profile_offset + row;
                self.select_profile(target)?;
                if double && target == self.profile {
                    self.start()?;
                }
                Ok(())
            }
            Pane::Editor if self.buffer.path.is_some() => {
                self.click_editor(area, row, mouse.column);
                if double {
                    self.perform(Action::Insert, 1)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn click_editor(&mut self, area: Rect, row: usize, column: u16) {
        use unicode_width::UnicodeWidthChar;
        self.buffer.row = self.buffer.scroll + row;
        self.buffer.clamp(false);
        let gutter = crate::editor::view::GUTTER;
        let cell = usize::from(column.saturating_sub(area.x + gutter));
        let mut width = 0;
        let mut col = self.buffer.hscroll;
        for ch in self.buffer.lines[self.buffer.row]
            .chars()
            .skip(self.buffer.hscroll)
        {
            let next = width + ch.width().unwrap_or(0);
            if next > cell {
                break;
            }
            width = next;
            col += 1;
        }
        self.buffer.col = col;
        self.buffer.clamp(false);
    }
}

// Pane rows start below the one-line header.
fn content_row(area: Rect, y: u16) -> Option<usize> {
    (y > area.y && y < area.bottom()).then(|| usize::from(y - area.y - 1))
}

pub fn tilde(path: &std::path::Path) -> String {
    let text = path.to_string_lossy().into_owned();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && text.starts_with(&home) => {
            format!("~{}", &text[home.len()..])
        }
        _ => text,
    }
}
