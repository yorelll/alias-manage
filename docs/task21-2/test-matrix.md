# Task 21-2 Test Matrix

Fill `Actual`, `Result`, and `Evidence` for every executed case. Do not check a case based on a CI result alone.

| ID | Platform/Shell | Feature | Expected | Actual | Result | Evidence |
|---|---|---|---|---|---|---| 

Do not fill `Actual`, `Result`, or `Evidence` from CI. These columns are completed only after the corresponding human case is run.
| L-001 | Linux | clean install/start/version | app starts and version is reported | | | |
| L-002 | Linux | add native alias | alias persists and appears in list | | | |
| L-003 | Linux | add Python/JAR/change-directory | each target type validates and renders | | | |
| L-004 | Linux | spaces/quotes/CJK/backslash/wildcards | arguments retain exact boundaries | | | |
| L-005 | Linux | `{{args}}` end/middle/invalid/repeated | valid positions work; invalid forms reject | | | |
| L-006 | Linux | working directory/environment | target runs in configured context without secret leakage | | | |
| L-007 | Linux | list/search/fuzzy/limit | results and ordering match request | | | |
| L-008 | Linux | tag facet single/multi/clear | selected tags use AND; clear restores all | | | |
| L-009 | Linux Bash | loader/reload | marked loader loads generated aliases | | | |
| L-010 | Linux Zsh | loader/reload | marked loader loads generated aliases | | | |
| L-011 | Linux Bash/Zsh | login/non-interactive | documented interactive behavior and login chain observed | | | |
| L-012 | Linux Zsh | oh-my-zsh ordering | post-loader override is reported and not silently changed | | | |
| L-013 | Linux | symlink RC/line endings | symlink remains; line endings preserved | | | |
| L-014 | Linux | manual generated edit | backup and user decision behavior occurs | | | |
| L-015 | Linux | disable/rename/delete/tombstone | reload removes managed residue without removing user definitions | | | |
| L-016 | Linux | sync failure/recovery | prior state restores and per-Shell state is accurate | | | |
| L-017 | Linux | JSON/TOML import | preview, warnings, confirmation, and persistence are correct | | | |
| L-018 | Linux | retain/purge uninstall | selected cleanup occurs; targets remain | | | |
| L-019 | Linux | upgrade/rollback | config and generated state survive or restore correctly | | | |
| PS51-001 | Windows | PS 5.1 clean install/start/version | app and PS 5.1 profile behavior works | | | |
| PS51-002 | Windows PS 5.1 | full lifecycle | add/edit/list/search/tag/sync/reload/delete works | | | |
| PS51-003 | Windows PS 5.1 | profile/OneDrive | resolved profile is correct and isolated | | | |
| PS51-004 | Windows PS 5.1 | BOM/CRLF | encoding and line endings are preserved | | | |
| PS51-005 | Windows PS 5.1 | built-in aliases | `ls`/`cp`/`gc` preemption and reserved names behave correctly | | | |
| PS51-006 | Windows PS 5.1 | ExecutionPolicy | Restricted/AllSigned/Group Policy/ConstrainedLanguage guidance is accurate | | | |
| PS51-007 | Windows PS 5.1 | native argv | exact argument matrix matches expected boundaries | | | |
| PS51-008 | Windows PS 5.1 | ACL/target protection | unsafe paths warn/block and targets remain intact | | | |
| PS51-009 | Windows PS 5.1 | loader/uninstall | marked changes are idempotent and safe | | | |
| PS51-010 | Windows | MSI/package hooks | installer/uninstaller uses intended retain/purge boundary | | | |
| PS7-001 | Windows | PS 7 clean install/start/version | app and PS 7 profile behavior works | | | |
| PS7-002 | Windows PS 7 | full lifecycle | add/edit/list/search/tag/sync/reload/delete works independently | | | |
| PS7-003 | Windows PS 7 | profile/OneDrive | resolved profile is correct and isolated | | | |
| PS7-004 | Windows PS 7 | BOM/CRLF | encoding and line endings are preserved | | | |
| PS7-005 | Windows PS 7 | built-in aliases | `ls`/`cp`/`gc` preemption and reserved names behave correctly | | | |
| PS7-006 | Windows PS 7 | ExecutionPolicy | Restricted/AllSigned/Group Policy/ConstrainedLanguage guidance is accurate | | | |
| PS7-007 | Windows PS 7 | native argv | exact argument matrix matches expected boundaries | | | |
| PS7-008 | Windows PS 7 | ACL/target protection | unsafe paths warn/block and targets remain intact | | | |
| PS7-009 | Windows PS 7 | loader/uninstall | marked changes are idempotent and safe | | | |
| GUI-001 | Linux/Windows | launch/version/status | window launches and drawer shows version/Shell/config | | | |
| GUI-002 | Linux/Windows | sidebar | Aliases/Sync/Doctor/Settings navigation works | | | |
| GUI-003 | Linux/Windows | table/description/tags | fields, tooltip, chips, and states render correctly | | | |
| GUI-004 | Linux/Windows | search/facet | text/fuzzy/limit/tag AND/clear work | | | |
| GUI-005 | Linux/Windows | wizard | basic/advanced/preview/save/cancel works | | | |
| GUI-006 | Linux/Windows | validation/conflicts | invalid/reserved/exact/case-fold errors preserve draft | | | |
| GUI-007 | Linux/Windows | Doctor | summary/details/actions and manual boundaries are accurate | | | |
| GUI-008 | Linux/Windows | import | preview/confirm/failure retention work | | | |
| GUI-009 | Linux/Windows | settings | draft/save/failure/read-only config path work | | | |
| GUI-010 | Linux/Windows | uninstall | retain/purge/risk/second confirmation work | | | |
| GUI-011 | Linux/Windows | overrides | copy/recoverable state works without execution | | | |
| GUI-012 | Linux/Windows | UX/accessibility | keyboard/focus/resize/visual layout acceptable | | | |
| PKG-001 | Linux/Windows | clean package install | package installs without modifying unrelated config | | | |
| PKG-002 | Linux/Windows | upgrade/rollback | package upgrade and rollback preserve expected state | | | |
| PKG-003 | Linux/Windows | hook/uninstall | postrm/MSI behavior is non-interactive and safe | | | |
| PKG-004 | Linux/Windows | unmanaged content | unrelated profiles/files remain unchanged | | | |
