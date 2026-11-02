use anyhow::{Context, Result, bail};
use crossterm::{
    cursor::SetCursorStyle,
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyEventKind,
    },
    execute,
};
use envoke::{app::App, projects::registry::Store, theme::Mode, ui};
use std::{
    io::{self, IsTerminal},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const HELP: &str = "envoke — switch .env profiles and run your dev server, Neovim style

Usage: envoke [--config-dir DIRECTORY]

Config:  $XDG_CONFIG_HOME/envoke/  (config.json, settings.toml, keymap.toml)
         or ~/.config/envoke/

Enter runs the highlighted env file · i edits · :w saves · ? lists every key · :q quits
Requires Linux, an interactive terminal, and your project's own package manager.
Every key and command: /usr/share/doc/envoke/KEYBINDINGS.md or press ? in the app.";

// The GNU convention for --version: who holds the copyright and that this is free software.
const LICENSE_NOTICE: &str = "Copyright (C) 2026 Sabbir Ahamed Maruf
License GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>.
This is free software: you are free to change and redistribute it.
There is NO WARRANTY, to the extent permitted by law.";

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            DisableBracketedPaste,
            SetCursorStyle::DefaultUserShape
        );
        ratatui::restore();
    }
}

fn main() -> Result<()> {
    let mut config_dir = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config-dir" => {
                config_dir = Some(PathBuf::from(
                    args.next().context("--config-dir needs a directory")?,
                ))
            }
            "--help" | "-h" => {
                println!("{HELP}");
                return Ok(());
            }
            "--version" | "-V" => {
                println!("envoke {}\n{LICENSE_NOTICE}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => bail!("Unknown option {arg}. Use --help."),
        }
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!(
            "Run envoke in an interactive terminal (at least {}×{}).",
            ui::MIN_WIDTH,
            ui::MIN_HEIGHT
        );
    }
    let (store, config) = Store::open(config_dir)?;
    let mut app = App::new(store, config)?;
    let interrupted = Arc::new(AtomicBool::new(false));
    for signal in [
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
        signal_hook::consts::SIGINT,
    ] {
        signal_hook::flag::register(signal, Arc::clone(&interrupted))?;
    }
    let mut terminal = ratatui::try_init()?;
    let _guard = TerminalGuard;
    execute!(io::stdout(), EnableMouseCapture, EnableBracketedPaste)?;
    let mut shape = None;
    while !app.quit && !interrupted.load(Ordering::Relaxed) {
        if app.needs_draw {
            app.needs_draw = false;
            terminal.draw(|frame| ui::draw(frame, &mut app))?;
            // Bar cursor while typing text, block otherwise, like Neovim's default guicursor.
            let bar = matches!(app.mode(), Mode::Insert | Mode::Command);
            if shape != Some(bar) {
                shape = Some(bar);
                let style = if bar {
                    SetCursorStyle::SteadyBar
                } else {
                    SetCursorStyle::SteadyBlock
                };
                execute!(io::stdout(), style)?;
            }
        }
        // Poll faster while a server may be printing; idle screens wake only for input or the clock.
        let wait = if app.server.running() { 30 } else { 200 };
        if event::poll(Duration::from_millis(wait))? {
            let result = match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => app.handle_key(key),
                Event::Mouse(mouse) => app.mouse(mouse),
                Event::Paste(text) => {
                    app.paste(&text);
                    Ok(())
                }
                Event::Resize(..) | Event::FocusGained => {
                    app.needs_draw = true;
                    Ok(())
                }
                _ => Ok(()),
            };
            if let Err(error) = result {
                app.fail(error);
                app.needs_draw = true;
            }
        }
        if let Err(error) = app.tick() {
            app.fail(error);
            app.needs_draw = true;
        }
    }
    app.server.stop()?;
    Ok(())
}
