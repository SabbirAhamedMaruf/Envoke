use crate::{
    app::{App, Pane, input::tilde},
    ui::{self, keep_visible},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let count = app.config.projects.len().to_string();
    let title = format!("{}Projects", app.icons.projects);
    ui::header(frame, app, area, Pane::Projects, &title, &count);
    let inner = ui::body(area);
    let theme = &app.theme;
    if app.config.projects.is_empty() {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(" No projects yet", theme.title)),
            Line::from(Span::styled(" a  add a project", theme.comment)),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }
    let capacity = usize::from(inner.height / 2);
    keep_visible(app.project, &mut app.project_offset, capacity);
    let theme = &app.theme;
    let width = usize::from(inner.width);
    let focused = app.pane == Pane::Projects;
    let mut lines = vec![];
    for (index, project) in app
        .config
        .projects
        .iter()
        .enumerate()
        .skip(app.project_offset)
        .take(capacity)
    {
        let selected = index == app.project;
        let running = app.server.runs(&project.path);
        let row_style = if selected && !theme.flat {
            theme.cursor_line
        } else {
            theme.base
        };
        let marker = if selected { "▎" } else { " " };
        let name_style = if selected {
            let s = if focused { theme.title } else { theme.base };
            s.add_modifier(Modifier::BOLD)
        } else {
            theme.base
        };
        let dot = if running { " ●" } else { "" };
        let name = ui::truncate_right(&project.name, width.saturating_sub(3 + dot.len()));
        lines.push(
            Line::from(vec![
                Span::styled(marker, theme.title),
                Span::raw(" "),
                Span::styled(name, name_style),
                Span::styled(dot, theme.ok),
            ])
            .style(row_style),
        );
        let path = ui::truncate_left(&tilde(&project.path), width.saturating_sub(2));
        lines.push(
            Line::from(vec![
                Span::styled(marker, theme.title),
                Span::raw(" "),
                Span::styled(path, theme.comment),
            ])
            .style(row_style),
        );
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
