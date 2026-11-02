use crate::{app::App, keymap::notation};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

// The last line: the ':' prompt while typing a command, otherwise messages plus Vim's showcmd on the right.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let width = usize::from(area.width);
    if let Some(prompt) = &app.prompt {
        let before: String = prompt
            .input
            .text
            .chars()
            .take(prompt.input.cursor)
            .collect();
        let label_width = prompt.label.width();
        let room = width.saturating_sub(label_width + 1);
        let shift = before.width().saturating_sub(room);
        let shown: String = prompt.input.text.chars().skip(shift).collect();
        let spans = vec![
            Span::styled(prompt.label.clone(), theme.base),
            Span::styled(shown, theme.base),
        ];
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        frame.set_cursor_position((
            area.x + (label_width + before.width() - shift) as u16,
            area.y,
        ));
        return;
    }
    let showcmd = format!(
        "{}{}",
        app.count.map(|c| c.to_string()).unwrap_or_default(),
        notation::display(&app.pending)
    );
    let room = width.saturating_sub(showcmd.width() + 12);
    let message = app.message.as_ref().map_or_else(
        || Span::raw(""),
        |m| {
            Span::styled(
                crate::ui::truncate_right(&m.text, room),
                if m.error { theme.error } else { theme.base },
            )
        },
    );
    let gap = width.saturating_sub(message.content.width() + showcmd.width() + 10);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            message,
            Span::raw(" ".repeat(gap)),
            Span::styled(format!("{showcmd:<10}"), theme.comment),
        ])),
        area,
    );
}
