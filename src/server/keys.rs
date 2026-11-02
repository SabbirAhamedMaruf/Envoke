use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

// Translates a key press into the bytes an xterm would send, so the server sees normal terminal input.
pub fn encode(key: KeyEvent, app_cursor: bool) -> Vec<u8> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let mut bytes = match key.code {
        KeyCode::Char(c) if ctrl => control(c).map_or_else(Vec::new, |b| vec![b]),
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => cursor(b'A', app_cursor, key.modifiers),
        KeyCode::Down => cursor(b'B', app_cursor, key.modifiers),
        KeyCode::Right => cursor(b'C', app_cursor, key.modifiers),
        KeyCode::Left => cursor(b'D', app_cursor, key.modifiers),
        KeyCode::Home => cursor(b'H', app_cursor, key.modifiers),
        KeyCode::End => cursor(b'F', app_cursor, key.modifiers),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::F(n) => function(n),
        _ => vec![],
    };
    if alt && !bytes.is_empty() {
        bytes.insert(0, 0x1b);
    }
    bytes
}

fn control(c: char) -> Option<u8> {
    match c.to_ascii_lowercase() {
        c @ 'a'..='z' => Some(c as u8 - b'a' + 1),
        '@' | ' ' | '2' => Some(0),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '/' | '7' => Some(0x1f),
        '8' | '?' => Some(0x7f),
        _ => None,
    }
}

fn cursor(letter: u8, app_cursor: bool, modifiers: KeyModifiers) -> Vec<u8> {
    let code = modifier_code(modifiers);
    if code > 1 {
        return format!("\x1b[1;{code}{}", letter as char).into_bytes();
    }
    if app_cursor {
        vec![0x1b, b'O', letter]
    } else {
        vec![0x1b, b'[', letter]
    }
}

fn modifier_code(m: KeyModifiers) -> u8 {
    1 + u8::from(m.contains(KeyModifiers::SHIFT))
        + 2 * u8::from(m.contains(KeyModifiers::ALT))
        + 4 * u8::from(m.contains(KeyModifiers::CONTROL))
}

fn function(n: u8) -> Vec<u8> {
    let code = match n {
        1 => return b"\x1bOP".to_vec(),
        2 => return b"\x1bOQ".to_vec(),
        3 => return b"\x1bOR".to_vec(),
        4 => return b"\x1bOS".to_vec(),
        5 => 15,
        6 => 17,
        7 => 18,
        8 => 19,
        9 => 20,
        10 => 21,
        11 => 23,
        12 => 24,
        _ => return vec![],
    };
    format!("\x1b[{code}~").into_bytes()
}
