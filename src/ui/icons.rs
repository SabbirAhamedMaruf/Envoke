// Nerd Font glyphs (the set NvChad uses) with plain fallbacks when settings.toml turns icons off.
pub struct Icons {
    pub projects: &'static str,
    pub env: &'static str,
    pub file: &'static str,
    pub server: &'static str,
    pub folder: &'static str,
    pub pos: &'static str,
    pub script: &'static str,
    pub help: &'static str,
    pub masked: &'static str,
    pub sep_left: &'static str,
    pub sep_right: &'static str,
}

impl Icons {
    pub fn new(enabled: bool, separators: &str) -> Self {
        if !enabled {
            return Self {
                projects: "",
                env: "",
                file: "",
                server: "",
                folder: "",
                pos: "",
                script: "",
                help: "",
                masked: "*",
                sep_left: "",
                sep_right: "",
            };
        }
        let (sep_left, sep_right) = match separators {
            "round" => ("\u{e0b6}", "\u{e0b4}"),
            "block" => ("\u{2588}", "\u{2588}"),
            "arrow" => ("\u{e0b2}", "\u{e0b0}"),
            "none" => ("", ""),
            _ => ("\u{e0ba}", "\u{e0bc}"),
        };
        Self {
            projects: "\u{f024b} ",
            env: "\u{f084} ",
            file: "\u{f462} ",
            server: "\u{f018d} ",
            folder: "\u{f024b} ",
            pos: "\u{e612} ",
            script: "\u{f0e7} ",
            help: "\u{f02d6} ",
            masked: "\u{f0209}",
            sep_left,
            sep_right,
        }
    }
}
