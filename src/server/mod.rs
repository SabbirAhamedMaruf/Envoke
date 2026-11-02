pub mod keys;
pub mod procs;
pub mod pty;
pub mod view;

use crate::projects::Script;
use anyhow::{Context, Result};
use crossterm::event::KeyEvent;
use nix::{
    errno::Errno,
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use pty::{Pty, Spawn};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    num::NonZeroU16,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::ExitStatus,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const STOP_GRACE: Duration = Duration::from_millis(1200);
const PORT_SCAN_EVERY: Duration = Duration::from_secs(1);
// A server that has not opened a port by now is shown as running anyway (workers, CLIs).
const STARTING_FOR: Duration = Duration::from_secs(8);

#[derive(Clone, PartialEq, Eq)]
pub enum State {
    Idle,
    Starting,
    Running,
    Stopped,
    Exited { code: String, ok: bool },
}

pub struct Session {
    pty: Pty,
    pub pid: i32,
    pub project: PathBuf,
    pub profile: PathBuf,
    pub started: Instant,
}

// What the server pane is about: kept after the process ends so its last output stays labelled.
#[derive(Clone)]
pub struct Label {
    pub project: PathBuf,
    pub profile: PathBuf,
    pub command: String,
}

pub struct Server {
    pub session: Option<Session>,
    pub screen: Arc<Mutex<vt100::Parser>>,
    pub state: State,
    pub ports: BTreeSet<u16>,
    pub label: Option<Label>,
    pub scroll: usize,
    dirty: Arc<AtomicBool>,
    size: (u16, u16),
    scrollback: usize,
    last_scan: Instant,
}

fn parser(rows: u16, cols: u16, scrollback: usize) -> vt100::Parser {
    let nz = |v: u16| NonZeroU16::new(v.max(1)).unwrap_or(NonZeroU16::MIN);
    vt100::Parser::new(nz(rows), nz(cols), scrollback)
}

fn describe(status: ExitStatus) -> (String, bool) {
    match (status.code(), status.signal()) {
        (Some(code), _) => (format!("code {code}"), code == 0),
        (None, Some(signal)) => (
            Signal::try_from(signal).map_or_else(|_| format!("signal {signal}"), |s| s.to_string()),
            false,
        ),
        _ => ("unknown".into(), false),
    }
}

impl Server {
    pub fn new(scrollback: usize) -> Self {
        Self {
            session: None,
            screen: Arc::new(Mutex::new(parser(24, 80, scrollback))),
            state: State::Idle,
            ports: BTreeSet::new(),
            label: None,
            scroll: 0,
            dirty: Arc::new(AtomicBool::new(false)),
            size: (24, 80),
            scrollback,
            last_scan: Instant::now(),
        }
    }

    pub fn running(&self) -> bool {
        self.session.is_some()
    }

    pub fn runs(&self, project: &Path) -> bool {
        self.session.as_ref().is_some_and(|s| s.project == project)
    }

    pub fn runs_profile(&self, project: &Path, profile: &Path) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| s.project == project && s.profile == profile)
    }

    fn note(&self, text: &str, sgr: &str) {
        if let Ok(mut screen) = self.screen.lock() {
            screen.process(format!("\x1b[0m\x1b[{sgr}m{text}\x1b[0m\r\n").as_bytes());
        }
        self.dirty.store(true, Ordering::Relaxed);
    }

    // Stops the managed server, clears any leftover server of this project, then starts the script in a fresh terminal.
    pub fn start(
        &mut self,
        root: &Path,
        profile: &Path,
        script: &Script,
        vars: &BTreeMap<String, String>,
    ) -> Result<usize> {
        self.stop()?;
        let stale = procs::find_stale(root, &HashSet::new());
        procs::terminate(&stale);
        let (rows, cols) = self.size;
        self.screen = Arc::new(Mutex::new(parser(rows, cols, self.scrollback)));
        for s in &stale {
            let ports: Vec<String> = s.ports.iter().map(|p| format!(":{p}")).collect();
            self.note(
                &format!(
                    "envoke  stopped {} (PID {}) on {}",
                    s.name,
                    s.pid,
                    ports.join(" ")
                ),
                "33",
            );
        }
        self.note(
            &format!("envoke  {}  ·  {}", script.command(), profile.display()),
            "2",
        );
        self.label = Some(Label {
            project: root.into(),
            profile: profile.into(),
            command: script.command(),
        });
        self.scroll = 0;
        self.ports.clear();
        let pty = Pty::spawn(
            Spawn {
                program: &script.program,
                args: &script.args,
                cwd: root,
                vars,
                rows,
                cols,
            },
            Arc::clone(&self.screen),
            Arc::clone(&self.dirty),
        );
        let pty = match pty {
            Ok(pty) => pty,
            Err(error) => {
                self.state = State::Exited {
                    code: "start failed".into(),
                    ok: false,
                };
                self.note(&format!("{error:#}"), "31");
                return Err(error);
            }
        };
        self.session = Some(Session {
            pid: pty.child.id() as i32,
            pty,
            project: root.into(),
            profile: profile.into(),
            started: Instant::now(),
        });
        self.state = State::Starting;
        self.last_scan = Instant::now()
            .checked_sub(PORT_SCAN_EVERY)
            .unwrap_or_else(Instant::now);
        Ok(stale.len())
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut session) = self.session.take() {
            if let Err(error) = stop_group(&mut session) {
                self.session = Some(session);
                return Err(error);
            }
            self.state = State::Stopped;
            self.ports.clear();
            self.note("envoke  stopped", "2");
        }
        Ok(())
    }

    // Returns true when anything the screen shows has changed.
    pub fn tick(&mut self) -> Result<bool> {
        let mut changed = self.dirty.swap(false, Ordering::Relaxed);
        let exited = match &mut self.session {
            Some(session) => session.pty.child.try_wait()?,
            None => None,
        };
        if let Some(status) = exited {
            // The package manager can exit while a child server still holds the port.
            if let Some(mut session) = self.session.take() {
                stop_group(&mut session)?;
            }
            let (code, ok) = describe(status);
            self.note(
                &format!("[process exited, {code}]"),
                if ok { "2" } else { "31" },
            );
            self.state = State::Exited { code, ok };
            self.ports.clear();
            return Ok(true);
        }
        if let Some(session) = &self.session
            && self.last_scan.elapsed() >= PORT_SCAN_EVERY
        {
            self.last_scan = Instant::now();
            let table = procs::table();
            let tree = procs::descendants(&table, session.pid);
            let ports = procs::ports_of(&tree, &procs::listeners());
            if ports != self.ports {
                self.ports = ports;
                changed = true;
            }
            if self.state == State::Starting
                && (!self.ports.is_empty() || session.started.elapsed() >= STARTING_FOR)
            {
                self.state = State::Running;
                changed = true;
            }
        }
        Ok(changed)
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        let (rows, cols) = (rows.max(2), cols.max(10));
        if self.size == (rows, cols) {
            return;
        }
        self.size = (rows, cols);
        if let Ok(mut screen) = self.screen.lock() {
            screen.set_size(
                NonZeroU16::new(rows).unwrap_or(NonZeroU16::MIN),
                NonZeroU16::new(cols).unwrap_or(NonZeroU16::MIN),
            );
        }
        if let Some(session) = &self.session {
            session.pty.resize(rows, cols);
        }
    }

    pub fn send_key(&mut self, key: KeyEvent) {
        let app_cursor = self
            .screen
            .lock()
            .map(|s| s.screen().application_cursor())
            .unwrap_or(false);
        if let Some(session) = &mut self.session {
            session.pty.write(&keys::encode(key, app_cursor));
        }
    }

    pub fn paste(&mut self, text: &str) {
        let bracketed = self
            .screen
            .lock()
            .map(|s| s.screen().bracketed_paste())
            .unwrap_or(false);
        if let Some(session) = &mut self.session {
            if bracketed {
                session
                    .pty
                    .write(format!("\x1b[200~{text}\x1b[201~").as_bytes());
            } else {
                session.pty.write(text.as_bytes());
            }
        }
    }

    pub fn scroll_by(&mut self, delta: isize) {
        let max = self
            .screen
            .lock()
            .map(|s| s.screen().scrollback_len())
            .unwrap_or(0);
        self.scroll = self.scroll.saturating_add_signed(delta).min(max);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_by(isize::MAX);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

// SIGTERM to the whole session's process group, SIGKILL after a grace period, then reap the leader.
fn stop_group(session: &mut Session) -> Result<()> {
    let group = Pid::from_raw(session.pid);
    match killpg(group, Signal::SIGTERM) {
        Ok(()) | Err(Errno::ESRCH) => {}
        Err(error) => return Err(error).context("Cannot stop managed process group"),
    }
    let deadline = Instant::now() + STOP_GRACE;
    while Instant::now() < deadline {
        let _ = session.pty.child.try_wait()?;
        if killpg(group, None) == Err(Errno::ESRCH) {
            break;
        }
        thread::sleep(Duration::from_millis(30));
    }
    match killpg(group, Signal::SIGKILL) {
        Ok(()) | Err(Errno::ESRCH) => {}
        Err(error) => return Err(error).context("Cannot terminate managed process group"),
    }
    session
        .pty
        .child
        .wait()
        .context("Cannot reap managed process")?;
    Ok(())
}
