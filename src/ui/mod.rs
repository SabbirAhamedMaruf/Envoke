pub mod cmdline;
pub mod icons;
pub mod popup;
pub mod statusline;

use crate::{
    app::{App, Areas, Pane},
    editor, profiles, projects, server,
    theme::Theme,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const MIN_WIDTH: u16 = 72;
pub const MIN_HEIGHT: u16 = 18;

/*
 Projects │ Env files │ Preview / editor
 ── server terminal header ─────────────
 server terminal
 ───────────────────────────────────────
 statusline
 command line / messages
*/
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let theme = &app.theme;
    frame.render_widget(Block::default().style(theme.base), area);
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let lines = vec![
            Line::from(Span::styled(
                "envoke",
                theme.title.add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!(
                "Make the terminal at least {MIN_WIDTH} × {MIN_HEIGHT}."
            )),
            Line::from(Span::styled(":q quits", theme.comment)),
        ];
        frame.render_widget(Paragraph::new(lines), area);
        app.areas = Areas::default();
        return;
    }
    let fixed = 4;
    let body = area.height - fixed;
    let term = (body * app.term_height / 100).clamp(3, body.saturating_sub(6));
    let rows = Layout::vertical([
        Constraint::Min(6),
        Constraint::Length(1),
        Constraint::Length(term),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let columns = Layout::horizontal([
        Constraint::Percentage(22),
        Constraint::Length(1),
        Constraint::Percentage(22),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .split(rows[0]);
    let server_area = Rect {
        height: rows[1].height + rows[2].height,
        ..rows[1]
    };
    app.areas = Areas {
        projects: columns[0],
        env: columns[2],
        editor: columns[4],
        server: server_area,
    };
    vertical_rule(frame, columns[1], app.theme.separator);
    vertical_rule(frame, columns[3], app.theme.separator);
    projects::view::render(frame, app, columns[0]);
    profiles::view::render(frame, app, columns[2]);
    editor::view::render(frame, app, columns[4]);
    server::view::render(frame, app, server_area);
    horizontal_rule(frame, rows[3], app.theme.separator);
    statusline::render(frame, app, rows[4]);
    cmdline::render(frame, app, rows[5]);
    if app.popup.is_some() {
        popup::render(frame, app);
    }
}

fn vertical_rule(frame: &mut Frame, area: Rect, style: Style) {
    let lines: Vec<Line> = (0..area.height).map(|_| Line::from("│")).collect();
    frame.render_widget(Paragraph::new(lines).style(style), area);
}

pub fn horizontal_rule(frame: &mut Frame, area: Rect, style: Style) {
    frame.render_widget(
        Paragraph::new("─".repeat(usize::from(area.width))).style(style),
        area,
    );
}

// Pane header in the spirit of a Neovim winbar: icon + title on the left, a counter on the right.
pub fn header(frame: &mut Frame, app: &App, area: Rect, pane: Pane, title: &str, right: &str) {
    let theme = &app.theme;
    let active = app.pane == pane;
    let title_style = if active {
        theme.title.add_modifier(Modifier::BOLD)
    } else {
        theme.comment
    };
    let left = format!(" {title}");
    let right = format!("{right} ");
    let gap = usize::from(area.width).saturating_sub(left.width() + right.width());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left, title_style),
            Span::raw(" ".repeat(gap)),
            Span::styled(right, theme.comment),
        ])),
        Rect { height: 1, ..area },
    );
}

pub fn body(area: Rect) -> Rect {
    Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    }
}

// Keeps the end of a long text (the folder name matters more than /home/...) and marks the cut with an ellipsis.
pub fn truncate_left(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    let mut kept: Vec<char> = vec![];
    let mut used = 1;
    for c in text.chars().rev() {
        let w = c.width().unwrap_or(0);
        if used + w > width {
            break;
        }
        used += w;
        kept.push(c);
    }
    std::iter::once('…').chain(kept.into_iter().rev()).collect()
}

pub fn truncate_right(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut used = 1;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > width {
            break;
        }
        used += w;
        out.push(c);
    }
    out.push('…');
    out
}

// One row of choices with the picked one highlighted, scrolled so the pick is always on screen.
pub fn wildmenu(
    items: &[String],
    pick: Option<usize>,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let gap = 2;
    let widths: Vec<usize> = items.iter().map(|i| i.width()).collect();
    let mut start = 0;
    if let Some(pick) = pick {
        while start < pick
            && widths[start..=pick].iter().map(|w| w + gap).sum::<usize>() + 2 > width
        {
            start += 1;
        }
    }
    let mut spans = vec![Span::styled(
        if start > 0 { "<" } else { " " },
        theme.statusline,
    )];
    let mut used = 1;
    for (index, item) in items.iter().enumerate().skip(start) {
        if used + widths[index] + gap + 1 > width {
            spans.push(Span::styled(">", theme.statusline));
            break;
        }
        let style = if Some(index) == pick {
            theme.pmenu_sel
        } else {
            theme.statusline
        };
        spans.push(Span::styled(format!(" {item} "), style));
        used += widths[index] + gap;
    }
    Line::from(spans)
}

pub fn keep_visible(selected: usize, offset: &mut usize, capacity: usize) {
    let capacity = capacity.max(1);
    if selected < *offset {
        *offset = selected;
    }
    if selected >= *offset + capacity {
        *offset = selected + 1 - capacity;
    }
}
