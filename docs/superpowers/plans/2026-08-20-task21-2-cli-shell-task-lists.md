# Task 21-2 CLI/Shell Task Lists Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 执行本计划。所有步骤使用复选框跟踪，并在每个独立任务后完成文档校验。

**Goal:** Add explicit Chinese CLI and Shell task lists that link to existing detailed manuals, distinguish script-verifiable and human-only cases, and synchronize the current sanitized results from `test/` without turning limitations into false PASS results.

**Architecture:** Keep the existing detailed manuals, `test-matrix.md`, and `feedback-template.md` as the source of truth. Add one concise checklist per directory: `01-cli/task-list.md` for artifact smoke and CLI boundaries, and `02-shell/task-list.md` for Bash/Zsh/PowerShell 5.1/7 plus exploratory Git Bash. Each item has a checkbox, ID, type, manual link, current result, and evidence path. Script items are checked only when their local sanitized summary reports `PASS`; manual items remain unchecked until the user fills Actual/Result/Evidence.

**Tech Stack:** Markdown, sanitized JSON summaries, existing Task 21-2 Chinese manuals.

---

## File map

- Create: `docs/task21-2/01-cli/task-list.md` — CLI artifact preparation, Linux/Windows CLI smoke checklist, script/manual split, and current result evidence.
- Create: `docs/task21-2/02-shell/task-list.md` — formal Bash/Zsh/PowerShell 5.1/7 checklist plus exploratory Git Bash classification and current script evidence.
- Modify: `docs/task21-2/01-cli/README.md` — add the task-list link and checkbox/result rules.
- Modify: `docs/task21-2/02-shell/README.md` — add the task-list link and checkbox/result rules.
- Read-only source: `docs/task21-2/test-matrix.md`, `docs/task21-2/feedback-template.md`, detailed platform manuals, and `test/**/verification-summary.json`.

## Shared checklist contract

Every checklist item must use this shape:

```markdown
- [x] L-001：Linux CLI 启动和版本
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [ ] L-003：Python/JAR/切换目录目标
  - 类型：人工验证
  - 当前结果：待执行
  - 详细说明：[linux-cli.md](linux-cli.md)
  - 证据：执行后填写 `docs/task21-2/test-matrix.md`
```

Rules to repeat in both lists:

- `[x]` means the specific checklist item has evidence; `[ ]` means pending.
- For script items, check only `PASS`. `PASS_WITH_EXPECTED_LIMITATIONS` checks only the individual cases whose result is `PASS`.
- Keep `EXPECTED-LIMITATION`, `BLOCKED`, and `NOT-APPLICABLE` unchecked and show the exact result plus reason.
- Human items can be checked only after the user records Actual, Result, Evidence, and redaction confirmation in `test-matrix.md` or `feedback-template.md`.
- Preserve the exact result enum: `PASS | FAIL | BLOCKED | EXPECTED-LIMITATION | NOT-APPLICABLE`.
- Do not infer human Profile, current-shell, GUI, visual, or package PASS from CI or script output.

---

### Task 1: Add the CLI task list

**Files:**
- Create: `docs/task21-2/01-cli/task-list.md`
- Modify: `docs/task21-2/01-cli/README.md`

- [ ] **Step 1: Add the CLI checklist entry point**

Add this near the top of `docs/task21-2/01-cli/README.md`:

```markdown
## 任务清单

开始执行前先打开：[task-list.md](task-list.md)。

清单中的脚本验证和人工验证必须分别完成。脚本结果只同步到对应脚本项目；不能用 CI 或 CLI 脚本结果代替真实用户配置、升级、回滚或安装包人工验收。
```

- [ ] **Step 2: Create the CLI task list sections**

Create `01-cli/task-list.md` with these sections and links:

1. `准备和校验` — artifact download, SHA256, extraction, metadata; link `artifact-smoke.md`.
2. `Linux CLI` — script-verifiable IDs `L-001`, `L-002`, `L-003d`, `L-004`, `L-004-ENV`, `L-005`, `L-006`, `L-007`, `L-008`, `L-009`, `L-010`, `L-011`, `L-012`, `L-012b`, `L-013`, `L-014`, `L-015`, `L-016`, `L-016b`, `L-016c`, `L-017`, `L-017b`, `L-018`, `L-019`; link `linux-cli.md` and the current `test/aliasmgr-last-result/verification-summary.json` evidence.
3. `Windows CLI` — script-verifiable IDs `W-001`, `W-002`, `W-002b`, `W-003`, `W-004`, `W-005`, `W-007`, `W-009`, `W-011`; link `windows-cli.md` and `test/aliasmgr-win-last-result/verification-summary.json`.
4. `CLI 人工验证` — keep unchecked entries for Python/JAR/change-directory targets, real clean-machine lifecycle, upgrade/rollback, and any matrix cases not covered by the scripts; link the exact manual subsection rather than copying all steps.
5. `提交结果` — link `test-matrix.md` and `feedback-template.md`, state that result/log paths must be sanitized.

For the current summaries, mark all listed Linux CLI and Windows CLI script cases whose result is `PASS` as `[x]`. Do not mark the manual cases or unsupported command cases as complete.

- [ ] **Step 3: Record current CLI result metadata**

At the top of `task-list.md`, add a metadata note:

```markdown
当前结果来源：`test/aliasmgr-last-result/verification-summary.json` 和 `test/aliasmgr-win-last-result/verification-summary.json`。
当前 CLI 脚本结果：Linux `PASS`（33/33）；Windows `PASS_WITH_EXPECTED_LIMITATIONS`（9 PASS，4 EXPECTED-LIMITATION）。
```

Do not copy full command output or sensitive paths into the task list.

- [ ] **Step 4: Verify CLI links and checklist semantics**

Check that every referenced manual and evidence path exists, every ID is present in `test-matrix.md` or is explicitly labeled as a script subcase, and no `EXPECTED-LIMITATION` item is checked. Use read-only repository checks; do not run local CLI/Rust/Shell commands.

- [ ] **Step 5: Commit the CLI checklist**

```bash
git add docs/task21-2/01-cli/task-list.md docs/task21-2/01-cli/README.md
git commit -m "docs: add CLI task checklist"
```

---

### Task 2: Add the Shell task list

**Files:**
- Create: `docs/task21-2/02-shell/task-list.md`
- Modify: `docs/task21-2/02-shell/README.md`

- [ ] **Step 1: Add the Shell checklist entry point**

Add this near the top of `docs/task21-2/02-shell/README.md`:

```markdown
## 任务清单

开始执行前先打开：[task-list.md](task-list.md)。

Bash、Zsh、PowerShell 5.1 和 PowerShell 7 必须分别执行。脚本通过不代表真实 login chain、Profile/OneDrive、symlink、当前会话或插件顺序已经人工通过。
```

- [ ] **Step 2: Create formal Shell script checklist sections**

Create `02-shell/task-list.md` with separate sections:

1. `Linux Bash 脚本验证` — B-ENV-001, B-001, B-002, B-003, B-004, B-005, B-006; link `linux-bash.md` and `test/aliasmgr-bash-last-result/verification-summary.json`.
2. `Linux Bash 人工验证` — B-007, B-MANUAL-001, B-MANUAL-003, B-MANUAL-004; link exact manual sections and keep all non-PASS/actual-environment items unchecked.
3. `Linux Zsh 脚本验证` — Z-ENV-001, Z-001, Z-002, Z-003, Z-005, Z-006, Z-007; link `linux-zsh.md` and `test/aliasmgr-zsh-last-result/verification-summary.json`.
4. `Linux Zsh 人工验证` — Z-004, Z-MANUAL-001, Z-MANUAL-002, Z-MANUAL-003, Z-MANUAL-004; keep unchecked and show current `EXPECTED-LIMITATION` reason.
5. `Windows PowerShell 5.1 脚本验证` — PS51-001, PS51-002, PS51-006; link `windows-powershell51.md` and `test/aliasmgr-ps51-last-result/verification-summary.json`.
6. `Windows PowerShell 5.1 人工/限制验证` — PS51-003, PS51-004, PS51-005, PS51-007, PS51-008, PS51-009, PS51-LANG-001; keep unchecked and link the exact policy/Profile/encoding/argv sections.
7. `Windows PowerShell 7 脚本验证` — PS7-001, PS7-002, PS7-006; link `windows-powershell7.md` and `test/aliasmgr-ps7-last-result/verification-summary.json`.
8. `Windows PowerShell 7 人工/限制验证` — PS7-003, PS7-004, PS7-005, PS7-007, PS7-008, PS7-009, PS7-LANG-001; keep unchecked and link the exact sections.
9. `Windows Git Bash 探索性验证` — unchecked `NOT-APPLICABLE` or `EXPECTED-LIMITATION`; link `windows-git-bash.md` and state it does not replace Linux Bash.
10. `提交结果` — link `test-matrix.md` and `feedback-template.md`.

Use one checkbox per ID or clearly labeled grouped ID only when every ID has the same result and evidence. Current `PASS` cases from all four summaries are checked; current `EXPECTED-LIMITATION` cases remain unchecked.

- [ ] **Step 3: Record current Shell result metadata**

Add this at the top:

```markdown
当前结果来源：`test/aliasmgr-bash-last-result/`、`test/aliasmgr-zsh-last-result/`、`test/aliasmgr-ps51-last-result/`、`test/aliasmgr-ps7-last-result/`。
当前结果：Bash `7 PASS + 4 EXPECTED-LIMITATION`；Zsh `7 PASS + 5 EXPECTED-LIMITATION`；PS5.1 `3 PASS + 7 EXPECTED-LIMITATION`；PS7 `4 PASS + 6 EXPECTED-LIMITATION`。
```

- [ ] **Step 4: Verify Shell links, IDs, and checkbox rules**

Check that Bash/Zsh/PS5.1/PS7 detailed manuals and all four result summaries exist. Confirm the checklist does not check any `EXPECTED-LIMITATION`, `NOT-APPLICABLE`, or manual-only item. Do not run local Shell or PowerShell commands.

- [ ] **Step 5: Commit the Shell checklist**

```bash
git add docs/task21-2/02-shell/task-list.md docs/task21-2/02-shell/README.md
git commit -m "docs: add shell task checklist"
```

---

### Task 3: Final documentation-only review

**Files:**
- Review: `docs/task21-2/01-cli/task-list.md`
- Review: `docs/task21-2/02-shell/task-list.md`
- Review: `docs/task21-2/README.md`
- Review: `docs/task21-2/test-matrix.md`
- Review: `docs/task21-2/feedback-template.md`

- [ ] **Step 1: Verify all links and result references**

Confirm every relative manual link resolves, every evidence path points to an existing sanitized result file, and every checked item has a corresponding `PASS` case in that file.

- [ ] **Step 2: Verify no false manual completion**

Confirm all Profile, OneDrive, login-chain, symlink, current-session, oh-my-zsh, BOM/CRLF, native argv, and package/manual cases remain unchecked unless the user supplied evidence. Confirm exact result enums are preserved.

- [ ] **Step 3: Verify Markdown structure**

Confirm both README files link their task list before the platform manual table, both lists have script/manual headings, and all checkboxes are valid Markdown. Use only file reads/searches; no local runtime commands are needed.

- [ ] **Step 4: Commit the documentation set**

```bash
git add docs/task21-2/01-cli docs/task21-2/02-shell
 git commit -m "docs: organize task21-2 CLI and shell checklists"
```
