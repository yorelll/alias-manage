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
| `p` | per-alias preview (`f` switches to the whole-file fallback) |
| `D` | Doctor |
| `t` | tag filter |
| `o` / `O` | cycle sort / toggle descending |
| `R` | manual refresh; reports external changes |
| `Esc` | clear applied search/tag/sort filters; from detail focus it returns to the list and clears them in one press |
| `I` | import aliases from JSON/TOML |
| `X` | export all aliases to JSON/TOML |
| `S` | edit shared `config.toml` settings |
| `?` | help |
| `q` / `Ctrl+C` | quit |

While the search field is active, letters such as `q`, `s`, `d` and `/` are
text, not shortcuts. `Enter` applies the filter. `Esc` clears applied search, tag, sort and fuzzy filters and restores the full
list; from detail focus a single `Esc` returns to the list and clears them too.

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

Printing any character in a text field only types it. This includes `R`:
manual refresh is only the Browse `R`; typing `R` in a text field inserts text.
`Ctrl+S` is the only save shortcut, and a save already in progress refuses a
second submit.

### Preview / Doctor / Help / Sync results

| Key | Action |
|---|---|
| `j` / `k`, `PageUp` / `PageDown`, `Home` / `End` | scroll |
| `Tab` / `[` `]` | switch preview shell |
| `f` | preview mode: per-alias definition / whole shell file |
| `s` | sync (Doctor) |
| `r` | rescan (Doctor); retry only failed shells (Sync results) |
| `Esc` | back to the browser |
| `q` | quit |

## Import and export

`I` opens an import flow: enter a path, `Enter` previews, `y` confirms and
`Esc` cancels. Extension `.toml` selects TOML; anything else selects JSON,
matching the CLI. The preview reports imported/skipped/unsupported/warnings
without opening for writes or creating the database. `c` cycles the core
conflict strategy (Ask default, Skip, Overwrite, Rename). Ask and Skip skip
conflicts; Overwrite updates exact-name rows in place only when the same id
existed at preview time (rows created or replaced in between are skipped),
while PowerShell case-fold conflicts that need manual resolution are skipped
with a warning.
On confirmation the file is read once and its SHA-256 is computed over the
same bytes that are parsed; the fingerprint is checked against the preview
before reading, against the read bytes, and again before the database is
opened for the transaction (a changed file must be previewed again, and is
refused without creating an empty database). The records are re-evaluated
against the current database, and accepted records are persisted in one
transaction (`import_aliases_atomic` / `insert_aliases_atomic`). Sensitive
environment keys are removed by core before preview, persistence or display.

`X` exports all aliases in the shared database. The format is chosen by
extension, a path that already exists requires `y` overwrite confirmation, and
sensitive environment values are omitted by core. Export files are written via
a temporary file in the same directory and renamed over the target, so a failed
write leaves the previous file intact; directory targets are rejected. Read/parse/write work runs on
the existing background job worker; preview and export never hold the sync
lock. Missing files, invalid JSON/TOML, unsupported `format_version`, missing
parent directories and permission errors are reported as actionable status
messages.

## Settings

`S` edits the shared `config.toml` loaded through `AppPaths`: default shell,
language, `allow_relative_paths`, `backups.rc_keep`/`generated_keep`/`db_keep`
and `logs.keep_files`. Fields are validated with the shared
`AppConfig::validate` rules and numeric bounds before anything is written;
invalid values keep the draft and show a field error. `Ctrl+S` re-reads
`config.toml` first, applies only the fields this screen edits, then writes via
a temporary file and rename, preserving shell path overrides, unknown top-level
keys and external edits to other fields. A failed write leaves the previous
config intact.
Backup/log retention applies to future rotations. The config directory is
fixed at startup and shown read-only; change it with `ALIASMGR_CONFIG_DIR` or
`--config-dir` before launching. `runtime.language` is persisted for the GUI;
it does not localize CLI output or the TUI (which stays English).
`runtime.default_shell` is saved to shared config but is not currently consumed
by the CLI or TUI. Unknown keys nested inside known config tables are not
preserved; top-level unknown keys are.

The GUI (R2.0-P2a) already uses the shared first-run/setup APIs and strict save
preflight, and auto-syncs affected shells after GUI writes. The TUI first-run
integration and auto-sync are later TUI work: the current TUI still requires
the explicit `s` sync action, and shell enable/loader management remains in the
CLI `doctor`/GUI Settings panel. Both front ends share the same
`config.toml`, database and `shells.enabled` intent, so a shell enabled in the
GUI is immediately visible to the TUI.

## Interaction details

- The footer adapts to width. Key hints wrap as whole tokens (never a truncated
  `d ...`), and the status line remains visible at 80x24 and 120x40.
- `ALIASMGR_TUI_ASCII=1` uses `+ - |` borders and `...` truncation. It changes
  glyphs only and does not disable colour; combine it with `NO_COLOR` or
  `TERM=dumb` for monochrome ASCII output.
- Focus is marked in text: the header shows `focus: list/detail/field X/modal`,
  pane titles carry `[focus]`, the editor marks the selected field with `>`,
  and modal titles start with `[modal]`.
- The list title shows `selected/total`; preview and Doctor titles show the
  visible row range and total, and Doctor prefixes its selection with `sel` so
  `sel 1/8` is distinct from `rows 13-30/30`.
- The header shows `refreshed Ns ago`; `R` re-reads the database and reports
  whether external CLI/GUI changes were applied.
- Rename validates the name live with the core rules (syntax, PowerShell
  reserved names, duplicates and case-fold conflicts) and refuses Enter while
  invalid. Renaming to the unchanged name reports a no-op and does not bump the
  revision.
- Delete confirmation lists affected Shells, tombstone revision, and states that
  target executable/files are not deleted.
- Conflict modals show a field-by-field stored -> external diff (environment
  values masked) while retaining reload/discard/explicit-overwrite choices.
- After a partial sync, `r` on the Sync results screen retries only the failed
  Shells; successful files are not rewritten.

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
preview replaces every environment value with `***` before the core renderer
runs, so previews, errors and logs cannot leak a value. Environment keys remain
visible. The preview is read-only.

By default the preview renders only the selected alias's definition through the
same Shell adapter used by sync (`f` switches to the whole generated file as an
explicit fallback). The per-alias preview needs at least four content rows; the
80x24 frame minimum guarantees this, and smaller frames use the too-small
guard. An unrelated alias with a missing target therefore cannot
break the default preview. The whole-file fallback can report
`preview unavailable: target is missing: /srv/missing-tool` for another alias;
that is expected for that mode. The title labels the active mode and its row
position.

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

A leftover generated file with no alias or tombstone, and a `shell_state`-only
shell, are not targeted by `sync`. They therefore never keep the header at
"sync: needed"; Doctor reports them with manual-removal guidance instead.
Deleting the orphan file (or creating an alias for that Shell) is the documented
remedy, and the finding disappears after it.

## Not implemented in the TUI (Phase 5 remainder)

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
