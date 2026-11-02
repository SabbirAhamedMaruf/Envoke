mod nvchad;

use ratatui::style::{Color, Modifier, Style};

// Sentinel for "this highlight group sets no color", mirroring an unset fg/bg in Neovim.
pub const N: u32 = u32::MAX;

#[derive(Clone, Copy)]
pub struct Hl {
    pub fg: u32,
    pub bg: u32,
    pub bold: bool,
}

pub const fn hl(fg: u32, bg: u32, bold: bool) -> Hl {
    Hl { fg, bg, bold }
}

pub struct Palette {
    pub name: &'static str,
    pub light: bool,
    pub normal: Hl,
    pub float: Hl,
    pub float_border: Hl,
    pub float_title: Hl,
    pub separator: Hl,
    pub cursor_line: Hl,
    pub line_nr: Hl,
    pub cursor_line_nr: Hl,
    pub visual: Hl,
    pub comment: Hl,
    pub string: Hl,
    pub key: Hl,
    pub operator: Hl,
    pub title: Hl,
    pub directory: Hl,
    pub error: Hl,
    pub warn: Hl,
    pub info: Hl,
    pub ok: Hl,
    pub search: Hl,
    pub pmenu_sel: Hl,
    pub statusline: Hl,
    pub empty_space: Hl,
    pub file: Hl,
    pub file_sep: Hl,
    pub git: Hl,
    pub lsp: Hl,
    pub cwd_sep: Hl,
    pub cwd_icon: Hl,
    pub cwd_text: Hl,
    pub pos_sep: Hl,
    pub pos_icon: Hl,
    pub pos_text: Hl,
    pub modes: [Hl; 6],
    pub mode_seps: [Hl; 6],
    pub ansi: [u32; 16],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Terminal,
    TerminalNormal,
    Confirm,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Command => "COMMAND",
            Mode::Terminal => "TERMINAL",
            Mode::TerminalNormal => "NTERMINAL",
            Mode::Confirm => "CONFIRM",
        }
    }
    fn index(self) -> usize {
        self as usize
    }
}

pub struct Theme {
    pub name: String,
    pub base: Style,
    pub float: Style,
    pub float_border: Style,
    pub float_title: Style,
    pub separator: Style,
    pub cursor_line: Style,
    pub line_nr: Style,
    pub cursor_line_nr: Style,
    pub visual: Style,
    pub comment: Style,
    pub string: Style,
    pub key: Style,
    pub operator: Style,
    pub title: Style,
    pub directory: Style,
    pub error: Style,
    pub warn: Style,
    pub info: Style,
    pub ok: Style,
    pub search: Style,
    pub pmenu_sel: Style,
    pub statusline: Style,
    pub empty_space: Style,
    pub file: Style,
    pub file_sep: Style,
    pub git: Style,
    pub lsp: Style,
    pub cwd_sep: Style,
    pub cwd_icon: Style,
    pub cwd_text: Style,
    pub pos_sep: Style,
    pub pos_icon: Style,
    pub pos_text: Style,
    modes: [Style; 6],
    mode_seps: [Style; 6],
    pub ansi: Option<[Color; 16]>,
    // The terminal theme has no cursorline background, so selection is carried by a marker and bold text instead.
    pub flat: bool,
}

impl Theme {
    pub fn mode(&self, mode: Mode) -> Style {
        self.modes[mode.index()]
    }
    pub fn mode_sep(&self, mode: Mode) -> Style {
        self.mode_seps[mode.index()]
    }

    pub fn names() -> impl Iterator<Item = &'static str> {
        std::iter::once("terminal").chain(nvchad::THEMES.iter().map(|p| p.name))
    }

    pub fn by_name(name: &str) -> Option<Self> {
        if name == "terminal" {
            return Some(Self::terminal());
        }
        nvchad::THEMES
            .iter()
            .find(|p| p.name == name)
            .map(Self::from_palette)
    }

    // Follows whatever palette the terminal itself uses: only the 16 ANSI colors and the default fg/bg.
    pub fn terminal() -> Self {
        let fg = |c: Color| Style::default().fg(c);
        let block = |c: Color| {
            Style::default()
                .fg(Color::Black)
                .bg(c)
                .add_modifier(Modifier::BOLD)
        };
        let mode_colors = [
            Color::Blue,
            Color::Magenta,
            Color::Green,
            Color::Green,
            Color::Yellow,
            Color::Cyan,
        ];
        Self {
            name: "terminal".into(),
            base: Style::default(),
            float: Style::default(),
            float_border: fg(Color::Blue),
            float_title: fg(Color::Blue).add_modifier(Modifier::BOLD),
            separator: fg(Color::DarkGray),
            cursor_line: Style::default(),
            line_nr: fg(Color::DarkGray),
            cursor_line_nr: fg(Color::Blue).add_modifier(Modifier::BOLD),
            visual: Style::default().add_modifier(Modifier::REVERSED),
            comment: fg(Color::DarkGray),
            string: fg(Color::Green),
            key: fg(Color::Red),
            operator: Style::default(),
            title: fg(Color::Blue),
            directory: fg(Color::Blue),
            error: fg(Color::Red),
            warn: fg(Color::Yellow),
            info: fg(Color::Green),
            ok: fg(Color::Green),
            search: block(Color::Yellow),
            pmenu_sel: block(Color::Blue),
            statusline: Style::default(),
            empty_space: fg(Color::DarkGray),
            file: Style::default().add_modifier(Modifier::BOLD),
            file_sep: fg(Color::DarkGray),
            git: fg(Color::DarkGray),
            lsp: fg(Color::Blue),
            cwd_sep: fg(Color::Red),
            cwd_icon: block(Color::Red),
            cwd_text: Style::default(),
            pos_sep: fg(Color::Green),
            pos_icon: block(Color::Green),
            pos_text: fg(Color::Green),
            modes: mode_colors.map(block),
            mode_seps: mode_colors.map(fg),
            ansi: None,
            flat: true,
        }
    }

    fn from_palette(p: &Palette) -> Self {
        let normal = style(p.normal, None);
        let bg = p.normal.bg;
        // Statusline groups that only set fg sit on the StatusLine background, like they do in Neovim.
        let on_status = |h: Hl| style(h, Some(p.statusline.bg));
        Self {
            name: p.name.into(),
            base: normal,
            float: style(p.float, Some(bg)).fg(rgb(p.normal.fg)),
            float_border: style(p.float_border, Some(p.float.bg)),
            float_title: style(p.float_title, None),
            separator: style(p.separator, Some(bg)),
            cursor_line: style(p.cursor_line, None),
            line_nr: style(p.line_nr, None),
            cursor_line_nr: style(p.cursor_line_nr, None),
            visual: style(p.visual, None),
            comment: style(p.comment, None),
            string: style(p.string, None),
            key: style(p.key, None),
            operator: style(p.operator, None),
            title: style(p.title, None),
            directory: style(p.directory, None),
            error: style(p.error, None),
            warn: style(p.warn, None),
            info: style(p.info, None),
            ok: style(p.ok, None),
            search: style(p.search, None),
            pmenu_sel: style(p.pmenu_sel, None),
            statusline: style(p.statusline, None).fg(rgb(p.normal.fg)),
            empty_space: on_status(p.empty_space),
            file: on_status(p.file),
            file_sep: on_status(p.file_sep),
            git: on_status(p.git),
            lsp: on_status(p.lsp),
            cwd_sep: on_status(p.cwd_sep),
            cwd_icon: on_status(p.cwd_icon),
            cwd_text: on_status(p.cwd_text),
            pos_sep: on_status(p.pos_sep),
            pos_icon: on_status(p.pos_icon),
            pos_text: on_status(p.pos_text),
            modes: p.modes.map(on_status),
            mode_seps: p.mode_seps.map(on_status),
            ansi: Some(p.ansi.map(rgb)),
            flat: false,
        }
    }

    // Maps a color reported by the server's terminal onto this theme, the way Neovim's terminal_color_N does.
    pub fn term_color(&self, color: vt100::Color, background: bool) -> Color {
        match color {
            vt100::Color::Default => {
                if background {
                    self.base.bg.unwrap_or(Color::Reset)
                } else {
                    self.base.fg.unwrap_or(Color::Reset)
                }
            }
            vt100::Color::Idx(i) if i < 16 => match &self.ansi {
                Some(ansi) => ansi[usize::from(i)],
                None => Color::Indexed(i),
            },
            vt100::Color::Idx(i) => Color::Indexed(i),
            vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
        }
    }
}

fn rgb(value: u32) -> Color {
    Color::Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn style(h: Hl, fallback_bg: Option<u32>) -> Style {
    let mut s = Style::default();
    if h.fg != N {
        s = s.fg(rgb(h.fg));
    }
    match (h.bg, fallback_bg) {
        (bg, _) if bg != N => s = s.bg(rgb(bg)),
        (_, Some(bg)) if bg != N => s = s.bg(rgb(bg)),
        _ => {}
    }
    if h.bold {
        s = s.add_modifier(Modifier::BOLD);
    }
    s
}
