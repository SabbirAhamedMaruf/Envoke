use crate::{
    app::{App, Pane},
    ui::{self, keep_visible},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthChar;

// Line-number column: " 1234 " plus one space before the text.
pub const GUTTER: u16 = 7;

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let name = app.buffer.name();
    let mut title = format!(
        "{}{}",
        app.icons.file,
        if name.is_empty() { "Preview" } else { &name }
    );
    if app.buffer.dirty() {
        title.push_str(" [+]");
    }
    let right = if app.masked {
        format!("{} masked", app.icons.masked)
    } else if app.buffer.path.is_some() {
        format!("{}L", app.buffer.lines.len())
    } else {
        String::new()
    };
    ui::header(frame, app, area, Pane::Editor, &title, &right);
    let inner = ui::body(area);
    let theme = &app.theme;
    if app.buffer.path.is_none() {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(" Nothing to preview", theme.title)),
            Line::from(Span::styled(
                " Pick a project and an env file; i edits, :w saves.",
                theme.comment,
            )),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }
    let height = usize::from(inner.height).max(1);
    let width = usize::from(inner.width.saturating_sub(GUTTER)).max(1);
    let focused = app.pane == Pane::Editor;
    let insert = app.insert;
    let b = &mut app.buffer;
    b.clamp(insert);
    keep_visible(b.row, &mut b.scroll, height);
    if b.col < b.hscroll {
        b.hscroll = b.col;
    }
    let cursor_width = |b: &crate::editor::Buffer| -> usize {
        b.lines[b.row]
            .chars()
            .skip(b.hscroll)
            .take(b.col.saturating_sub(b.hscroll))
            .map(|c| c.width().unwrap_or(0))
            .sum()
    };
    while cursor_width(b) >= width {
        b.hscroll += 1;
    }
    let theme = &app.theme;
    let b = &app.buffer;
    let mut lines = vec![];
    for (row, line) in b.lines.iter().enumerate().skip(b.scroll).take(height) {
        let current = row == b.row;
        let number_style = if current {
            theme.cursor_line_nr
        } else {
            theme.line_nr
        };
        let mut spans = vec![Span::styled(format!(" {:>4}  ", row + 1), number_style)];
        spans.extend(highlight(app, line, b.hscroll));
        let style = if current && focused && !theme.flat {
            theme.cursor_line
        } else {
            Style::default()
        };
        lines.push(Line::from(spans).style(style));
    }
    frame.render_widget(Paragraph::new(lines), inner);
    if focused && app.prompt.is_none() && app.popup.is_none() {
        frame.set_cursor_position((
            inner.x + GUTTER + cursor_width(b) as u16,
            inner.y + (b.row - b.scroll) as u16,
        ));
    }
}

// Comment / KEY / = / value colors from the theme; masking hides every character, multiline values included.
fn highlight(app: &App, line: &str, skip: usize) -> Vec<Span<'static>> {
    let theme = &app.theme;
    let visible: String = if app.masked {
        "•".repeat(line.chars().count().saturating_sub(skip))
    } else {
        line.chars().skip(skip).collect()
    };
    if app.masked {
        return vec![Span::styled(visible, theme.comment)];
    }
    if line.trim_start().starts_with('#') {
        return vec![Span::styled(visible, theme.comment)];
    }
    match visible.split_once('=') {
        Some((key, value)) => {
            let (export, key) = match key.strip_prefix("export ") {
                Some(rest) => ("export ", rest),
                None => ("", key),
            };
            vec![
                Span::styled(export.to_owned(), theme.operator),
                Span::styled(key.to_owned(), theme.key),
                Span::styled("=", theme.operator),
                Span::styled(value.to_owned(), theme.string),
            ]
        }
        None => vec![Span::styled(visible, theme.string)],
    }
}
