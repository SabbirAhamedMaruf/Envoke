use super::complete::{Completion, folders};
use crate::editor::byte_at;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Default, Clone)]
pub struct Input {
    pub text: String,
    pub cursor: usize,
}

impl Input {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.chars().count();
        Self { text, cursor }
    }

    pub fn set(&mut self, text: impl Into<String>) {
        *self = Self::new(text);
    }

    pub fn insert(&mut self, value: &str) {
        for c in value.chars().filter(|c| !c.is_control()) {
            self.text.insert(byte_at(&self.text, self.cursor), c);
            self.cursor += 1;
        }
    }

    // Readline-style editing, as in Vim's command line.
    pub fn key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('u') if ctrl => {
                let end = byte_at(&self.text, self.cursor);
                self.text.replace_range(..end, "");
                self.cursor = 0;
            }
            KeyCode::Char('w') if ctrl => {
                let chars: Vec<char> = self.text.chars().collect();
                let mut start = self.cursor;
                while start > 0 && chars[start - 1] == ' ' {
                    start -= 1;
                }
                while start > 0 && chars[start - 1] != ' ' {
                    start -= 1;
                }
                let (a, b) = (byte_at(&self.text, start), byte_at(&self.text, self.cursor));
                self.text.replace_range(a..b, "");
                self.cursor = start;
            }
            KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
                self.insert(&c.to_string())
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let start = byte_at(&self.text, self.cursor - 1);
                let end = byte_at(&self.text, self.cursor);
                self.text.replace_range(start..end, "");
                self.cursor -= 1;
            }
            KeyCode::Delete => {
                let start = byte_at(&self.text, self.cursor);
                let end = byte_at(&self.text, self.cursor + 1);
                self.text.replace_range(start..end, "");
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.text.chars().count()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.chars().count(),
            _ => {}
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    RemoveProject,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Kind {
    Command,
    NewEnv,
    Confirm(Confirm),
}

pub struct Prompt {
    pub kind: Kind,
    pub label: String,
    pub input: Input,
    pub history: Option<usize>,
    pub completion: Completion,
}

impl Prompt {
    pub fn new(kind: Kind, label: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
            input: Input::new(text),
            history: None,
            completion: Completion::default(),
        }
    }

    pub fn is_confirm(&self) -> bool {
        matches!(self.kind, Kind::Confirm(_))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Folder,
}

// The add / edit project dialog: a name, a folder with Tab completion, and the last error shown inline.
pub struct ProjectForm {
    pub editing: bool,
    pub name: Input,
    pub folder: Input,
    pub field: Field,
    pub completion: Completion,
    pub error: Option<String>,
}

impl ProjectForm {
    pub fn new(editing: bool, name: impl Into<String>, folder: impl Into<String>) -> Self {
        Self {
            editing,
            name: Input::new(name),
            folder: Input::new(folder),
            field: Field::Name,
            completion: Completion::default(),
            error: None,
        }
    }

    pub fn complete(&mut self, forward: bool) {
        let text = if self.completion.active() {
            self.completion.step(forward)
        } else {
            let (head, word, names) = folders(&self.folder.text);
            let text = self.completion.start(head, &word, names);
            if text.is_none() {
                self.error = Some(format!("No folder matches \"{}\"", self.folder.text));
            }
            text
        };
        if let Some(text) = text {
            self.folder.set(text);
        }
    }
}

pub enum Popup {
    Help { scroll: usize },
    Scripts { selected: usize, start_after: bool },
    Project(ProjectForm),
}
