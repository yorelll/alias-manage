# TUI

`aliasmgr tui` is the unified interactive terminal front end. One TUI manages
aliases for Bash, Zsh, PowerShell 5.1 and PowerShell 7; there is no per-Shell
TUI and no second database.

```bash
aliasmgr tui
aliasmgr --config-dir /tmp/aliasmgr-demo tui
ALIASMGR_CONFIG_DIR=/tmp/aliasmgr-demo aliasmgr tui
```

The TUI resolves the configuration root exactly like the CLI: `--config-dir`,
then `ALIASMGR_CONFIG_DIR`, then the platform default. It reads and writes the
same `<config-root>/aliases.db` and calls the same core `AliasService`,
`sync::sync_config_dir_collect_io`, `doctor::diagnose`, loader integration and
shell renderers. The CLI, TUI and GUI therefore always agree.

## Build

The `tui` feature is on by default:

```bash
cargo build --release -p aliasmgr-cli                     # CLI + TUI
cargo build --release -p aliasmgr-cli --no-default-features  # minimal CLI only
```

With `--no-default-features`, `aliasmgr tui` prints that TUI support is not
compiled in and exits with code 1. No ratatui/crossterm code is linked.

## Terminal requirements

- Interactive stdin and stdout. In a pipe, CI job, or redirected output,
  `aliasmgr tui` fails immediately with a clear message and exit code 1 instead
  of blocking or emitting escape sequences.
- Usable from 80x24. At 120 columns or more the browser shows the alias list
  and the detail pane side by side; 80–119 columns use a single pane where
  `Enter` opens detail, `Tab`/`Shift+Tab` switch focus, and `Esc` goes back.
- Smaller than 80x24 shows "Terminal too small" with the frame size and the
  minimum. While too small every key except `q`/`Ctrl+C` is ignored, any armed
  modal is cleared, and resizing recovers automatically. `q`/`Ctrl+C` quit
  immediately even with an unsaved draft, because no confirmation can be shown.
- No mouse, clipboard, Nerd Font, emoji or true-colour dependency.
- `NO_COLOR` (presence) and `TERM=dumb` disable colour; `TERM=dumb` and
  `ALIASMGR_TUI_ASCII=1` use plain borders and `...` instead of `…`.
- Status is never colour-only: `[on]`, `[off]`, `[ok]`, `[warn]`, `[error]`,
  `[x]`, `[ ]` and `!` carry the meaning.
- Ctrl+C is handled as a key (raw mode disables the signal) and asks before
  discarding an unsaved editor draft.

## Keys

### Browse

| Key | Action |
|---|---|
| `Up` / `Down`, `j` / `k` | move selection |
| `PageUp` / `PageDown` | page |
| `Home` / `End`, `g` / `G` | first / last |
| `Enter` | detail (narrow) or focus detail (wide) |
| `Tab` / `Shift+Tab` | list/detail focus |
| `/` | search; printable keys go to the query |
| `F` | toggle fuzzy search |
| `n` | new alias |
| `e` | edit selected alias |
| `r` | rename (confirmation modal) |
| `Space` | enable / disable |
| `d` | delete (confirmation modal) |
| `s` | sync in the background |
| `p` | generated-content preview |
| `D` | Doctor |
| `t` | tag filter |
| `o` / `O` | cycle sort / toggle descending |
| `?` | help |
| `q` / `Ctrl+C` | quit |

While the search field is active, letters such as `q`, `s`, `d` and `/` are
text, not shortcuts. `Enter` applies the filter; `Esc` clears it.

### Edit

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | next / previous field |
| `Ctrl+S` | save |
| `Esc` | cancel (asks when dirty) |
| `Up` / `Down`, `j` / `k` | move inside the focused list |
| `a` | add a fixed arg / tag / environment entry / restriction path |
| `d` | delete the focused row |
| `J` / `K` | move the focused row down / up |
| `Enter` | edit the focused row |
| `Space` | toggle pass-args, shell, path mode, restriction mode |
| `Left` / `Right` | change target type or restriction mode |
| `F3` | show / hide advanced fields |

Printing any character in a text field only types it. `Ctrl+S` is the only
save shortcut, and a save already in progress refuses a second submit.

### Preview / Doctor / Help / Sync results

| Key | Action |
|---|---|
| `j` / `k`, `PageUp` / `PageDown`, `Home` / `End` | scroll |
| `Tab` / `[` `]` | switch preview shell |
| `s` | sync (Doctor) |
| `r` | rescan (Doctor) |
| `Esc` | back to the browser |
| `q` | quit |

## Target type semantics

Target type is classification and validation metadata. Only two types change
runtime behaviour:

- `batch` adds the Batch/CMD metacharacter check.
- `change_directory` changes directory instead of running the executable.

`native_executable`, `python_script`, `powershell_script`, `shell_script`,
`java_jar` and `cmdlet` all run `executable` directly. No interpreter is
inserted. To run `python test.py`, set `executable=python` and a fixed argument
`test.py`; do not put `python test.py` in the executable. The detail pane and
the editor's argv preview show this expansion.

`RawShellCommand` is not offered and remains rejected by validation.

## Structured arguments

Fixed arguments are a list of argv elements, one per row:

- spaces, single and double quotes and backslashes are literal characters;
- an empty string is a real element, shown as `(empty string)`;
- CJK and other Unicode text is preserved;
- `{{args}}` inserts user arguments at that position exactly once;
- `{{{{args}}}}` renders the literal text `{{args}}`;
- nothing is split, joined, re-quoted, `eval`-ed or passed through
  `Invoke-Expression`.

The editor's argv preview lists one element per line (with placeholders for
user arguments), so an element boundary is never ambiguous. Validation uses the
core `validate_alias`; the TUI does not keep a second rule set.

## Status layers

The detail pane separates four independent facts:

1. **DB** – the record was saved and its revision.
2. **Generated** – the shell's file is compared with a freshly rendered
   expected body and its embedded checksum, not just with revision counters.
   It reports up to date, stale content/checksum mismatch, missing, unreadable,
   render failure, last-sync failure, or not synced yet. The header uses the
   same per-shell verdict, so it always agrees with the detail pane; a checked
   edit or delete that does not advance the global derived revision is still
   detected.
3. **Loader** – the marked loader block is installed in the Shell profile
   (including shared-profile and unsupported-platform cases).
4. **Current shell** – always "unknown". The TUI cannot inspect or reload the
   shell that launched it.

Saving only writes the database and then says "press s to sync". Sync runs in
a background thread, reports a result per Shell (status, generated path, loader
state, errors), and then says the reload command still has to be run manually.
Reload is never automatic: the TUI never sources the generated file, starts a
shell, changes ExecutionPolicy, or writes outside the marked loader block.

## Preview and secrets

Environment values are masked as `***` in the editor list and detail pane.
They are only visible while the user explicitly edits that one value. A
generated-content preview replaces every environment value with `***` before
the core renderer runs, so previews, errors and logs cannot leak a value.
Environment keys remain visible. The preview is read-only.

## Conflicts and external edits

The TUI re-reads the database on refresh and detects writes from the CLI/GUI.
Opening a stored record captures its revision. Save is revision-checked in the
database update (`UPDATE ... WHERE id = ? AND revision = ?`):

- an unchanged revision saves;
- a changed record shows a conflict modal with **reload latest**, **discard
  draft**, or **explicit overwrite**; nothing is overwritten silently;
- a record deleted in another front end shows a deletion modal with **discard
  draft** or **recreate from this draft**;
- delete, rename and enable/disable use the same checked path, and the rename
  modal stays bound to the record it was opened on even if the list refreshes;
- an external change while editing keeps the local draft and warns that saving
  will ask before overwriting.

A rename performed in the editor retires the old name for the bound shells, so
the next sync emits the cleanup block, matching `aliasmgr rename`.

## Doctor

`D` runs the shared read-only diagnostics and lists findings with severity,
Shell, stable code, message, action and detail. `s` starts a sync (the
actionable fix for stale/missing generated files); `r` rescans. The TUI does
not install or remove loaders in this version; use
`aliasmgr shell install|uninstall` with the CLI.

## Not implemented in the TUI (Phase 5 remainder)

- Import/export preview and confirmation.
- Settings (`config.toml`) editing.
- Loader install/uninstall actions (Doctor shows the finding and points at the
  CLI).
- Bulk multi-select operations.
- `uninstall` (retain/purge).
- Mouse and clipboard integration (deliberately out of scope).
- Search-field selection beyond "all fields" (name/target/description/tags/
  shells/target type); tag filtering and sorting are available.

## Manual platforms

Automated tests use ratatui `TestBackend`; they prove rendering and state
transitions, not a real terminal. Real-terminal acceptance is still required on
Windows PowerShell 5.1, PowerShell 7 (Windows Terminal), Linux Bash and Zsh:
alternate-screen/raw-mode restoration, resize, Ctrl+C, CJK fonts, colour
fallback, and exit cleanup. This change was built and tested on Windows only;
Linux terminal behaviour is unverified.
