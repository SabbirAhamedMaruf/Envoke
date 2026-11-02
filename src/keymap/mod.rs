pub mod defaults;
pub mod notation;

use crate::projects::registry::atomic_write;
use defaults::DEFAULTS;
use notation::Key;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

pub const FILE: &str = "keymap.toml";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Context {
    Global,
    Projects,
    Env,
    Editor,
    Server,
    Insert,
    Terminal,
}

impl Context {
    pub const ALL: [Context; 7] = [
        Context::Global,
        Context::Projects,
        Context::Env,
        Context::Editor,
        Context::Server,
        Context::Insert,
        Context::Terminal,
    ];

    pub fn section(self) -> &'static str {
        match self {
            Context::Global => "global",
            Context::Projects => "projects",
            Context::Env => "env",
            Context::Editor => "editor",
            Context::Server => "server",
            Context::Insert => "insert",
            Context::Terminal => "terminal",
        }
    }

    fn about(self) -> &'static str {
        match self {
            Context::Global => {
                "Work in NORMAL mode in every pane, unless a pane binds the same key"
            }
            Context::Projects => "Projects pane (left)",
            Context::Env => "Env files pane (middle)",
            Context::Editor => "Preview / editor pane in NORMAL mode",
            Context::Server => "Server terminal pane in NORMAL mode",
            Context::Insert => "Editor in INSERT mode; every other key types text",
            Context::Terminal => {
                "Server terminal in TERMINAL mode; every other key goes to the server"
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    FocusNext,
    FocusPrev,
    FocusLeft,
    FocusDown,
    FocusUp,
    FocusRight,
    CommandLine,
    Help,
    Quit,
    GrowTerminal,
    ShrinkTerminal,
    ToggleMask,
    Down,
    Up,
    Top,
    Bottom,
    HalfPageDown,
    HalfPageUp,
    Open,
    AddProject,
    EditProject,
    RemoveProject,
    PickScript,
    Rescan,
    Start,
    Back,
    Preview,
    NewEnv,
    Restart,
    Stop,
    Left,
    Right,
    WordNext,
    WordPrev,
    WordEnd,
    LineStart,
    FirstNonBlank,
    LineEnd,
    Insert,
    Append,
    InsertLineStart,
    AppendLineEnd,
    OpenBelow,
    OpenAbove,
    AddKey,
    DeleteChar,
    DeleteLine,
    DeleteToEnd,
    YankLine,
    PasteAfter,
    PasteBefore,
    Undo,
    Redo,
    Save,
    TerminalInsert,
    NormalMode,
}

pub struct Binding {
    pub keys: Vec<Key>,
    pub action: Action,
}

pub enum Lookup {
    Fire(Action),
    Pending,
    Miss,
}

pub struct Keymap {
    maps: HashMap<Context, Vec<Binding>>,
    pub path: PathBuf,
}

impl Keymap {
    // Built-in keys first, then keymap.toml on top; problems become warnings instead of a failed start.
    pub fn load(dir: &Path) -> (Self, Vec<String>) {
        let path = dir.join(FILE);
        let mut warnings = vec![];
        let mut chosen: HashMap<(Context, &'static str), Vec<String>> = DEFAULTS
            .iter()
            .map(|d| {
                (
                    (d.ctx, d.name),
                    d.keys.iter().map(|k| k.to_string()).collect(),
                )
            })
            .collect();
        match fs::read_to_string(&path) {
            Ok(text) => match text.parse::<toml::Table>() {
                Ok(table) => apply_file(&table, &mut chosen, &mut warnings),
                Err(error) => warnings.push(format!("{FILE}: {}", error.message().trim())),
            },
            Err(_) => {
                if let Err(error) = atomic_write(&path, template().as_bytes(), 0o644) {
                    warnings.push(format!("Could not create {FILE}: {error}"));
                }
            }
        }
        let mut maps: HashMap<Context, Vec<Binding>> = HashMap::new();
        for def in DEFAULTS {
            for text in &chosen[&(def.ctx, def.name)] {
                match notation::parse(text) {
                    Ok(keys) => maps.entry(def.ctx).or_default().push(Binding {
                        keys,
                        action: def.action,
                    }),
                    Err(error) => warnings.push(format!(
                        "{FILE} [{}] {}: {error}",
                        def.ctx.section(),
                        def.name
                    )),
                }
            }
        }
        (Self { maps, path }, warnings)
    }

    // Contexts are searched in priority order; a longer binding that starts with the typed keys keeps waiting.
    pub fn lookup(&self, contexts: &[Context], typed: &[Key]) -> Lookup {
        let mut exact = None;
        let mut longer = false;
        for ctx in contexts {
            for binding in self.maps.get(ctx).into_iter().flatten() {
                if binding.keys == typed {
                    exact = exact.or(Some(binding.action));
                } else if binding.keys.starts_with(typed) {
                    longer = true;
                }
            }
        }
        match (exact, longer) {
            (_, true) => Lookup::Pending,
            (Some(action), false) => Lookup::Fire(action),
            (None, false) => Lookup::Miss,
        }
    }

    pub fn exact(&self, contexts: &[Context], typed: &[Key]) -> Option<Action> {
        contexts.iter().find_map(|ctx| {
            self.maps
                .get(ctx)?
                .iter()
                .find(|b| b.keys == typed)
                .map(|b| b.action)
        })
    }

    // Help rows: (section, keys, description), using the keys actually in effect.
    pub fn help(&self) -> Vec<(&'static str, String, &'static str)> {
        let mut rows = vec![];
        for ctx in Context::ALL {
            for def in DEFAULTS.iter().filter(|d| d.ctx == ctx) {
                let keys: Vec<String> = self
                    .maps
                    .get(&ctx)
                    .into_iter()
                    .flatten()
                    .filter(|b| b.action == def.action)
                    .map(|b| notation::display(&b.keys))
                    .collect();
                if !keys.is_empty() {
                    rows.push((ctx.section(), keys.join(" "), def.desc));
                }
            }
        }
        rows
    }
}

fn apply_file(
    table: &toml::Table,
    chosen: &mut HashMap<(Context, &'static str), Vec<String>>,
    warnings: &mut Vec<String>,
) {
    for (section, value) in table {
        let Some(ctx) = Context::ALL.into_iter().find(|c| c.section() == section) else {
            warnings.push(format!("{FILE}: unknown section [{section}]"));
            continue;
        };
        let Some(actions) = value.as_table() else {
            warnings.push(format!("{FILE}: [{section}] must be a table"));
            continue;
        };
        for (name, keys) in actions {
            let Some(def) = DEFAULTS.iter().find(|d| d.ctx == ctx && d.name == name) else {
                warnings.push(format!("{FILE}: [{section}] has no action '{name}'"));
                continue;
            };
            let list = match keys {
                toml::Value::String(s) => Some(vec![s.clone()]),
                toml::Value::Array(items) => items
                    .iter()
                    .map(|i| i.as_str().map(str::to_owned))
                    .collect(),
                _ => None,
            };
            match list {
                Some(list) => {
                    chosen.insert((ctx, def.name), list);
                }
                None => warnings.push(format!(
                    "{FILE}: [{section}] {name} must be a key or a list of keys"
                )),
            }
        }
    }
}

fn quote(key: &str) -> String {
    if key.contains('\'') {
        format!("\"{}\"", key.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        format!("'{key}'")
    }
}

pub fn template() -> String {
    let mut out = String::from(
        "# envoke keymap, in Vim key notation.\n\
         #   <C-s> Ctrl+s   <A-x> Alt+x   <S-Tab> Shift+Tab   <CR> Enter   <Esc>   <BS>   <Del>   <Space>\n\
         #   <Up> <Down> <Left> <Right> <Home> <End> <PageUp> <PageDown> <F1>..<F12>   <lt> for '<'\n\
         #   Sequences work like Vim: 'gg', 'dd', '<C-w>+', '<C-\\><C-n>'.\n\
         # Give an action one key or a list of keys. Delete a line to get its default back; [] unbinds it.\n\
         # A count works before list and editor keys, for example 3j or 2dd.\n",
    );
    for ctx in Context::ALL {
        out.push_str(&format!("\n# {}\n[{}]\n", ctx.about(), ctx.section()));
        for def in DEFAULTS.iter().filter(|d| d.ctx == ctx) {
            let keys: Vec<String> = def.keys.iter().map(|k| quote(k)).collect();
            let value = if keys.len() == 1 {
                keys[0].clone()
            } else {
                format!("[{}]", keys.join(", "))
            };
            out.push_str(&format!("{} = {}  # {}\n", def.name, value, def.desc));
        }
    }
    out
}
