# Task 21-2 Human-Assisted Acceptance

This is the manual acceptance track for the release boundary. Do not infer these results from CI. Run the platform-specific documents on clean or dedicated machines and return the completed matrix plus feedback using `feedback-template.md`.

## Result values

Use exactly one result per case:

- `PASS`: expected result observed.
- `FAIL`: implementation or release defect observed.
- `BLOCKED`: environment prevented execution; include the exact blocker.
- `EXPECTED-LIMITATION`: behavior matches documented limitation.
- `NOT-APPLICABLE`: case cannot apply to the tested platform; explain why.

## Safety before testing

- Use a dedicated temporary user/profile/configuration area where possible.
- Do not paste complete profiles, environment dumps, tokens, passwords, API keys, or private target contents into reports.
- Sanitize usernames and machine-specific paths in screenshots/logs.
- Record OS, architecture, Shell/version, application/installer version, commit or artifact SHA256, and configuration directory.
- Stop immediately on unexpected deletion, profile corruption, secret exposure, or data loss; mark `FAIL` and preserve evidence.

## Case execution rule

The platform manuals are instructions, not test results. Leave `Actual`, `Result`, and `Evidence` blank until a human runs the case. A CI run, source inspection, or frontend build may support an automated boundary but cannot produce `PASS` for a manual case.

## Feedback submission

Return `test-matrix.md` with every executed case filled in, plus `feedback-template.md` entries for every FAIL/BLOCKED/EXPECTED-LIMITATION result. The agent will review the report before changing any release checkbox.

Manual acceptance does not automatically authorize merge, tag, release, signing, or publishing.

## Test groups

- Linux: `linux-clean-machine.md`
- Windows PowerShell 5.1: `windows-powershell51.md`
- Windows PowerShell 7: `windows-powershell7.md`
- Bash/Zsh user configuration: `bash-zsh-user-config.md`
- GUI: `gui-workflow.md`
- Package/install lifecycle: `package-install-lifecycle.md`

The same scenario must be repeated independently for PowerShell 5.1 and 7; one result never substitutes for the other.
