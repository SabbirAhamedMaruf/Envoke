# envoke key bindings

envoke works like Neovim: you are always in a **mode**, shown at the left of the statusline.

| Mode | When | How to leave |
| --- | --- | --- |
| NORMAL | Moving around the projects, env files and preview panes | — |
| INSERT | Typing into the env file in the editor | `Esc` |
| COMMAND | Typing a `:` command | `Enter` runs it, `Esc` cancels |
| TERMINAL | Typing straight into the running server | `Ctrl+x` (or `Ctrl+\` `Ctrl+n`) |
| NTERMINAL | NORMAL mode on the server pane: scroll its output | `Tab`, `Ctrl+k` |
| CONFIRM | A yes / no question on the command line | `y` confirms, anything else cancels |

Every key below is the default and can be changed in `keymap.toml` (see [Changing keys](#changing-keys)). Keys are written in Vim notation: `<C-s>` is Ctrl+S, `<S-Tab>` is Shift+Tab, `<CR>` is Enter. A key written as two letters, such as `gg`, means pressing them one after the other.

A count in front of a list or editor key repeats it: `3j` moves three lines, `2dd` deletes two lines.

## Everywhere (NORMAL mode)

| Key | Action |
| --- | --- |
| `<Tab>` / `<S-Tab>` | Next / previous pane |
| `<C-h>` `<C-j>` `<C-k>` `<C-l>` | Pane to the left / below (server terminal) / above / to the right |
| `:` | Command line |
| `?` | Show every key and command |
| `<C-q>` | Quit and stop the server |
| `<C-w>+` / `<C-w>-` | Make the server terminal taller / shorter |
| `zm` | Hide / show env values |

`Ctrl+C` does not quit (as in Neovim); it reminds you to use `:q`.

## Projects pane (left)

| Key | Action |
| --- | --- |
| `j` `<Down>` / `k` `<Up>` | Next / previous project |
| `gg` `<Home>` / `G` `<End>` | First / last project |
| `<CR>` `l` | Open the project's env files |
| `a` | Add a project (dialog) |
| `e` | Rename / move the project (dialog) |
| `dd` | Remove from the list (the project's files are kept) |
| `s` | Choose the run script |
| `R` | Rescan env files and scripts |
| `q` | Quit |

## Env files pane (middle)

Moving through env files only changes the preview. Nothing starts until you press Enter.

| Key | Action |
| --- | --- |
| `j` `<Down>` / `k` `<Up>` | Next / previous env file |
| `gg` `<Home>` / `G` `<End>` | First / last env file |
| `<CR>` `o` | Run the server with this env file |
| `r` | Restart the server |
| `x` | Stop the server |
| `h` | Back to projects |
| `l` | Go to the preview / editor |
| `a` | Create an env file |
| `s` | Choose the run script |
| `R` | Rescan env files and scripts |
| `q` | Quit |

## Preview / editor pane (right, NORMAL mode)

| Key | Action |
| --- | --- |
| `h` `<Left>` / `l` `<Right>` | Cursor left / right |
| `j` `<Down>` / `k` `<Up>` | Cursor down / up |
| `w` / `b` / `e` | Next word / previous word / end of word |
| `0` `<Home>` / `^` / `$` `<End>` | Start of line / first character / end of line |
| `gg` / `G` | First / last line |
| `<C-d>` / `<C-u>` | Half a page down / up |
| `i` / `a` | Insert before / after the cursor |
| `I` / `A` | Insert at the start / end of the line |
| `o` / `O` | New line below / above |
| `ga` | Append `NEW_KEY=` and start editing it |
| `x` `<Del>` | Delete the character |
| `dd` | Delete the line |
| `D` | Delete to the end of the line |
| `yy` `Y` | Copy the line |
| `p` / `P` | Paste after / before |
| `u` / `<C-r>` | Undo / redo |
| `<C-s>` | Save (restarts the server if this file is the one running) |

Double-clicking a line also enters INSERT mode. Entering INSERT mode shows hidden values.

## INSERT mode

| Key | Action |
| --- | --- |
| `<Esc>` | Back to NORMAL mode |
| `<C-s>` | Save without leaving INSERT mode |
| `<Tab>` / `<S-Tab>` | Leave INSERT mode and go to the next / previous pane |

Every other key types text. Arrows, Home / End, Backspace, Delete, Enter and paste work as usual.

## Server terminal pane (bottom, NORMAL mode)

| Key | Action |
| --- | --- |
| `j` `<Down>` / `k` `<Up>` | Scroll the output down / up |
| `<C-d>` / `<C-u>` | Half a page down / up |
| `gg` / `G` | Oldest output / newest output (follow) |
| `i` `a` | TERMINAL mode: type into the server |
| `<CR>` | Run the highlighted env file |
| `r` | Restart the server |
| `x` | Stop the server |
| `q` | Quit |

## TERMINAL mode

| Key | Action |
| --- | --- |
| `<C-x>` or `<C-\><C-n>` | Back to NORMAL mode (NvChad's `<C-x>`) |

Every other key, including `Ctrl+C`, goes to the server, exactly as in a normal terminal.

## Project dialog

Opened with `a` / `e` in the projects pane, `:add-project` / `:edit-project`, and on the very first launch.

| Key | Action |
| --- | --- |
| `<Tab>` `<CR>` `<Down>` on Name | Go to the folder field |
| `<Tab>` on Folder | Complete the folder: fill in what all matches share and list them |
| `<Tab>` `<Down>` / `<S-Tab>` `<Up>` while the list is shown | Next / previous folder in the list |
| `<CR>` on a highlighted folder | Choose it (the list closes) |
| `<CR>` | Save the project |
| `<S-Tab>` `<Up>` | Back to the name field |
| `<Esc>` | Close the list, or cancel the dialog |

## Popups

| Popup | Keys |
| --- | --- |
| Help (`?`) | `j` `k` scroll, `d` `u` ten lines, `g` / `G` top / bottom, `q` `<Esc>` close |
| Run script (`s`) | `j` `k` move, `<CR>` choose, `q` `<Esc>` close |

## Command line

Type `:` and a command. Every command has a long and a short name.

| Long | Short | Does |
| --- | --- | --- |
| `:write` | `:w` | Save the open env file (restarts its server) |
| `:quit` | `:q` | Quit and stop the server; refuses while there are unsaved edits |
| `:quit!` | `:q!` | Quit and throw away unsaved edits |
| `:writequit` | `:wq` (also `:x`) | Save if changed, then quit |
| `:edit [file]` | `:e [file]` | Open an env file of this project; with no name, reload the open one |
| `:edit!` | `:e!` | Throw away edits and reload from disk |
| `:run [file]` | `:r [file]` | Run the server with the highlighted (or named) env file |
| `:restart` | `:rs` | Restart the running server |
| `:stop` | `:st` | Stop the server |
| `:script [name]` | `:sc [name]` | Choose the run script from a list, or by name |
| `:rescan` | `:rsc` | Rescan env files and scripts |
| `:new [file]` | `:n [file]` | Create an env file in the project folder |
| `:add-project` | `:ap` | Open the add project dialog |
| `:edit-project` | `:ep` | Rename or move the selected project |
| `:remove-project` | `:rp` | Remove the project from the list (files are kept) |
| `:theme [name]` | `:th [name]` | Show or switch the theme (`terminal` or any NvChad theme) |
| `:mask` | `:m` | Hide / show env values |
| `:keys` | `:k` | Show where `keymap.toml` lives |
| `:help` | `:h` | List every key and command |
| `:<number>` | | Jump to that line in the editor |

Keys on the command line:

| Key | Action |
| --- | --- |
| `<Tab>` | Complete the command, then its argument (env file, script or theme). More Tabs walk through the choices shown in the statusline; `<S-Tab>` goes back |
| `<Up>` / `<Down>` | Older / newer command from history (or move through the choices while they are shown) |
| `<C-u>` / `<C-w>` | Delete to the start / delete the previous word |
| `<BS>` on an empty line | Leave the command line |
| `<Esc>` | Cancel |

## Mouse

| Action | Result |
| --- | --- |
| Click a project or env file | Select it (an env file is only previewed) |
| Double-click an env file | Run the server with it |
| Click in the editor | Move the cursor there; double-click to edit |
| Wheel | Scroll the pane under the pointer, without moving the focus |

## Changing keys

envoke writes `~/.config/envoke/keymap.toml` on first launch with every action and its default keys. Edit it and restart envoke. Each action takes one key or a list of keys:

```toml
[env]
start = ['<CR>', '<Space>']   # also run with Space

[editor]
save = '<C-s>'

[terminal]
normal = '<C-x>'
```

- Sections: `[global]`, `[projects]`, `[env]`, `[editor]`, `[server]`, `[insert]`, `[terminal]`. A pane section wins over `[global]` for the same key.
- Delete a line to get its default back; `[]` unbinds the action.
- Notation: `<C-x>` Ctrl, `<A-x>` Alt, `<S-Tab>` Shift+Tab, `<CR>`, `<Esc>`, `<BS>`, `<Del>`, `<Space>`, `<Up>` `<Down>` `<Left>` `<Right>`, `<Home>` `<End>`, `<PageUp>` `<PageDown>`, `<F1>`..`<F12>`, `<lt>` for `<`.
- A mistake in the file never stops envoke from opening: the first problem is shown on the command line and the built-in key is used instead.
- How long envoke waits for the second key of a sequence such as `gg` is `timeout_ms` in `settings.toml` (default 1000).
