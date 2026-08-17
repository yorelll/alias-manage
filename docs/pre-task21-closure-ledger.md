# Pre-Task 21 Closure Ledger

This ledger classifies unchecked items before Task 21. It is an audit record, not a duplicate broad requirement list. Original Task 1–20 checkboxes in `plan.md` remain authoritative.

| Area | Classification | Evidence target | Current state |
|---|---|---|---|
| GUI npm cache | implementation-feasible | lockfile + pinned GUI CI cache | Still deferred: no committed `package-lock.json`; current GUI jobs use `npm install` |
| Filesystem reliability / `UnreliableFilesystem` | implementation-feasible where APIs are reliable; otherwise environment-blocked | core tests + platform CI | `config.rs` classifies known Linux local/remote filesystem types conservatively; CI run `32003275982` passed; Windows filesystem/ACL certainty remains environment-blocked |
| Windows ACL writable path | implementation-feasible | Windows core test | Incomplete |
| Search sort/descending and complete CLI output | implementation-feasible | core/CLI tests + fast CI | Core sorting plus CLI field/sort/descending/limit/tag propagation and complete JSON rows implemented; CI run `32001624885` passed; integration run `32001809714` passed |
| Native argument matrix | CI-verifiable | Linux/Windows fixtures + integration jobs | Structured exact-boundary and Batch metacharacter tests plus fixtures implemented; fast CI `32004704944` and integration `32004874576` passed; host-specific PS native argv limits remain documented/manual |
| Tombstone fingerprint/override recovery | CI-verifiable | Shell integration tests | Generated metadata now carries tombstone names and deterministic fingerprints; CLI sync includes retired names; fast CI `32007567925` passed. Runtime fingerprint skip and override snapshots remain incomplete |
| SQLite sync transaction/forward recovery | CI-verifiable | sync/storage tests | Per-Shell `shell_state` success/failure now persists in SQLite; fast CI `32014090284` and integration `32014638440` passed. Transaction binding, committed forward recovery, and crash injection remain incomplete |
| CLI reload/doctor/transfer persistence | CI-verifiable | CLI integration tests | CLI sync includes retired Shell names; preview is read-only and confirmed import persistence is covered by fast CI `32011753420`; durable doctor reads `shell_state` and reports missing/expired/failed states; fast CI `32013771136` and integration `32010337861` passed |
| GUI Task 19/20 runtime commands | CI-verifiable | standalone command tests + GUI frontend CI | Task 20 command responses now read core/config/generated files, import reports, uninstall core, and override storage; frontend tests/typecheck/build passed fast CI `32009748252`; standalone Tauri/native runtime remains open |
| Task 21-1 automated matrix/release | implementation-feasible | workflow runs/artifacts | Not started |
| Task 21-2 clean-machine/Profile/GUI/package acceptance | manual/environment-dependent | user test reports | Not started |
| Fish/POSIX, import scanner, plugin API | post-MVP/out-of-scope | future plans | Not part of Task 21 closure |

## Rules

- Do not check implementation items without source and focused test evidence.
- Do not check manual items without a completed Task 21-2 report.
- Do not infer GUI runtime or visual acceptance from frontend typecheck/build.
- Record each CI run ID beside the checkbox it proves.
