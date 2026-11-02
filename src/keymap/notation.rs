// Vim key notation: "gg", "<C-s>", "<CR>", "<S-Tab>", "<C-\><C-n>".
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

const MODS: KeyModifiers = KeyModifiers::SHIFT
    .union(KeyModifiers::CONTROL)
    .union(KeyModifiers::ALT);

impl Key {
    pub fn plain(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            mods: KeyModifiers::NONE,
        }
    }

    // Brings a terminal key event into the same shape the parser produces, so lookups compare equal.
    pub fn from_event(event: KeyEvent) -> Self {
        let mut mods = event.modifiers & MODS;
        let code = match event.code {
            KeyCode::Char(c) => {
                mods.remove(KeyModifiers::SHIFT);
                if mods.contains(KeyModifiers::CONTROL) {
                    // Without the kitty protocol, terminals report Ctrl+\ ] ^ _ as Ctrl+4..7.
                    KeyCode::Char(match c {
                        '4' => '\\',
                        '5' => ']',
                        '6' => '^',
                        '7' => '_',
                        c => c.to_ascii_lowercase(),
                    })
                } else {
                    KeyCode::Char(c)
                }
            }
            KeyCode::BackTab => {
                mods.remove(KeyModifiers::SHIFT);
                KeyCode::BackTab
            }
            code => code,
        };
        Self { code, mods }
    }

    pub fn typed_char(&self) -> Option<char> {
        match self.code {
            KeyCode::Char(c)
                if !self
                    .mods
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                Some(c)
            }
            _ => None,
        }
    }

    pub fn event(&self) -> KeyEvent {
        KeyEvent::new(self.code, self.mods)
    }
}

pub fn parse(text: &str) -> Result<Vec<Key>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut keys = vec![];
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<'
            && let Some(len) = chars[i + 1..].iter().position(|&c| c == '>')
            && len > 0
        {
            let inner: String = chars[i + 1..i + 1 + len].iter().collect();
            keys.push(special(&inner).ok_or_else(|| format!("unknown key <{inner}>"))?);
            i += len + 2;
            continue;
        }
        keys.push(Key::plain(chars[i]));
        i += 1;
    }
    if keys.is_empty() {
        return Err("empty key".into());
    }
    Ok(keys)
}

fn special(inner: &str) -> Option<Key> {
    let mut mods = KeyModifiers::NONE;
    let mut rest = inner;
    while rest.len() > 2 && rest.as_bytes()[1] == b'-' {
        match rest.as_bytes()[0].to_ascii_uppercase() {
            b'C' => mods |= KeyModifiers::CONTROL,
            b'A' | b'M' => mods |= KeyModifiers::ALT,
            b'S' => mods |= KeyModifiers::SHIFT,
            _ => return None,
        }
        rest = &rest[2..];
    }
    let code = match rest.to_ascii_lowercase().as_str() {
        "cr" | "enter" | "return" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "tab" if mods.contains(KeyModifiers::SHIFT) => {
            mods.remove(KeyModifiers::SHIFT);
            KeyCode::BackTab
        }
        "tab" => KeyCode::Tab,
        "bs" | "backspace" => KeyCode::Backspace,
        "del" | "delete" => KeyCode::Delete,
        "space" => KeyCode::Char(' '),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "insert" => KeyCode::Insert,
        "lt" => KeyCode::Char('<'),
        "bslash" => KeyCode::Char('\\'),
        "bar" => KeyCode::Char('|'),
        name if name.len() > 1 && name.starts_with('f') => KeyCode::F(name[1..].parse().ok()?),
        _ => {
            let mut chars = rest.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            if mods.contains(KeyModifiers::SHIFT) {
                mods.remove(KeyModifiers::SHIFT);
                KeyCode::Char(c.to_ascii_uppercase())
            } else if mods.contains(KeyModifiers::CONTROL) {
                KeyCode::Char(c.to_ascii_lowercase())
            } else {
                KeyCode::Char(c)
            }
        }
    };
    Some(Key { code, mods })
}

pub fn display(keys: &[Key]) -> String {
    keys.iter().map(display_one).collect()
}

fn display_one(key: &Key) -> String {
    let name = match key.code {
        KeyCode::Char('<') => "lt".into(),
        KeyCode::Char(' ') => "Space".into(),
        KeyCode::Char(c) if key.mods.is_empty() => return c.to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "CR".into(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::BackTab => "S-Tab".into(),
        KeyCode::Backspace => "BS".into(),
        KeyCode::Delete => "Del".into(),
        KeyCode::Up => "Up".into(),
        KeyCode::Down => "Down".into(),
        KeyCode::Left => "Left".into(),
        KeyCode::Right => "Right".into(),
        KeyCode::Home => "Home".into(),
        KeyCode::End => "End".into(),
        KeyCode::PageUp => "PageUp".into(),
        KeyCode::PageDown => "PageDown".into(),
        KeyCode::Insert => "Insert".into(),
        KeyCode::F(n) => format!("F{n}"),
        _ => "?".into(),
    };
    let mut prefix = String::new();
    if key.mods.contains(KeyModifiers::CONTROL) {
        prefix.push_str("C-");
    }
    if key.mods.contains(KeyModifiers::ALT) {
        prefix.push_str("A-");
    }
    if key.mods.contains(KeyModifiers::SHIFT) {
        prefix.push_str("S-");
    }
    format!("<{prefix}{name}>")
}
