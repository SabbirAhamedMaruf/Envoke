use anyhow::{Context, Result};
use nix::{
    libc,
    pty::{OpenptyResult, Winsize, openpty},
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::process::CommandExt,
    },
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

pub struct Pty {
    pub child: Child,
    writer: File,
    master: OwnedFd,
}

fn winsize(rows: u16, cols: u16) -> Winsize {
    Winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    }
}

fn cloexec(fd: &OwnedFd) {
    // openpty returns inheritable descriptors; the master must never leak into the server.
    unsafe {
        libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC);
    }
}

pub struct Spawn<'a> {
    pub program: &'a str,
    pub args: &'a [String],
    pub cwd: &'a Path,
    pub vars: &'a BTreeMap<String, String>,
    pub rows: u16,
    pub cols: u16,
}

impl Pty {
    // Runs the command as the leader of a new session whose controlling terminal is the pty, like a terminal tab would.
    pub fn spawn(
        spec: Spawn,
        screen: Arc<Mutex<vt100::Parser>>,
        dirty: Arc<AtomicBool>,
    ) -> Result<Self> {
        let OpenptyResult { master, slave } =
            openpty(&winsize(spec.rows, spec.cols), None).context("Cannot open a terminal")?;
        cloexec(&master);
        cloexec(&slave);
        let child = {
            let mut command = Command::new(spec.program);
            command
                .args(spec.args)
                .current_dir(spec.cwd)
                .envs(spec.vars)
                .env("TERM", "xterm-256color")
                .stdin(Stdio::from(slave.try_clone()?))
                .stdout(Stdio::from(slave.try_clone()?))
                .stderr(Stdio::from(slave));
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            command.spawn().with_context(|| {
                format!(
                    "Cannot run {}. Is it installed and on your PATH?",
                    spec.program
                )
            })?
        };
        let mut reader = File::from(master.try_clone()?);
        let writer = File::from(master.try_clone()?);
        thread::spawn(move || {
            let mut buffer = [0u8; 16 * 1024];
            // Ends with EIO once every process holding the terminal has exited.
            while let Ok(size) = reader.read(&mut buffer) {
                if size == 0 {
                    break;
                }
                if let Ok(mut parser) = screen.lock() {
                    parser.process(&buffer[..size]);
                }
                dirty.store(true, Ordering::Relaxed);
            }
        });
        Ok(Self {
            child,
            writer,
            master,
        })
    }

    pub fn write(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        let size = winsize(rows, cols);
        unsafe {
            libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &size);
        }
    }
}
