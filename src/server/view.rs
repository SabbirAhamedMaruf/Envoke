use super::State;
use crate::{
    app::{App, Pane},
    theme::Theme,
};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

pub fn uptime(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3600, secs % 3600 / 60),
    }
}

// Dot + word for the server state, shared with the statusline.
pub fn state_badge(app: &App) -> (String, Style) {
    let theme = &app.theme;
    let server = &app.server;
    let ports: Vec<String> = server.ports.iter().map(|p| format!(":{p}")).collect();
    let ports = if ports.is_empty() {
        String::new()
    } else {
        format!(" {}", ports.join(" "))
    };
    match &server.state {
        State::Idle => ("○ idle".into(), theme.comment),
        State::Starting => (format!("◌ starting{ports}"), theme.warn),
        State::Running => (format!("● running{ports}"), theme.ok),
        State::Stopped => ("○ stopped".into(), theme.comment),
        State::Exited { code, ok: true } => (format!("○ exited · {code}"), theme.comment),
        State::Exited { code, ok: false } => (format!("✗ crashed · {code}"), theme.error),
    }
}

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let header = Rect { height: 1, ..area };
    let body = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    render_header(frame, app, header);
    app.server.resize(body.height, body.width);
    if app.server.label.is_none() {
        let theme = &app.theme;
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No server yet · pick an env file and press Enter",
                theme.comment,
            )),
            Line::from(Span::styled(
                "  Its output shows up here, colors and all; i lets you type into it, <C-x> comes back.",
                theme.comment,
            )),
        ];
        frame.render_widget(Paragraph::new(lines), body);
        return;
    }
    let scroll = if app.terminal_mode {
        0
    } else {
        app.server.scroll
    };
    let screen = std::sync::Arc::clone(&app.server.screen);
    let Ok(mut parser) = screen.lock() else {
        return;
    };
    parser.screen_mut().set_scrollback(scroll);
    let cursor = paint(frame.buffer_mut(), parser.screen(), &app.theme, body);
    if app.terminal_mode
        && scroll == 0
        && app.prompt.is_none()
        && app.popup.is_none()
        && !parser.screen().hide_cursor()
        && let Some(position) = cursor
    {
        frame.set_cursor_position(position);
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let active = app.pane == Pane::Server;
    let title_style = if active {
        theme.title.add_modifier(Modifier::BOLD)
    } else {
        theme.comment
    };
    let mut left = vec![
        Span::styled("── ", theme.separator),
        Span::styled(format!("{}Server", app.icons.server), title_style),
    ];
    if let Some(label) = &app.server.label {
        left.push(Span::styled("  ", theme.separator));
        left.push(Span::styled(label.command.clone(), theme.base));
        left.push(Span::styled(
            format!("  ·  {}", label.profile.display()),
            theme.comment,
        ));
    }
    if app.server.scroll > 0 && !app.terminal_mode {
        left.push(Span::styled(
            format!("  ↑ {} lines back · G follows", app.server.scroll),
            theme.warn,
        ));
    }
    let (badge, badge_style) = state_badge(app);
    let mut right = vec![Span::styled(badge, badge_style)];
    if let Some(session) = &app.server.session {
        right.push(Span::styled(
            format!(
                "  ·  PID {}  ·  {}",
                session.pid,
                uptime(session.started.elapsed().as_secs())
            ),
            theme.comment,
        ));
    }
    right.push(Span::styled(" ──", theme.separator));
    let used: usize = left
        .iter()
        .chain(right.iter())
        .map(|s| s.content.width())
        .sum();
    let fill = usize::from(area.width).saturating_sub(used + 2);
    let mut spans = left;
    spans.push(Span::styled(
        format!(" {} ", "─".repeat(fill)),
        theme.separator,
    ));
    spans.extend(right);
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// Copies the emulated terminal's cells into the frame and reports where its cursor is.
fn paint(
    buf: &mut Buffer,
    screen: &vt100::Screen,
    theme: &Theme,
    area: Rect,
) -> Option<(u16, u16)> {
    for row in 0..area.height {
        for col in 0..area.width {
            let Some(target) = buf.cell_mut((area.x + col, area.y + row)) else {
                continue;
            };
            let Some(cell) = screen.cell(row, col) else {
                target.reset();
                continue;
            };
            // Ratatui itself blanks the cell hidden behind a double-width character.
            if cell.is_wide_continuation() {
                target.reset();
                continue;
            }
            let fg = theme.term_color(cell.fgcolor(), false);
            let bg = theme.term_color(cell.bgcolor(), true);
            let mut modifier = Modifier::empty();
            for (on, m) in [
                (cell.bold(), Modifier::BOLD),
                (cell.dim(), Modifier::DIM),
                (cell.italic(), Modifier::ITALIC),
                (cell.underline(), Modifier::UNDERLINED),
            ] {
                if on {
                    modifier |= m;
                }
            }
            // With real theme colors the swap is explicit; with terminal colors the terminal swaps its own defaults.
            let style = match (cell.inverse(), theme.ansi.is_some()) {
                (true, true) => Style::default().fg(bg).bg(fg),
                (true, false) => Style::default()
                    .fg(fg)
                    .bg(bg)
                    .add_modifier(Modifier::REVERSED),
                _ => Style::default().fg(fg).bg(bg),
            };
            let contents = cell.contents();
            target.reset();
            target.set_symbol(if contents.is_empty() { " " } else { contents });
            target.set_style(style.add_modifier(modifier));
        }
    }
    let (row, col) = screen.cursor_position();
    (row < area.height && col < area.width).then_some((area.x + col, area.y + row))
}
