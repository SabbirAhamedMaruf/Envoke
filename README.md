# envoke

Switch `.env` profiles and run your dev server in a Neovim-style terminal workspace. Built in Rust with **Ratatui + Crossterm**.

Four panes: **projects** (left), **env files** (middle), a **preview / editor** for the highlighted env file (right), and a real **server terminal** along the bottom. Moving through env files only previews them; a server starts only when you press Enter.

## Install

Download the package for your system from the [latest release](https://github.com/SabbirAhamedMaruf/Envoke/releases/latest):

```sh
# Debian / Ubuntu
sudo apt install ./envoke_1.0.0-1_amd64.deb

# Fedora
sudo dnf install ./envoke-1.0.0-1.fc44.x86_64.rpm
```

Then run `envoke` in a terminal of at least **72×18** (**120×32** or larger is nicer). Icons need a Nerd Font, or set `icons = false` (see [Configuration](#configuration)). Your projects still need their own runtime and package manager, such as Node and npm.

The packages install `/usr/bin/envoke`, this README, [KEYBINDINGS.md](KEYBINDINGS.md) and the license (GPL-3.0-or-later). The RPM **replaces the older `env-management` package** if it is installed. Remove envoke with `sudo apt remove envoke` or `sudo dnf remove envoke`; your configuration and projects stay.

## Build from source

Requires a current stable Rust toolchain and a C linker (`gcc`).

```sh
git clone https://github.com/SabbirAhamedMaruf/Envoke.git
cd Envoke
cargo build --release --locked
./target/release/envoke
```

`--help` and `--version` work without opening the TUI. To try it with an isolated configuration:

```sh
./target/release/envoke --config-dir /tmp/envoke-demo-config
```

## Build the packages

Both scripts build a native release binary and write the package plus a SHA-256 checksum to `dist/`. Neither needs `sudo`. The version comes from `Cargo.toml`.

**Debian / Ubuntu (`.deb`)**: needs Python 3.11+ and `readelf` (binutils), nothing from dpkg, so it also runs on Fedora.

```sh
./scripts/build-deb.sh            # dist/envoke_1.0.0-1_amd64.deb
DEB_REVISION=2 ./scripts/build-deb.sh
```

`Depends` is read from the binary itself: the newest glibc symbol version it uses becomes `libc6 (>= …)`. Build on the oldest Debian / Ubuntu you want to support.

**Fedora (`.rpm`)**: needs Python 3.11+, RPM query tools and `readelf` (`sudo dnf install gcc python3 rpm binutils coreutils`). The pinned [`cargo-generate-rpm`](https://github.com/cat-in-136/cargo-generate-rpm) 0.21.0 is downloaded into `target/rpm-tools` on first use; no `rpmbuild` or spec file.

```sh
./scripts/build-rpm.sh            # dist/envoke-1.0.0-1.fc44.x86_64.rpm
RPM_RELEASE=2 ./scripts/build-rpm.sh
```

Inspect a package before installing with `dpkg-deb --info / --contents` or `rpm -qpi / -qpl / -qpR`. These are unsigned packages; build them on the release and CPU architecture you install on.

**Releases**: pushing a `v*` tag runs `.github/workflows/release.yml`, which builds the `.deb` on Ubuntu 22.04 and the `.rpm` in a Fedora 44 container, test-installs both, and attaches them with their checksums to the GitHub release for that tag.

## First project

1. Press **a** in the projects pane, or type **:add-project**. A dialog asks for a **name** and a **folder**. **Tab** completes the folder: the first Tab fills in what all matches share and lists them, the next Tabs (or ↓ / ↑) walk through the list, **Enter** picks one and a second **Enter** saves. The dialog also opens by itself on the very first launch.
2. The app finds `.env`, `.env.*` and `*.env` files in the folder and its subfolders up to five levels deep, skipping dependencies, build folders and symlinks.
3. **Enter** or **l** opens the project's env files. **j / k** move through them and only change the preview.
4. **Enter** or **o** on an env file runs the project's script with that file's values. **s** chooses the script; `dev`, then `start`, then `cargo run` is picked automatically when available.
5. **l** moves into the preview. **i** edits, **Esc** returns to NORMAL mode, **:w** (or **Ctrl+S**) saves. Saving the file that is running restarts the server.
6. **?** lists every key. **:q** quits and stops the server.

## Keys and commands

Keys follow Neovim: NORMAL mode in every pane, INSERT mode in the editor, TERMINAL mode in the server pane. **[KEYBINDINGS.md](KEYBINDINGS.md)** lists every key, mode and command; press **?** for the same list inside the app. The most used ones:

| Key | Action |
| --- | --- |
| Tab / Shift+Tab, Ctrl+h / j / k / l | Move between panes |
| j / k, gg / G | Move down / up, first / last |
| Enter | Projects: open · env files: run the server |
| r / x | Restart / stop the server |
| a / e / dd | Projects: add / edit (dialog) / remove from the list |
| i, a, o, dd, yy, p, u, Ctrl+R | Editor: the usual Vim edits |
| i in the server pane, Ctrl+X | Type into the server / back to NORMAL |
| : / ? / Ctrl+Q | Command line / help / quit |

Every `:` command has a long and a short name:

| Long | Short | Long | Short |
| --- | --- | --- | --- |
| `:write` | `:w` | `:script [name]` | `:sc` |
| `:quit` / `:quit!` | `:q` / `:q!` | `:rescan` | `:rsc` |
| `:writequit` | `:wq` | `:new [file]` | `:n` |
| `:edit [file]` / `:edit!` | `:e` / `:e!` | `:add-project` | `:ap` |
| `:run [file]` | `:r` | `:edit-project` | `:ep` |
| `:restart` | `:rs` | `:remove-project` | `:rp` |
| `:stop` | `:st` | `:theme [name]` | `:th` |
| `:mask` | `:m` | `:keys` / `:help` | `:k` / `:h` |

Tab completes commands and their arguments (env files, scripts, themes); more Tabs walk through the choices.

## Configuration

Everything lives in `$XDG_CONFIG_HOME/envoke/`, normally `~/.config/envoke/`:

- `config.json` — registered projects, discovered env files and scripts, and the selected profile/script. **Env values stay in their own files**, never in the registry. Writes are atomic and private; a lock stops two instances editing it at once.
- `settings.toml` — theme (`terminal` or any NvChad base46 name such as `onedark`, `catppuccin`, `tokyonight`), icons, statusline separators, terminal height, scrollback, starting with values hidden, and the key-sequence timeout. A broken file shows a warning and falls back to defaults.
- `keymap.toml` — your own key bindings, generated from the built-in table. Invalid entries show a warning instead of stopping the app.

On first launch, a registry from the old `env-management` app (`~/.config/env-management/config.json`) is copied over once, so existing projects carry across.

## Commands discovered

| Manifest | Commands |
| --- | --- |
| `package.json` | Every string entry in `scripts`; launched with `npm/pnpm/yarn/bun run <script>` |
| `Cargo.toml` | `cargo run`, `cargo check`, `cargo test`, `cargo build` |
| `pyproject.toml` | `[project.scripts]` entry points via `uv run <entry>` |

For JavaScript projects, `packageManager` is preferred; otherwise lockfiles decide between pnpm, Yarn and Bun, falling back to npm. Manifests are read from the project root; for a monorepo, register the package folder you want to run. `uv` must be installed to use Python entry points.

## How a profile is applied

- **One server runs at a time.** Running a profile stops the previous server first.
- The env file is read with the same rules as Node's `dotenv` 17 (what Vite, Next and most Node tools use), so the server gets exactly the values those tools would read: `export`, `#` comments, single / double / backtick quotes, multiline quoted values, `\n` inside double quotes, and unquoted values with spaces such as `EMAIL_FROM=Name <hello@example.com>`. Like `dotenv`, lines it cannot read are skipped rather than rejected, a duplicate key takes its last value, and `${VAR}` is passed through as written (no expansion). The values are layered over the environment of a fresh child process. Nothing is copied over `.env`, and the app's own environment is untouched, so a previous profile's values never leak into the next one.
- Frameworks that read other `.env` files themselves may still pick up values the profile does not set. Frontend build-time values need a rebuild.
- Stopping sends SIGTERM to the server's whole process group and SIGKILL after a short grace period.
- Before starting, a leftover dev server of the same project that still holds a port (node, bun, deno, python, uv, cargo or a binary built in the project, running in the project folder, owned by you) is stopped. Anything running under an AI coding agent is never touched.
- The running env file is watched for outside changes; a change restarts the server with the new values.
- The editor writes the real file, keeping comments, order, line endings and permissions. Files over 1 MiB are not opened. Conflicting outside edits block the save instead of overwriting.
- Normal exit, SIGTERM and SIGHUP stop the server and restore the terminal. SIGKILL or a crash cannot run cleanup.

## Try the included demo

Register the `examples/demo` folder of your clone as a project and use its `dev` script. Node.js must be installed; no `npm install` is needed. Both profiles listen on **127.0.0.1:4317**, so switching exercises port release as well as env replacement.

```sh
curl http://127.0.0.1:4317
```

The JSON response shows the current profile and PID. Edit `GREETING`, save with `:w`, and request it again to see the restarted server's value.

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
```

## License

envoke is free software: you can redistribute it and/or modify it under the terms of the [GNU General Public License](LICENSE) as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version. It is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY.

Copyright (C) 2026 Sabbir Ahamed Maruf.
