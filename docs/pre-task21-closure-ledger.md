# Pre-Task 21 Closure Ledger

This ledger classifies unchecked items before Task 21. It is an audit record, not a duplicate broad requirement list. Original Task 1–20 checkboxes in `plan.md` remain authoritative.

| Area | Classification | Evidence target | Current state |
|---|---|---|---|
| GUI npm cache | implementation-feasible | lockfile + pinned GUI CI cache | Deferred until lockfile is committed |
| Filesystem reliability / `UnreliableFilesystem` | implementation-feasible where APIs are reliable; otherwise environment-blocked | core tests + platform CI | Fallback exists; filesystem-type certainty incomplete |
| Windows ACL writable path | implementation-feasible | Windows core test | Incomplete |
| Search sort/descending and complete CLI output | implementation-feasible | core/CLI tests + fast CI | Incomplete |
| Native argument matrix | CI-verifiable | Linux/Windows fixtures + integration jobs | Incomplete |
| Tombstone fingerprint/override recovery | CI-verifiable | Shell integration tests | Incomplete |
| SQLite sync transaction/forward recovery | CI-verifiable | sync/storage tests | Incomplete |
| CLI reload/doctor/transfer persistence | CI-verifiable | CLI integration tests | Incomplete |
| GUI Task 19/20 runtime commands | CI-verifiable | standalone command tests + GUI frontend CI | Partly placeholder |
| Task 21-1 automated matrix/release | implementation-feasible | workflow runs/artifacts | Not started |
| Task 21-2 clean-machine/Profile/GUI/package acceptance | manual/environment-dependent | user test reports | Not started |
| Fish/POSIX, import scanner, plugin API | post-MVP/out-of-scope | future plans | Not part of Task 21 closure |

## Rules

- Do not check implementation items without source and focused test evidence.
- Do not check manual items without a completed Task 21-2 report.
- Do not infer GUI runtime or visual acceptance from frontend typecheck/build.
- Record each CI run ID beside the checkbox it proves.
