use crate::projects::discovery::expand_path;
use std::{fs, path::Path};

// Vim's wildmenu: the first Tab fills in the shared prefix and lists the choices, later Tabs walk through them.
#[derive(Default)]
pub struct Completion {
    // Text kept in front of every choice, such as "theme " on the command line.
    pub head: String,
    pub items: Vec<String>,
    pub pick: Option<usize>,
}

impl Completion {
    pub fn active(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    // Starts a completion from fresh choices; returns the text the input should now hold, if it changes.
    pub fn start(&mut self, head: String, word: &str, items: Vec<String>) -> Option<String> {
        self.clear();
        let items: Vec<String> = items.into_iter().filter(|i| i.starts_with(word)).collect();
        match items.len() {
            0 => None,
            1 => Some(format!("{head}{}", items[0])),
            _ => {
                let text = format!("{head}{}", longest_prefix(&items));
                self.head = head;
                self.items = items;
                Some(text)
            }
        }
    }

    pub fn step(&mut self, forward: bool) -> Option<String> {
        let len = self.items.len();
        if len == 0 {
            return None;
        }
        let next = match (self.pick, forward) {
            (None, true) => 0,
            (None, false) => len - 1,
            (Some(i), true) => (i + 1) % len,
            (Some(i), false) => (i + len - 1) % len,
        };
        self.pick = Some(next);
        Some(format!("{}{}", self.head, self.items[next]))
    }
}

fn longest_prefix(items: &[String]) -> String {
    let first = &items[0];
    let mut len = first.len();
    for item in &items[1..] {
        len = first
            .char_indices()
            .zip(item.chars())
            .take_while(|((_, a), b)| a == b)
            .last()
            .map_or(0, |((i, c), _)| i + c.len_utf8())
            .min(len);
    }
    first[..len].to_owned()
}

// Splits a typed path into the folder part kept as the head and the sub-folders that can follow it.
pub fn folders(typed: &str) -> (String, String, Vec<String>) {
    let typed = if typed == "~" { "~/" } else { typed };
    let (head, word) = match typed.rfind('/') {
        Some(i) => (&typed[..=i], &typed[i + 1..]),
        None => ("", typed),
    };
    let dir = if head.is_empty() {
        Path::new(".").to_path_buf()
    } else {
        expand_path(head)
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return (head.into(), word.into(), vec![]);
    };
    let show_hidden = word.starts_with('.');
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| show_hidden || !n.starts_with('.'))
        .map(|n| format!("{n}/"))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    (head.into(), word.into(), names)
}
