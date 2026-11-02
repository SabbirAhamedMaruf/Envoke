use super::{
    App, Pane,
    input::tilde,
    prompt::{Confirm, Kind, Popup, ProjectForm, Prompt},
};
use crate::{editor::vim, keymap::Action};
use anyhow::Result;

const TOP: [Pane; 3] = [Pane::Projects, Pane::Env, Pane::Editor];
const CYCLE: [Pane; 4] = [Pane::Projects, Pane::Env, Pane::Editor, Pane::Server];

impl App {
    pub fn perform(&mut self, action: Action, count: usize) -> Result<()> {
        use Action as A;
        match action {
            A::FocusNext | A::FocusPrev => {
                let at = CYCLE.iter().position(|p| *p == self.pane).unwrap_or(0);
                let step = if action == A::FocusNext {
                    1
                } else {
                    CYCLE.len() - 1
                };
                self.focus(CYCLE[(at + step) % CYCLE.len()]);
            }
            A::FocusLeft | A::FocusRight => {
                if let Some(at) = TOP.iter().position(|p| *p == self.pane) {
                    let next = if action == A::FocusRight {
                        (at + 1).min(TOP.len() - 1)
                    } else {
                        at.saturating_sub(1)
                    };
                    self.focus(TOP[next]);
                }
            }
            A::FocusDown => self.focus(Pane::Server),
            A::FocusUp => self.focus(self.last_top),
            A::CommandLine => self.prompt = Some(Prompt::new(Kind::Command, ":", "")),
            A::Help => self.popup = Some(Popup::Help { scroll: 0 }),
            A::Quit => self.request_quit(false),
            A::GrowTerminal => self.term_height = (self.term_height + 5 * count as u16).min(70),
            A::ShrinkTerminal => {
                self.term_height = self.term_height.saturating_sub(5 * count as u16).max(20)
            }
            A::ToggleMask => self.masked = !self.masked,
            A::Down | A::Up | A::Top | A::Bottom | A::HalfPageDown | A::HalfPageUp => {
                self.navigate(action, count)?
            }
            A::Open | A::Preview => self.focus(if action == A::Open {
                Pane::Env
            } else {
                Pane::Editor
            }),
            A::Back => self.focus(Pane::Projects),
            A::AddProject => self.popup = Some(Popup::Project(ProjectForm::new(false, "", "~/"))),
            A::EditProject => {
                if let Some(project) = self.selected() {
                    let form = ProjectForm::new(true, project.name.clone(), tilde(&project.path));
                    self.popup = Some(Popup::Project(form));
                }
            }
            A::RemoveProject => {
                if let Some(project) = self.selected() {
                    let label = format!(
                        "Remove {} from the list? Files are kept. [y/N] ",
                        project.name
                    );
                    self.prompt = Some(Prompt::new(
                        Kind::Confirm(Confirm::RemoveProject),
                        label,
                        "",
                    ));
                }
            }
            A::PickScript => {
                if let Some(project) = self.selected() {
                    let selected = project
                        .scripts
                        .iter()
                        .position(|s| Some(&s.name) == project.selected_script.as_ref())
                        .unwrap_or(0);
                    self.popup = Some(Popup::Scripts {
                        selected,
                        start_after: false,
                    });
                }
            }
            A::Rescan => self.rescan()?,
            A::Start => self.start()?,
            A::Restart => self.restart()?,
            A::Stop => self.stop()?,
            A::NewEnv => {
                if self.selected().is_some() {
                    self.prompt = Some(Prompt::new(Kind::NewEnv, "New env file: ", ".env."));
                }
            }
            A::TerminalInsert => {
                if self.server.running() {
                    self.server.scroll = 0;
                    self.terminal_mode = true;
                } else {
                    self.info("No server is running · Enter on an env file starts one");
                }
            }
            A::NormalMode => {
                if self.insert {
                    self.leave_insert();
                }
                self.terminal_mode = false;
            }
            A::Save => self.save_buffer()?,
            _ => self.edit(action, count)?,
        }
        Ok(())
    }

    fn navigate(&mut self, action: Action, count: usize) -> Result<()> {
        use Action as A;
        let half = usize::from(self.areas.editor.height.saturating_sub(1) / 2).max(1);
        match self.pane {
            Pane::Projects | Pane::Env => {
                let (current, len) = if self.pane == Pane::Projects {
                    (self.project, self.config.projects.len())
                } else {
                    (
                        self.profile,
                        self.selected().map_or(0, |p| p.env_files.len()),
                    )
                };
                if len == 0 {
                    return Ok(());
                }
                let target = match action {
                    A::Down => (current + count).min(len - 1),
                    A::Up => current.saturating_sub(count),
                    A::Top => 0,
                    _ => len - 1,
                };
                if self.pane == Pane::Projects {
                    self.select_project(target)
                } else {
                    self.select_profile(target)
                }
            }
            Pane::Editor => {
                let b = &mut self.buffer;
                match action {
                    A::Down => b.row += count,
                    A::Up => b.row = b.row.saturating_sub(count),
                    A::Top => b.row = 0,
                    A::Bottom => b.row = b.lines.len().saturating_sub(1),
                    A::HalfPageDown => b.row += half * count,
                    _ => b.row = b.row.saturating_sub(half * count),
                }
                if matches!(action, A::Top | A::Bottom) {
                    b.clamp(false);
                    b.col = vim::first_non_blank(&b.lines[b.row]);
                }
                b.clamp(false);
                Ok(())
            }
            Pane::Server => {
                let page = usize::from(self.areas.server.height.saturating_sub(1)).max(2);
                let step = count as isize;
                match action {
                    A::Down => self.server.scroll_by(-step),
                    A::Up => self.server.scroll_by(step),
                    A::HalfPageDown => self.server.scroll_by(-((page / 2) as isize) * step),
                    A::HalfPageUp => self.server.scroll_by((page / 2) as isize * step),
                    A::Top => self.server.scroll_to_top(),
                    _ => self.server.scroll = 0,
                }
                Ok(())
            }
        }
    }

    // Editor commands in NORMAL mode.
    fn edit(&mut self, action: Action, count: usize) -> Result<()> {
        use Action as A;
        if self.buffer.path.is_none() {
            return Ok(());
        }
        let b = &mut self.buffer;
        b.clamp(false);
        match action {
            A::Left => b.col = b.col.saturating_sub(count),
            A::Right => b.col += count,
            A::WordNext | A::WordPrev | A::WordEnd => {
                for _ in 0..count {
                    let (row, col) = match action {
                        A::WordNext => vim::word_next(b, b.row, b.col),
                        A::WordPrev => vim::word_prev(b, b.row, b.col),
                        _ => vim::word_end(b, b.row, b.col),
                    };
                    (b.row, b.col) = (row, col);
                }
            }
            A::LineStart => b.col = 0,
            A::FirstNonBlank => b.col = vim::first_non_blank(&b.lines[b.row]),
            A::LineEnd => b.col = usize::MAX,
            A::DeleteChar => b.delete_chars(count),
            A::DeleteLine => b.delete_lines(count),
            A::DeleteToEnd => b.delete_to_end(),
            A::YankLine => {
                let lines = b.yank_lines(count);
                self.info(format!(
                    "{lines} line{} yanked",
                    if lines == 1 { "" } else { "s" }
                ));
            }
            A::PasteAfter | A::PasteBefore => {
                if !b.paste(action == A::PasteAfter, count) {
                    self.error("E353: Nothing in register \"");
                }
            }
            A::Undo => {
                if !(0..count).fold(false, |done, _| b.undo() || done) {
                    self.info("Already at oldest change");
                }
            }
            A::Redo => {
                if !(0..count).fold(false, |done, _| b.redo() || done) {
                    self.info("Already at newest change");
                }
            }
            A::Insert | A::Append | A::InsertLineStart | A::AppendLineEnd => {
                match action {
                    A::Append if b.line_len(b.row) > 0 => b.col += 1,
                    A::InsertLineStart => b.col = vim::first_non_blank(&b.lines[b.row]),
                    A::AppendLineEnd => b.col = usize::MAX,
                    _ => {}
                }
                self.enter_insert();
            }
            A::OpenBelow | A::OpenAbove => {
                self.enter_insert();
                self.buffer.open_line(action == A::OpenBelow);
            }
            A::AddKey => {
                self.buffer.add_key();
                self.insert = true;
                self.masked = false;
                self.buffer.clamp(true);
            }
            _ => {}
        }
        if !self.insert {
            self.buffer.clamp(false);
        }
        Ok(())
    }

    // The whole INSERT session becomes one undo step, as in Vim.
    fn enter_insert(&mut self) {
        self.buffer.snapshot();
        self.insert = true;
        self.masked = false;
        self.buffer.clamp(true);
    }

    pub fn leave_insert(&mut self) {
        self.insert = false;
        self.buffer.drop_empty_snapshot();
        self.buffer.col = self.buffer.col.saturating_sub(1);
        self.buffer.clamp(false);
    }

    pub fn discard(&mut self) -> Result<()> {
        let (row, col) = (self.buffer.row, self.buffer.col);
        self.load_profile()?;
        (self.buffer.row, self.buffer.col) = (row, col);
        self.buffer.clamp(false);
        self.info(format!("\"{}\" reloaded from disk", self.buffer.name()));
        Ok(())
    }
}
