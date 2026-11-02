pub mod view;
pub mod vim;

use crate::{
    profiles::{Stamp, parse_env},
    projects::registry::atomic_write,
};
use anyhow::{Context, Result, bail};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

const MAX_FILE: u64 = 1024 * 1024;
const MAX_UNDO: usize = 200;

#[derive(Clone)]
pub enum Register {
    Lines(Vec<String>),
    Text(String),
}

#[derive(Clone)]
struct Snapshot {
    lines: Vec<String>,
    row: usize,
    col: usize,
}

#[derive(Default)]
pub struct Buffer {
    pub path: Option<PathBuf>,
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
    pub scroll: usize,
    pub hscroll: usize,
    pub register: Option<Register>,
    original: String,
    crlf: bool,
    stamp: Option<Stamp>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Buffer {
    pub fn load(path: PathBuf) -> Result<Self> {
        let metadata = fs::symlink_metadata(&path).context("Cannot read env file")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!("Env file must be a regular file, not a symlink");
        }
        if metadata.len() > MAX_FILE {
            bail!("Env file exceeds the 1 MiB editor limit");
        }
        let original = fs::read_to_string(&path).context("Env file must contain UTF-8 text")?;
        let crlf = original.contains("\r\n");
        let lines = original
            .replace("\r\n", "\n")
            .split('\n')
            .map(str::to_owned)
            .collect();
        Ok(Self {
            stamp: Stamp::of(&path),
            path: Some(path),
            lines,
            original,
            crlf,
            ..Self::default()
        })
    }

    // Like Neovim's autoread: an unmodified buffer follows edits made by other programs.
    pub fn reload_if_changed(&mut self) -> Result<bool> {
        let Some(path) = self.path.clone() else {
            return Ok(false);
        };
        if self.dirty() || Stamp::of(&path) == self.stamp {
            return Ok(false);
        }
        let (row, col, scroll) = (self.row, self.col, self.scroll);
        let register = self.register.take();
        *self = Self::load(path)?;
        (self.row, self.col, self.scroll, self.register) = (row, col, scroll, register);
        self.clamp(false);
        Ok(true)
    }

    pub fn text(&self) -> String {
        self.lines.join(if self.crlf { "\r\n" } else { "\n" })
    }

    pub fn dirty(&self) -> bool {
        self.path.is_some() && self.text() != self.original
    }

    pub fn name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn save(&mut self) -> Result<()> {
        let path = self.path.clone().context("Select an env file first")?;
        let text = self.text();
        parse_env(&text)?;
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            bail!("Env file became a symlink; save cancelled");
        }
        if fs::read_to_string(&path)? != self.original {
            bail!("File changed on disk since it was opened. Copy your edits, then :e! to reload.");
        }
        atomic_write(&path, text.as_bytes(), metadata.permissions().mode())?;
        self.original = text;
        self.stamp = Stamp::of(&path);
        Ok(())
    }

    pub fn line_len(&self, row: usize) -> usize {
        self.lines.get(row).map_or(0, |l| l.chars().count())
    }

    // Normal mode keeps the cursor on a character; insert mode may sit just past the end of the line.
    pub fn clamp(&mut self, insert: bool) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.row = self.row.min(self.lines.len() - 1);
        let len = self.line_len(self.row);
        let max = if insert { len } else { len.saturating_sub(1) };
        self.col = self.col.min(max);
    }

    pub fn snapshot(&mut self) {
        if self.undo.len() >= MAX_UNDO {
            self.undo.remove(0);
        }
        self.undo.push(Snapshot {
            lines: self.lines.clone(),
            row: self.row,
            col: self.col,
        });
        self.redo.clear();
    }

    // An insert session that typed nothing should not leave an empty undo step behind.
    pub fn drop_empty_snapshot(&mut self) {
        if self.undo.last().is_some_and(|s| s.lines == self.lines) {
            self.undo.pop();
        }
    }

    pub fn undo(&mut self) -> bool {
        self.swap_history(true)
    }

    pub fn redo(&mut self) -> bool {
        self.swap_history(false)
    }

    fn swap_history(&mut self, undo: bool) -> bool {
        let (from, to) = if undo {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(snapshot) = from.pop() else {
            return false;
        };
        to.push(Snapshot {
            lines: std::mem::replace(&mut self.lines, snapshot.lines),
            row: self.row,
            col: self.col,
        });
        self.row = snapshot.row;
        self.col = snapshot.col;
        self.clamp(false);
        true
    }

    pub fn insert_str(&mut self, value: &str) {
        self.clamp(true);
        let byte = byte_at(&self.lines[self.row], self.col);
        let suffix = self.lines[self.row].split_off(byte);
        let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
        let mut parts = normalized.split('\n');
        let first = parts.next().unwrap_or("");
        self.lines[self.row].push_str(first);
        self.col += first.chars().count();
        for part in parts {
            self.row += 1;
            self.lines.insert(self.row, part.into());
            self.col = part.chars().count();
        }
        self.lines[self.row].push_str(&suffix);
    }

    pub fn backspace(&mut self) {
        self.clamp(true);
        if self.col > 0 {
            let end = byte_at(&self.lines[self.row], self.col);
            let start = byte_at(&self.lines[self.row], self.col - 1);
            self.lines[self.row].replace_range(start..end, "");
            self.col -= 1;
        } else if self.row > 0 {
            let line = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.line_len(self.row);
            self.lines[self.row].push_str(&line);
        }
    }

    pub fn delete_forward(&mut self) {
        self.clamp(true);
        if self.col < self.line_len(self.row) {
            let start = byte_at(&self.lines[self.row], self.col);
            let end = byte_at(&self.lines[self.row], self.col + 1);
            self.lines[self.row].replace_range(start..end, "");
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        }
    }

    pub fn delete_chars(&mut self, count: usize) {
        self.clamp(false);
        let len = self.line_len(self.row);
        if len == 0 {
            return;
        }
        self.snapshot();
        let end_col = (self.col + count).min(len);
        let line = &mut self.lines[self.row];
        let (start, end) = (byte_at(line, self.col), byte_at(line, end_col));
        self.register = Some(Register::Text(line[start..end].to_owned()));
        line.replace_range(start..end, "");
        self.clamp(false);
    }

    pub fn delete_to_end(&mut self) {
        self.clamp(false);
        if self.line_len(self.row) == 0 {
            return;
        }
        self.snapshot();
        let line = &mut self.lines[self.row];
        let start = byte_at(line, self.col);
        self.register = Some(Register::Text(line[start..].to_owned()));
        line.truncate(start);
        self.clamp(false);
    }

    pub fn delete_lines(&mut self, count: usize) {
        self.snapshot();
        self.clamp(false);
        let end = (self.row + count).min(self.lines.len());
        let removed: Vec<String> = self.lines.drain(self.row..end).collect();
        self.register = Some(Register::Lines(removed));
        self.col = 0;
        self.clamp(false);
        self.col = vim::first_non_blank(&self.lines[self.row]);
    }

    pub fn yank_lines(&mut self, count: usize) -> usize {
        self.clamp(false);
        let end = (self.row + count).min(self.lines.len());
        self.register = Some(Register::Lines(self.lines[self.row..end].to_vec()));
        end - self.row
    }

    pub fn paste(&mut self, after: bool, count: usize) -> bool {
        let Some(register) = self.register.clone() else {
            return false;
        };
        self.snapshot();
        self.clamp(false);
        match register {
            Register::Lines(lines) => {
                let at = if after { self.row + 1 } else { self.row };
                let block: Vec<String> = (0..count).flat_map(|_| lines.clone()).collect();
                self.lines.splice(at..at, block);
                self.row = at;
                self.col = vim::first_non_blank(&self.lines[self.row]);
            }
            Register::Text(text) => {
                let text = text.repeat(count);
                if after && self.line_len(self.row) > 0 {
                    self.col += 1;
                }
                self.insert_str(&text);
                self.col = self.col.saturating_sub(1);
            }
        }
        self.clamp(false);
        true
    }

    pub fn open_line(&mut self, below: bool) {
        self.clamp(false);
        let at = if below { self.row + 1 } else { self.row };
        self.lines.insert(at, String::new());
        self.row = at;
        self.col = 0;
    }

    // Appends a NEW_KEY= line and leaves the cursor on the key, ready to be renamed.
    pub fn add_key(&mut self) {
        self.snapshot();
        if self.lines.last().is_some_and(|s| s.is_empty()) {
            self.row = self.lines.len() - 1;
            self.lines[self.row] = "NEW_KEY=".into();
        } else {
            self.lines.push("NEW_KEY=".into());
            self.row = self.lines.len() - 1;
        }
        self.col = 0;
    }
}

pub fn byte_at(text: &str, column: usize) -> usize {
    text.char_indices()
        .nth(column)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}
