use super::{
    App, Pane,
    prompt::{Popup, Prompt},
};
use crate::{keymap::Action, theme::Theme};
use anyhow::{Result, bail};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Write,
    Quit,
    QuitForce,
    WriteQuit,
    Edit,
    EditForce,
    Run,
    Restart,
    Stop,
    Script,
    Rescan,
    New,
    AddProject,
    EditProject,
    RemoveProject,
    Theme,
    Mask,
    Keys,
    Help,
}

// What Tab offers after the command name.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    None,
    EnvFile,
    Script,
    Theme,
}

pub struct Spec {
    pub command: Command,
    pub long: &'static str,
    pub short: &'static str,
    // Vim spellings that keep working without being advertised.
    pub extra: &'static [&'static str],
    pub arg: Arg,
    pub usage: &'static str,
    pub desc: &'static str,
}

const fn spec(
    command: Command,
    long: &'static str,
    short: &'static str,
    extra: &'static [&'static str],
    arg: Arg,
    usage: &'static str,
    desc: &'static str,
) -> Spec {
    Spec {
        command,
        long,
        short,
        extra,
        arg,
        usage,
        desc,
    }
}

use Arg as R;
use Command as C;

// Every ':' command with its long and short name; dispatch, Tab completion and the help popup all read this table.
pub const COMMANDS: &[Spec] = &[
    spec(
        C::Write,
        "write",
        "w",
        &["w!", "write!"],
        R::None,
        "",
        "Save the open env file (restarts its server)",
    ),
    spec(
        C::Quit,
        "quit",
        "q",
        &["qa"],
        R::None,
        "",
        "Quit and stop the server",
    ),
    spec(
        C::QuitForce,
        "quit!",
        "q!",
        &["qa!"],
        R::None,
        "",
        "Quit and throw away unsaved edits",
    ),
    spec(
        C::WriteQuit,
        "writequit",
        "wq",
        &["x", "xit", "wqa", "xa"],
        R::None,
        "",
        "Save if changed, then quit",
    ),
    spec(
        C::Edit,
        "edit",
        "e",
        &[],
        R::EnvFile,
        "[file]",
        "Open an env file; with no name, reload this one",
    ),
    spec(
        C::EditForce,
        "edit!",
        "e!",
        &[],
        R::None,
        "",
        "Throw away edits and reload from disk",
    ),
    spec(
        C::Run,
        "run",
        "r",
        &["start"],
        R::EnvFile,
        "[file]",
        "Run the server with the highlighted (or named) env file",
    ),
    spec(
        C::Restart,
        "restart",
        "rs",
        &[],
        R::None,
        "",
        "Restart the running server",
    ),
    spec(C::Stop, "stop", "st", &[], R::None, "", "Stop the server"),
    spec(
        C::Script,
        "script",
        "sc",
        &[],
        R::Script,
        "[name]",
        "Choose the run script (list, or by name)",
    ),
    spec(
        C::Rescan,
        "rescan",
        "rsc",
        &[],
        R::None,
        "",
        "Rescan env files and scripts",
    ),
    spec(
        C::New,
        "new",
        "n",
        &[],
        R::None,
        "[file]",
        "Create an env file in the project folder",
    ),
    spec(
        C::AddProject,
        "add-project",
        "ap",
        &[],
        R::None,
        "",
        "Open the add project dialog",
    ),
    spec(
        C::EditProject,
        "edit-project",
        "ep",
        &[],
        R::None,
        "",
        "Rename or move the selected project",
    ),
    spec(
        C::RemoveProject,
        "remove-project",
        "rp",
        &[],
        R::None,
        "",
        "Remove the project from the list (files are kept)",
    ),
    spec(
        C::Theme,
        "theme",
        "th",
        &["colorscheme", "colo"],
        R::Theme,
        "[name]",
        "Show or switch the theme",
    ),
    spec(
        C::Mask,
        "mask",
        "m",
        &[],
        R::None,
        "",
        "Hide / show env values",
    ),
    spec(
        C::Keys,
        "keys",
        "k",
        &["keymap"],
        R::None,
        "",
        "Show where keymap.toml lives",
    ),
    spec(
        C::Help,
        "help",
        "h",
        &[],
        R::None,
        "",
        "List every key and command",
    ),
];

pub fn resolve(name: &str) -> Option<&'static Spec> {
    COMMANDS
        .iter()
        .find(|s| s.long == name || s.short == name || s.extra.contains(&name))
}

impl App {
    pub fn run_command(&mut self, line: &str) -> Result<()> {
        let line = line.trim_start_matches(':').trim();
        let (name, arg) = line
            .split_once(char::is_whitespace)
            .map_or((line, ""), |(n, a)| (n, a.trim()));
        if name.is_empty() {
            return Ok(());
        }
        if let Ok(number) = name.parse::<usize>() {
            self.focus(Pane::Editor);
            self.buffer.row = number.saturating_sub(1);
            self.buffer.clamp(false);
            return Ok(());
        }
        // The pre-1.0 spelling :project add|edit|remove keeps working.
        if name == "project" {
            let action = match arg {
                "add" | "" => Action::AddProject,
                "edit" => Action::EditProject,
                "remove" => Action::RemoveProject,
                _ => bail!("Use :add-project, :edit-project or :remove-project"),
            };
            return self.perform(action, 1);
        }
        let Some(spec) = resolve(name) else {
            bail!("E492: Not an editor command: {line}");
        };
        match spec.command {
            C::Write => self.save_buffer()?,
            C::Quit => self.request_quit(false),
            C::QuitForce => self.request_quit(true),
            C::WriteQuit => {
                if self.buffer.dirty() {
                    self.save_buffer()?;
                }
                self.request_quit(false);
            }
            C::Edit if !arg.is_empty() => self.open_env(arg)?,
            C::Edit | C::EditForce => self.discard()?,
            C::Run => {
                if !arg.is_empty() {
                    self.open_env(arg)?;
                }
                self.start()?;
            }
            C::Restart => self.restart()?,
            C::Stop => self.stop()?,
            C::Script if arg.is_empty() => self.perform(Action::PickScript, 1)?,
            C::Script => {
                let index = self
                    .selected()
                    .and_then(|p| p.scripts.iter().position(|s| s.name == arg));
                match index {
                    Some(index) => self.choose_script(index, false)?,
                    None => bail!("No script named '{arg}' in this project"),
                }
            }
            C::Rescan => self.rescan()?,
            C::New if arg.is_empty() => self.perform(Action::NewEnv, 1)?,
            C::New => self.create_env(arg)?,
            C::AddProject => self.perform(Action::AddProject, 1)?,
            C::EditProject => self.perform(Action::EditProject, 1)?,
            C::RemoveProject => self.perform(Action::RemoveProject, 1)?,
            C::Theme if arg.is_empty() => self.info(format!("theme {}", self.theme.name)),
            C::Theme => match Theme::by_name(arg) {
                Some(theme) => {
                    self.theme = theme;
                    self.info(format!(
                        "theme {arg} · put theme = \"{arg}\" in settings.toml to keep it"
                    ));
                }
                None => bail!("E185: Cannot find theme '{arg}' (Tab lists them)"),
            },
            C::Mask => self.masked = !self.masked,
            C::Keys => {
                let path = super::input::tilde(&self.keymap.path);
                self.info(format!("Keys live in {path} · edit it and restart envoke"));
            }
            C::Help => self.popup = Some(Popup::Help { scroll: 0 }),
        }
        Ok(())
    }

    fn open_env(&mut self, name: &str) -> Result<()> {
        let index = self
            .selected()
            .and_then(|p| p.env_files.iter().position(|f| f.to_string_lossy() == name));
        match index {
            Some(index) => {
                self.select_profile(index)?;
                self.focus(Pane::Editor);
                Ok(())
            }
            None => bail!("No env file '{name}' in this project (:new {name} creates it)"),
        }
    }

    // Tab on the command line: command names first, then the argument the command takes.
    pub fn complete(&mut self, prompt: &mut Prompt, forward: bool) {
        let text = if prompt.completion.active() {
            prompt.completion.step(forward)
        } else {
            let typed = prompt.input.text.clone();
            let (head, word, choices) = match typed.split_once(' ') {
                None => (
                    String::new(),
                    typed.clone(),
                    COMMANDS.iter().map(|s| s.long.to_owned()).collect(),
                ),
                Some((cmd, word)) => (format!("{cmd} "), word.to_owned(), self.arguments(cmd)),
            };
            prompt.completion.start(head, &word, choices)
        };
        if let Some(text) = text {
            prompt.input.set(text);
        }
    }

    fn arguments(&self, command: &str) -> Vec<String> {
        if command == "project" {
            return ["add", "edit", "remove"].map(String::from).to_vec();
        }
        match resolve(command).map(|s| s.arg) {
            Some(Arg::Theme) => Theme::names().map(str::to_owned).collect(),
            Some(Arg::EnvFile) => self.selected().map_or(vec![], |p| {
                p.env_files
                    .iter()
                    .map(|f| f.to_string_lossy().into_owned())
                    .collect()
            }),
            Some(Arg::Script) => self.selected().map_or(vec![], |p| {
                p.scripts.iter().map(|s| s.name.clone()).collect()
            }),
            _ => vec![],
        }
    }
}
