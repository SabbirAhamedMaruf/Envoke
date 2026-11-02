use crate::{
    app::{App, Pane},
    ui::{self, keep_visible},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let total = app.selected().map_or(0, |p| p.env_files.len());
    let title = format!("{}Env files", app.icons.env);
    ui::header(frame, app, area, Pane::Env, &title, &total.to_string());
    let inner = ui::body(area);
    let Some(project) = app.config.projects.get(app.project) else {
        return;
    };
    let theme = &app.theme;
    if project.env_files.is_empty() {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(" No env files found", theme.warn)),
            Line::from(Span::styled(" a  create one", theme.comment)),
            Line::from(Span::styled(" R  rescan the folder", theme.comment)),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }
    let capacity = usize::from(inner.height);
    keep_visible(app.profile, &mut app.profile_offset, capacity);
    let theme = &app.theme;
    let width = usize::from(inner.width);
    let focused = app.pane == Pane::Env;
    let mut lines = vec![];
    for (index, file) in project
        .env_files
        .iter()
        .enumerate()
        .skip(app.profile_offset)
        .take(capacity)
    {
        let selected = index == app.profile;
        let running = app.server.runs_profile(&project.path, file);
        let row_style = if selected && !theme.flat {
            theme.cursor_line
        } else {
            theme.base
        };
        let badge = if running { "● run " } else { "" };
        let name_style = match (selected, running) {
            (_, true) => theme.ok.add_modifier(Modifier::BOLD),
            (true, _) if focused => theme.title.add_modifier(Modifier::BOLD),
            (true, _) => theme.base.add_modifier(Modifier::BOLD),
            _ => theme.base,
        };
        let icon_style = if running { theme.ok } else { theme.warn };
        let room = width.saturating_sub(2 + app.icons.env.width() + badge.width());
        let name = ui::truncate_left(&file.to_string_lossy(), room);
        let gap = room.saturating_sub(name.width());
        lines.push(
            Line::from(vec![
                Span::styled(if selected { "▎" } else { " " }, theme.title),
                Span::raw(" "),
                Span::styled(app.icons.env, icon_style),
                Span::styled(name, name_style),
                Span::raw(" ".repeat(gap)),
                Span::styled(badge, theme.ok),
            ])
            .style(row_style),
        );
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
