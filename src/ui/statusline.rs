use crate::{
    app::{App, Pane},
    server::view::state_badge,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

// NvChad's "default" statusline: mode, file, script ... server state, project, position.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    // While Tab is completing a command, the statusline turns into Vim's wildmenu.
    if let Some(prompt) = &app.prompt
        && prompt.completion.active()
    {
        let c = &prompt.completion;
        let line = super::wildmenu(&c.items, c.pick, usize::from(area.width), theme);
        frame.render_widget(Paragraph::new(line).style(theme.statusline), area);
        return;
    }
    let icons = &app.icons;
    let mode = app.mode();
    let file = match app.buffer.name() {
        name if name.is_empty() => "[No Name]".to_owned(),
        name if app.buffer.dirty() => format!("{name} [+]"),
        name => name,
    };
    let left = vec![
        Span::styled(format!(" {} ", mode.label()), theme.mode(mode)),
        Span::styled(icons.sep_right, theme.mode_sep(mode)),
        Span::styled(icons.sep_right, theme.empty_space),
        Span::styled(format!(" {}{file} ", icons.file), theme.file),
        Span::styled(icons.sep_right, theme.file_sep),
    ];
    let script = app
        .selected()
        .and_then(|p| p.script())
        .map(|s| format!("  {}{}", icons.script, s.command()))
        .unwrap_or_default();
    let (badge, badge_style) = state_badge(app);
    let project = app
        .selected()
        .map_or_else(|| "no project".to_owned(), |p| p.name.clone());
    let cwd = vec![
        Span::styled(icons.sep_left, theme.cwd_sep),
        Span::styled(icons.folder, theme.cwd_icon),
        Span::styled(format!(" {project} "), theme.cwd_text),
    ];
    let pos = vec![
        Span::styled(icons.sep_left, theme.pos_sep),
        Span::styled(icons.pos, theme.pos_icon),
        Span::styled(format!(" {} ", position(app)), theme.pos_text),
    ];
    let width = usize::from(area.width);
    let measure = |spans: &[Span]| spans.iter().map(|s| s.content.width()).sum::<usize>();
    let mut middle_left = vec![Span::styled(script, theme.git)];
    let badge_style = match theme.statusline.bg {
        Some(bg) => badge_style.bg(bg),
        None => badge_style,
    };
    let mut middle_right = vec![Span::styled(format!("{badge}  "), badge_style)];
    let mut right: Vec<Span> = cwd.into_iter().chain(pos).collect();
    // Narrow terminals drop the least important parts first.
    for step in 0..3 {
        let used =
            measure(&left) + measure(&middle_left) + measure(&middle_right) + measure(&right);
        if used <= width {
            break;
        }
        match step {
            0 => middle_left.clear(),
            1 => right.truncate(3),
            _ => middle_right.clear(),
        }
    }
    let used = measure(&left) + measure(&middle_left) + measure(&middle_right) + measure(&right);
    let mut spans = left;
    spans.extend(middle_left);
    spans.push(Span::styled(
        " ".repeat(width.saturating_sub(used)),
        theme.statusline,
    ));
    spans.extend(middle_right);
    spans.extend(right);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(theme.statusline),
        area,
    );
}

fn position(app: &App) -> String {
    match app.pane {
        Pane::Projects => {
            let total = app.config.projects.len();
            format!("{}/{}", (app.project + 1).min(total), total)
        }
        Pane::Env => {
            let total = app.selected().map_or(0, |p| p.env_files.len());
            format!("{}/{}", (app.profile + 1).min(total), total)
        }
        Pane::Editor => format!("Ln {}, Col {}", app.buffer.row + 1, app.buffer.col + 1),
        Pane::Server if app.server.scroll > 0 => format!("↑{}", app.server.scroll),
        Pane::Server => "Bot".into(),
    }
}
