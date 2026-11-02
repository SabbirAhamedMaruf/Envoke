use crate::app::{
    App,
    commands::COMMANDS,
    prompt::{Field, Input, Popup, ProjectForm},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

fn frame_popup(frame: &mut Frame, app: &App, title: &str, width: u16, height: u16) -> Rect {
    let screen = frame.area();
    let width = width.min(screen.width.saturating_sub(4));
    let height = height.min(screen.height.saturating_sub(4));
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    );
    let theme = &app.theme;
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.float_border)
        .style(theme.float)
        .title(Span::styled(format!(" {title} "), theme.float_title));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

pub fn render(frame: &mut Frame, app: &mut App) {
    match &app.popup {
        Some(Popup::Help { .. }) => help(frame, app),
        Some(Popup::Scripts { selected, .. }) => scripts(frame, app, *selected),
        Some(Popup::Project(form)) => project_form(frame, app, form),
        None => {}
    }
}

const LIST_ROWS: usize = 8;
const LABEL: u16 = 12;

/*
 ▎ Name      My app
   Folder    ~/Projects/
             󰉋 api/             <- folders matching the Tab completion, picked one highlighted
             󰉋 web-app/
   error or hint
   key hint
*/
fn project_form(frame: &mut Frame, app: &App, form: &ProjectForm) {
    let theme = &app.theme;
    let list = &form.completion.items;
    let shown = list.len().min(LIST_ROWS);
    let title = format!(
        "{}{}",
        app.icons.folder,
        if form.editing {
            "Edit project"
        } else {
            "Add project"
        }
    );
    let inner = frame_popup(frame, app, &title, 76, 9 + shown as u16);
    let row = |n: u16| Rect {
        y: inner.y + n,
        height: 1,
        ..inner
    };
    field(
        frame,
        app,
        row(1),
        "Name",
        &form.name,
        form.field == Field::Name,
    );
    field(
        frame,
        app,
        row(3),
        "Folder",
        &form.folder,
        form.field == Field::Folder,
    );
    let offset = form
        .completion
        .pick
        .map_or(0, |p| p.saturating_sub(LIST_ROWS - 1));
    for (i, item) in list.iter().enumerate().skip(offset).take(LIST_ROWS) {
        let area = row(4 + (i - offset) as u16);
        let area = Rect {
            x: area.x + LABEL + 1,
            width: area.width.saturating_sub(LABEL + 2),
            ..area
        };
        let style = if form.completion.pick == Some(i) {
            theme.pmenu_sel
        } else {
            theme.float
        };
        let text = format!(" {}{item}", app.icons.folder);
        frame.render_widget(Paragraph::new(text).style(style), area);
    }
    let message = match &form.error {
        Some(error) => Span::styled(format!("  {error}"), theme.error),
        None if list.len() > LIST_ROWS => Span::styled(
            format!(
                "  {} folders · keep pressing Tab, or type more letters",
                list.len()
            ),
            theme.comment,
        ),
        None => Span::styled(
            "  Env files and run scripts are found automatically.",
            theme.comment,
        ),
    };
    frame.render_widget(Paragraph::new(message), row(5 + shown as u16));
    let keys = if form.field == Field::Name {
        "  Tab / Enter next field  ·  Esc cancel"
    } else if form.completion.active() {
        "  Tab / ↓ next  ·  S-Tab / ↑ previous  ·  Enter choose  ·  Esc close list"
    } else {
        "  Tab complete folder  ·  Enter save  ·  S-Tab back to name  ·  Esc cancel"
    };
    frame.render_widget(
        Paragraph::new(Span::styled(keys, theme.comment)),
        row(6 + shown as u16),
    );
}

fn field(frame: &mut Frame, app: &App, area: Rect, label: &str, input: &Input, active: bool) {
    let theme = &app.theme;
    let label_style = if active {
        theme.title.add_modifier(Modifier::BOLD)
    } else {
        theme.comment
    };
    let marker = if active { "▎ " } else { "  " };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(marker, theme.title),
            Span::styled(label.to_owned(), label_style),
        ])),
        area,
    );
    let box_area = Rect {
        x: area.x + LABEL,
        width: area.width.saturating_sub(LABEL + 1),
        ..area
    };
    let room = usize::from(box_area.width).saturating_sub(2);
    let before: String = input.text.chars().take(input.cursor).collect();
    let shift = before.width().saturating_sub(room);
    let shown: String = input.text.chars().skip(shift).collect();
    // NvChad themes get a cursorline-colored box; the terminal theme gets an underline instead.
    let box_style = if theme.flat {
        theme.float.add_modifier(Modifier::UNDERLINED)
    } else {
        theme.cursor_line.fg(theme.float.fg.unwrap_or_default())
    };
    frame.render_widget(
        Paragraph::new(format!(" {shown}")).style(box_style),
        box_area,
    );
    if active {
        frame.set_cursor_position((box_area.x + 1 + (before.width() - shift) as u16, box_area.y));
    }
}

fn help(frame: &mut Frame, app: &mut App) {
    let theme = &app.theme;
    let mut lines = vec![];
    let mut section = "";
    for (ctx, keys, desc) in app.keymap.help() {
        if ctx != section {
            section = ctx;
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                format!(" {ctx}"),
                theme.title.add_modifier(Modifier::BOLD),
            )));
        }
        lines.push(Line::from(vec![
            Span::styled(format!("   {keys:<22}"), theme.key),
            Span::styled(desc, theme.float),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " commands  (long  ·  short)",
        theme.title.add_modifier(Modifier::BOLD),
    )));
    for spec in COMMANDS {
        let long = format!(":{} {}", spec.long, spec.usage);
        lines.push(Line::from(vec![
            Span::styled(format!("   {:<24}", long.trim_end()), theme.key),
            Span::styled(format!("{:<7}", format!(":{}", spec.short)), theme.string),
            Span::styled(spec.desc, theme.float),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled(format!("   {:<24}", ":<number>"), theme.key),
        Span::styled(format!("{:<7}", ""), theme.string),
        Span::styled("Jump to that line in the editor", theme.float),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            " Change any key in {}",
            crate::app::input::tilde(&app.keymap.path)
        ),
        theme.comment,
    )));
    let title = format!(
        "{}Keys  ·  j/k scroll  ·  G commands  ·  q close",
        app.icons.help
    );
    let height = frame.area().height.saturating_sub(4);
    let inner = frame_popup(frame, app, &title, 92, height);
    let visible = usize::from(inner.height);
    let total = lines.len();
    let Some(Popup::Help { scroll }) = &mut app.popup else {
        return;
    };
    *scroll = (*scroll).min(total.saturating_sub(visible));
    let shown: Vec<Line> = lines.into_iter().skip(*scroll).take(visible).collect();
    frame.render_widget(Paragraph::new(shown), inner);
}

fn scripts(frame: &mut Frame, app: &App, selected: usize) {
    let Some(project) = app.selected() else {
        return;
    };
    let title = format!(
        "{}Run script  ·  Enter choose  ·  q close",
        app.icons.script
    );
    let height = (project.scripts.len() as u16 * 2 + 3).clamp(6, 24);
    let inner = frame_popup(frame, app, &title, 76, height);
    let theme = &app.theme;
    let capacity = usize::from(inner.height.saturating_sub(1) / 2).max(1);
    let offset = selected.saturating_sub(capacity - 1);
    let mut lines = vec![Line::from("")];
    if project.scripts.is_empty() {
        lines.push(Line::from(Span::styled(
            " No scripts found. Add package.json scripts, Cargo.toml or pyproject.toml, then R.",
            theme.warn,
        )));
    }
    for (index, script) in project
        .scripts
        .iter()
        .enumerate()
        .skip(offset)
        .take(capacity)
    {
        let style = if index == selected {
            theme.pmenu_sel
        } else {
            theme.float
        };
        let mark = if project.selected_script.as_ref() == Some(&script.name) {
            "● "
        } else {
            "  "
        };
        lines.push(
            Line::from(vec![
                Span::raw(format!(" {mark}")),
                Span::styled(
                    script.name.clone(),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  {}", script.source)),
            ])
            .style(style),
        );
        lines.push(Line::from(Span::styled(
            format!("     {}", script.command()),
            theme.comment,
        )));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
