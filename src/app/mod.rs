pub mod actions;
pub mod commands;
pub mod complete;
pub mod input;
pub mod prompt;

use crate::{
    editor::Buffer,
    keymap::{Context, Keymap, notation::Key},
    profiles::{Stamp, parse_env},
    projects::{
        Config, Project,
        discovery::{self, is_env_name},
        registry::Store,
    },
    server::Server,
    settings::Settings,
    theme::{Mode, Theme},
    ui::icons::Icons,
};
use anyhow::{Context as _, Result, bail};
use prompt::{Popup, ProjectForm, Prompt};
use ratatui::layout::Rect;
use std::{
    fs,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const WATCH_EVERY: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pane {
    Projects,
    Env,
    Editor,
    Server,
}

impl Pane {
    pub fn context(self) -> Context {
        match self {
            Pane::Projects => Context::Projects,
            Pane::Env => Context::Env,
            Pane::Editor => Context::Editor,
            Pane::Server => Context::Server,
        }
    }
}

// Screen rectangles from the last draw, used to map mouse clicks back to rows.
#[derive(Default, Clone, Copy)]
pub struct Areas {
    pub projects: Rect,
    pub env: Rect,
    pub editor: Rect,
    pub server: Rect,
}

pub struct Message {
    pub text: String,
    pub error: bool,
}

pub struct App {
    pub store: Store,
    pub config: Config,
    pub settings: Settings,
    pub theme: Theme,
    pub icons: Icons,
    pub keymap: Keymap,
    pub project: usize,
    pub profile: usize,
    pub project_offset: usize,
    pub profile_offset: usize,
    pub pane: Pane,
    // The top pane to return to when leaving the server terminal upwards.
    pub last_top: Pane,
    pub insert: bool,
    pub terminal_mode: bool,
    pub buffer: Buffer,
    pub server: Server,
    pub prompt: Option<Prompt>,
    pub popup: Option<Popup>,
    pub message: Option<Message>,
    pub masked: bool,
    pub term_height: u16,
    pub quit: bool,
    pub areas: Areas,
    pub pending: Vec<Key>,
    pub count: Option<usize>,
    pub history: Vec<String>,
    pub needs_draw: bool,
    pending_at: Instant,
    watched: Option<Watched>,
    last_watch: Instant,
    last_click: Option<(u16, u16, Instant)>,
    last_second: u64,
}

// The env file the running server was started with, to notice when another program edits it.
struct Watched {
    path: PathBuf,
    stamp: Option<Stamp>,
    text: String,
}

impl App {
    pub fn new(store: Store, mut config: Config) -> Result<Self> {
        let (settings, settings_warning) = Settings::load(&store.dir);
        let (keymap, key_warnings) = Keymap::load(&store.dir);
        let (theme, theme_warning) = match Theme::by_name(&settings.theme) {
            Some(theme) => (theme, None),
            None => (
                Theme::terminal(),
                Some(format!(
                    "Unknown theme '{}'; using terminal colors",
                    settings.theme
                )),
            ),
        };
        let mut failed = 0;
        for project in &mut config.projects {
            if discovery::refresh(project).is_err() {
                failed += 1;
            }
        }
        store.save(&config)?;
        let empty = config.projects.is_empty();
        let mut app = Self {
            icons: Icons::new(settings.icons, &settings.separators),
            server: Server::new(settings.scrollback),
            masked: settings.mask_values,
            term_height: settings.terminal_height,
            store,
            config,
            settings,
            theme,
            keymap,
            project: 0,
            profile: 0,
            project_offset: 0,
            profile_offset: 0,
            pane: Pane::Projects,
            last_top: Pane::Projects,
            insert: false,
            terminal_mode: false,
            buffer: Buffer::default(),
            prompt: None,
            popup: None,
            message: None,
            quit: false,
            areas: Areas::default(),
            pending: vec![],
            count: None,
            history: vec![],
            needs_draw: true,
            pending_at: Instant::now(),
            watched: None,
            last_watch: Instant::now(),
            last_click: None,
            last_second: 0,
        };
        if empty {
            app.popup = Some(Popup::Project(ProjectForm::new(false, "", "~/")));
        } else if let Err(error) = app.load_project() {
            app.fail(error);
        }
        let warning = settings_warning
            .or(theme_warning)
            .or_else(|| key_warnings.first().cloned());
        if let Some(warning) = warning {
            app.error(warning);
        } else if failed > 0 {
            app.error(format!(
                "{failed} project(s) could not be rescanned. Check the folder with e or R."
            ));
        } else if !empty {
            app.info("Enter runs the highlighted env file · ? shows every key · :q quits");
        }
        Ok(app)
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            error: false,
        });
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            error: true,
        });
    }

    pub fn fail(&mut self, error: anyhow::Error) {
        self.error(format!("{error:#}"));
    }

    pub fn mode(&self) -> Mode {
        match &self.prompt {
            Some(p) if p.is_confirm() => Mode::Confirm,
            Some(_) => Mode::Command,
            None if self.terminal_mode => Mode::Terminal,
            None if self.insert => Mode::Insert,
            None if self.pane == Pane::Server => Mode::TerminalNormal,
            None => Mode::Normal,
        }
    }

    pub fn selected(&self) -> Option<&Project> {
        self.config.projects.get(self.project)
    }

    pub fn profile_relative(&self) -> Option<PathBuf> {
        self.selected()?.env_files.get(self.profile).cloned()
    }

    pub fn profile_path(&self) -> Option<PathBuf> {
        let project = self.selected()?;
        project
            .env_files
            .get(self.profile)
            .map(|e| project.path.join(e))
    }

    pub fn focus(&mut self, pane: Pane) {
        if pane != Pane::Server {
            self.last_top = pane;
        }
        self.pane = pane;
        self.insert = false;
        self.terminal_mode = false;
    }

    pub fn ensure_clean(&self) -> Result<()> {
        if self.buffer.dirty() {
            bail!(
                "E37: No write since last change in {} (:w to save, :e! to discard)",
                self.buffer.name()
            );
        }
        Ok(())
    }

    // Saves a changed copy of the registry first and only then swaps it in, so a failed write changes nothing.
    pub fn update_config(&mut self, change: impl FnOnce(&mut Config) -> Result<()>) -> Result<()> {
        let mut next = self.config.clone();
        change(&mut next)?;
        self.store.save(&next)?;
        self.config = next;
        Ok(())
    }

    pub fn load_project(&mut self) -> Result<()> {
        self.profile_offset = 0;
        self.profile = self
            .selected()
            .and_then(|p| {
                p.env_files
                    .iter()
                    .position(|f| Some(f) == p.selected_env.as_ref())
            })
            .unwrap_or(0);
        self.load_profile()
    }

    pub fn load_profile(&mut self) -> Result<()> {
        self.buffer = Buffer::default();
        self.insert = false;
        if let Some(path) = self.profile_path() {
            self.buffer = Buffer::load(path)?;
        }
        Ok(())
    }

    pub fn select_project(&mut self, index: usize) -> Result<()> {
        if index >= self.config.projects.len() || index == self.project {
            return Ok(());
        }
        self.ensure_clean()?;
        self.project = index;
        self.load_project()
    }

    // Moving through env files only previews them; Enter is what starts a server.
    pub fn select_profile(&mut self, index: usize) -> Result<()> {
        let len = self.selected().map_or(0, |p| p.env_files.len());
        if index >= len || index == self.profile {
            return Ok(());
        }
        self.ensure_clean()?;
        self.profile = index;
        self.load_profile()
    }

    pub fn start(&mut self) -> Result<()> {
        self.ensure_clean()?;
        let relative = self
            .profile_relative()
            .context("No env files here. Press a in the env pane to create one.")?;
        self.start_profile(self.project, &relative)
    }

    pub fn restart(&mut self) -> Result<()> {
        let running = self
            .server
            .label
            .as_ref()
            .filter(|_| self.server.running())
            .and_then(|label| {
                self.config
                    .projects
                    .iter()
                    .position(|p| p.path == label.project)
                    .map(|index| (index, label.profile.clone()))
            });
        match running {
            Some((index, profile)) => self.start_profile(index, &profile),
            None => self.start(),
        }
    }

    fn start_profile(&mut self, index: usize, relative: &Path) -> Result<()> {
        let project = self
            .config
            .projects
            .get(index)
            .context("Add a project first")?
            .clone();
        let path = project.path.join(relative);
        let text = fs::read_to_string(&path).context("Cannot read the env file")?;
        // Parsed before anything is stopped, so a broken file never takes the running server down.
        let vars = parse_env(&text)?;
        let Some(script) = project.script().cloned() else {
            self.popup = Some(Popup::Scripts {
                selected: 0,
                start_after: true,
            });
            self.info("Choose the script this project runs");
            return Ok(());
        };
        self.update_config(|c| {
            c.projects[index].selected_env = Some(relative.to_path_buf());
            Ok(())
        })?;
        let stopped = self.server.start(&project.path, relative, &script, &vars)?;
        self.watched = Some(Watched {
            stamp: Stamp::of(&path),
            path,
            text,
        });
        let extra = match stopped {
            0 => String::new(),
            1 => " · stopped 1 old server".into(),
            n => format!(" · stopped {n} old servers"),
        };
        self.info(format!(
            "Running {} · {}{extra}",
            relative.display(),
            script.command()
        ));
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if self.server.running() {
            self.server.stop()?;
            self.info("Server stopped");
        } else {
            self.info("No server is running");
        }
        Ok(())
    }

    pub fn save_buffer(&mut self) -> Result<()> {
        if self.buffer.path.is_none() {
            bail!("No env file is open");
        }
        self.buffer.save()?;
        let written = format!(
            "\"{}\" {}L written",
            self.buffer.name(),
            self.buffer.lines.len()
        );
        let runs_this = self
            .server
            .session
            .as_ref()
            .is_some_and(|s| Some(s.project.join(&s.profile)) == self.buffer.path);
        if runs_this {
            self.restart()?;
            self.info(format!("{written} · server restarted"));
        } else {
            self.info(written);
        }
        Ok(())
    }

    pub fn rescan(&mut self) -> Result<()> {
        self.ensure_clean()?;
        let index = self.project;
        self.update_config(|c| match c.projects.get_mut(index) {
            Some(project) => discovery::refresh(project),
            None => Ok(()),
        })?;
        self.load_project()?;
        self.info("Rescanned env files and scripts");
        Ok(())
    }

    pub fn create_env(&mut self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.contains('/') || name.contains('\\') || !is_env_name(name) {
            bail!("Use a file name such as .env.staging or staging.env (no folders)");
        }
        self.ensure_clean()?;
        let project = self.selected().context("Add a project first")?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(project.path.join(name))
            .context("Cannot create the file; it may already exist")?;
        file.write_all(b"# New environment profile\n")?;
        let index = self.project;
        self.update_config(|c| {
            discovery::refresh(&mut c.projects[index])?;
            c.projects[index].selected_env = Some(PathBuf::from(name));
            Ok(())
        })?;
        self.load_project()?;
        self.focus(Pane::Editor);
        self.info(format!(
            "Created {name} · i to edit · :w to save · Enter on it to run"
        ));
        Ok(())
    }

    pub fn submit_project(&mut self, editing: bool, name: &str, path: &str) -> Result<()> {
        self.ensure_clean()?;
        let mut project = discovery::create(name, path)?;
        for (index, existing) in self.config.projects.iter().enumerate() {
            if !(editing && index == self.project)
                && (existing.name == project.name || existing.path == project.path)
            {
                bail!("A project with that name or folder already exists");
            }
        }
        let index = self.project;
        if editing {
            let old = &self.config.projects[index];
            if old.path == project.path {
                project.selected_script = old.selected_script.clone();
                project.selected_env = old.selected_env.clone();
                discovery::refresh(&mut project)?;
            } else if self.server.runs(&old.path) {
                self.server.stop()?;
            }
        }
        self.update_config(|c| {
            if editing {
                c.projects[index] = project;
            } else {
                c.projects.push(project);
            }
            Ok(())
        })?;
        if !editing {
            self.project = self.config.projects.len() - 1;
        }
        self.load_project()?;
        self.focus(Pane::Env);
        self.info("Project saved · Enter runs the highlighted env file · s picks the script");
        Ok(())
    }

    pub fn remove_project(&mut self) -> Result<()> {
        let project = self.selected().context("Select a project")?.clone();
        if self.server.runs(&project.path) {
            self.server.stop()?;
        }
        let index = self.project;
        self.update_config(|c| {
            c.projects.remove(index);
            Ok(())
        })?;
        self.project = self
            .project
            .min(self.config.projects.len().saturating_sub(1));
        self.load_project()?;
        self.info(format!(
            "Removed {} from the list. Its files were kept.",
            project.name
        ));
        Ok(())
    }

    pub fn choose_script(&mut self, index: usize, start_after: bool) -> Result<()> {
        let project = self.selected().context("Select a project")?;
        let name = project
            .scripts
            .get(index)
            .context(
                "No scripts found. Add package.json scripts, Cargo.toml or pyproject.toml, then R.",
            )?
            .name
            .clone();
        let restart = start_after || self.server.runs(&project.path);
        let current = self.project;
        self.update_config(|c| {
            c.projects[current].selected_script = Some(name.clone());
            Ok(())
        })?;
        if restart {
            self.start()
        } else {
            self.info(format!("Script: {name} · Enter on an env file to run it"));
            Ok(())
        }
    }

    pub fn request_quit(&mut self, force: bool) {
        if self.buffer.dirty() && !force {
            self.error("E37: No write since last change (add ! to override)");
        } else {
            self.quit = true;
        }
    }

    pub fn tick(&mut self) -> Result<()> {
        if self.server.tick()? {
            self.needs_draw = true;
        }
        if self.server.running() {
            let second = self
                .server
                .session
                .as_ref()
                .map_or(0, |s| s.started.elapsed().as_secs());
            if second != self.last_second {
                self.last_second = second;
                self.needs_draw = true;
            }
        } else if self.terminal_mode {
            self.terminal_mode = false;
            self.needs_draw = true;
        }
        if !self.pending.is_empty()
            && self.pending_at.elapsed() >= Duration::from_millis(self.settings.timeout_ms)
        {
            self.flush_pending()?;
            self.needs_draw = true;
        }
        if self.last_watch.elapsed() < WATCH_EVERY {
            return Ok(());
        }
        self.last_watch = Instant::now();
        if self.buffer.reload_if_changed()? {
            self.needs_draw = true;
        }
        self.watch_running()
    }

    // A valid outside edit of the running env file restarts the server; an invalid one leaves it running.
    fn watch_running(&mut self) -> Result<()> {
        if !self.server.running() {
            return Ok(());
        }
        let Some(watched) = &mut self.watched else {
            return Ok(());
        };
        let stamp = Stamp::of(&watched.path);
        if stamp == watched.stamp {
            return Ok(());
        }
        watched.stamp = stamp;
        let text = fs::read_to_string(&watched.path)
            .context("The running env file became unreadable; the server keeps its old values")?;
        if text == watched.text {
            return Ok(());
        }
        watched.text = text.clone();
        self.needs_draw = true;
        parse_env(&text)?;
        self.restart()?;
        self.info("The running env file changed on disk · server restarted");
        Ok(())
    }

    pub fn contexts(&self) -> [Context; 2] {
        [self.pane.context(), Context::Global]
    }
}
