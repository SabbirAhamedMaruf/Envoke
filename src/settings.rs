use crate::projects::registry::atomic_write;
use anyhow::Result;
use serde::Deserialize;
use std::{fs, path::Path};

pub const FILE: &str = "settings.toml";

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub theme: String,
    pub icons: bool,
    pub separators: String,
    pub terminal_height: u16,
    pub scrollback: usize,
    pub mask_values: bool,
    pub timeout_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "terminal".into(),
            icons: true,
            separators: "default".into(),
            terminal_height: 40,
            scrollback: 5000,
            mask_values: false,
            timeout_ms: 1000,
        }
    }
}

const TEMPLATE: &str = r#"# envoke settings. Delete a line to fall back to its default.

# "terminal" uses your terminal's own colors. Any NvChad base46 theme name works too,
# for example "onedark", "oxocarbon", "catppuccin", "tokyonight". Try them live with :theme <Tab>.
theme = "terminal"

# Nerd Font icons and statusline separators. Set icons = false if your font has no Nerd Font glyphs.
icons = true
# NvChad separator style: "default", "round", "block" or "arrow".
separators = "default"

# Height of the server terminal, in percent of the screen (20-70).
terminal_height = 40
# Lines of server output kept for scrolling back.
scrollback = 5000

# Start with env values hidden behind dots (toggle with zm).
mask_values = false

# How long to wait for the next key of a sequence such as gg or dd, in milliseconds.
timeout_ms = 1000
"#;

impl Settings {
    // Returns the settings plus an optional warning; a broken file never stops the app from opening.
    pub fn load(dir: &Path) -> (Self, Option<String>) {
        let path = dir.join(FILE);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => {
                let warning = write_template(&path)
                    .err()
                    .map(|e| format!("Could not create {FILE}: {e}"));
                return (Self::default(), warning);
            }
        };
        match toml::from_str::<Self>(&text) {
            Ok(mut settings) => {
                settings.terminal_height = settings.terminal_height.clamp(20, 70);
                (settings, None)
            }
            Err(error) => (
                Self::default(),
                Some(format!(
                    "{FILE}: {}; using defaults",
                    error.message().trim()
                )),
            ),
        }
    }
}

fn write_template(path: &Path) -> Result<()> {
    atomic_write(path, TEMPLATE.as_bytes(), 0o644)
}
