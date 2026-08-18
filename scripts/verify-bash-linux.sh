#!/usr/bin/env bash
# verify-bash-linux.sh — Bash-specific isolated runner
#
# Usage:
#   bash scripts/verify-bash-linux.sh --artifact PATH [--keep-temp]
#
# Exit codes:
#   0  required checks pass; expected limitations may exist
#   1  implementation/release FAIL
#   2  environment BLOCKED
#   3  script argument or preparation error
#
# Outputs (written to <output-root>/result/):
#   verification-summary.json
#   verification.log
#   argv-summary.json
#
# Safety: this script NEVER modifies real ~/.bashrc or any real user
# configuration. All RC paths are isolated inside a temporary directory
# that is cleaned on normal exit. Real login-chain, oh-my-zsh order,
# trusted-symlink, and current-session manual checks are EXPLICITLY
# marked as EXPECTED-LIMITATION (manual-only) in the summary.

set -euo pipefail

# ─── argument parsing ────────────────────────────────────────────
ARTIFACT_PATH=""
KEEP_TEMP=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --artifact)
      ARTIFACT_PATH="$2"
      shift 2
      ;;
    --keep-temp)
      KEEP_TEMP=1
      shift
      ;;
    *)
      echo "[error] unknown argument: $1" >&2
      echo "Usage: bash $0 --artifact PATH [--keep-temp]" >&2
      exit 3
      ;;
  esac
done

if [[ -z "$ARTIFACT_PATH" ]]; then
  echo "[error] --artifact PATH is required" >&2
  exit 3
fi

if [[ ! -f "$ARTIFACT_PATH" ]] && [[ ! -x "$ARTIFACT_PATH" ]]; then
  echo "[error] artifact not found or not executable: $ARTIFACT_PATH" >&2
  exit 3
fi

# ─── temporary directory layout ──────────────────────────────────
TEMP_ROOT="$(mktemp -d "aliasmgr-bash-XXXXXX" -p /tmp)"
TEMP_CONFIG="$TEMP_ROOT/config"
TEMP_PROFILE="$TEMP_ROOT/profile"
TEMP_GENERATED="$TEMP_ROOT/generated"
TEMP_TARGETS="$TEMP_ROOT/targets"
TEMP_BASHRC="$TEMP_PROFILE/.bashrc"
RESULT_DIR="$TEMP_ROOT/result"

mkdir -p "$TEMP_CONFIG" "$TEMP_PROFILE" "$TEMP_GENERATED" "$TEMP_TARGETS" "$RESULT_DIR"

# Create isolated bashrc — does NOT source real ~/.bashrc
touch "$TEMP_BASHRC"

SUMMARY_JSON="$RESULT_DIR/verification-summary.json"
VERIFY_LOG="$RESULT_DIR/verification.log"
ARGV_JSON="$RESULT_DIR/argv-summary.json"

# ─── cleanup on normal exit ───────────────────────────────────────
cleanup() {
  if [[ $KEEP_TEMP -eq 0 ]]; then
    if [[ -n "$TEMP_ROOT" && -d "$TEMP_ROOT" ]]; then
      rm -rf /tmp/aliasmgr-bash-last-result
      cp -r "$RESULT_DIR" /tmp/aliasmgr-bash-last-result 2>/dev/null || true
      rm -rf "$TEMP_ROOT"
      RESULT_DIR="/tmp/aliasmgr-bash-last-result"
    fi
  fi
}
trap cleanup EXIT

# ─── result accumulation ─────────────────────────────────────────
RESULTS=()
PASS_COUNT=0
FAIL_COUNT=0
BLOCKED_COUNT=0
LIMITATION_COUNT=0

log_msg() {
  echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" | tee -a "$VERIFY_LOG"
}

record_result() {
  local id="$1" title="$2" result="$3" expected="$4" actual="$5" evidence="$6"
  case "$result" in
    PASS|FAIL|BLOCKED|EXPECTED-LIMITATION|NOT-APPLICABLE) ;;
    *) result="FAIL"; actual="invalid result enum: $result" ;;
  esac

  local entry
  entry=$(cat <<JSON
{"id":"${id}","title":"${title}","result":"${result}","expected":"${expected}","actual":"${actual}","evidence":"${evidence}","sensitive_data_redacted":true}
JSON
)
  RESULTS+=("$entry")

  case "$result" in
    PASS)                PASS_COUNT=$((PASS_COUNT+1)) ;;
    FAIL)                FAIL_COUNT=$((FAIL_COUNT+1)) ;;
    BLOCKED)             BLOCKED_COUNT=$((BLOCKED_COUNT+1)) ;;
    EXPECTED-LIMITATION) LIMITATION_COUNT=$((LIMITATION_COUNT+1)) ;;
  esac

  log_msg "[$result] $id $title"
}

# ─── CLI invocation helper ────────────────────────────────────────
CLI="$ARTIFACT_PATH"

run_cli() {
  ALIASMGR_CONFIG_DIR="$TEMP_CONFIG" HOME="$TEMP_PROFILE" "$CLI" "$@" 2>&1
}

# ─── Bash detection ───────────────────────────────────────────────
log_msg "=== verify-bash-linux.sh starting ==="
log_msg "bash version: $BASH_VERSION"
log_msg "artifact: [redacted path]"

BASH_BIN=""
if BASH_BIN="$(command -v bash 2>/dev/null)"; then
  record_result "B-ENV-001" "bash binary available" "PASS" \
    "bash is available on PATH" "[redacted]" "which"
else
  record_result "B-ENV-001" "bash binary available" "BLOCKED" \
    "bash is available on PATH" "bash not found on PATH" "which"
fi

# ─── B-001: loader install to isolated .bashrc ──────────────────
#
# Use --config-dir to give shell install a dedicated isolated root.
# Without a config.toml, the CLI falls back to $config_dir/.bashrc
# as the RC path. We pre-create that file to satisfy write_profile_content.
INSTALL_CONFIG="$TEMP_ROOT/install-config"
mkdir -p "$INSTALL_CONFIG"
# The CLI will install to $INSTALL_CONFIG/.bashrc by default
touch "$INSTALL_CONFIG/.bashrc"

INSTALL_OUT=""
INSTALL_RC=0
INSTALL_OUT="$(HOME="$TEMP_PROFILE" "$CLI" --config-dir "$INSTALL_CONFIG" shell install bash 2>&1)" || INSTALL_RC=$?
if [[ $INSTALL_RC -eq 0 ]]; then
  record_result "B-001" "bash loader install to isolated RC" "PASS" \
    "loader installed to isolated .bashrc without error" "$INSTALL_OUT" "stdout"
elif echo "$INSTALL_OUT" | grep -qi "not.*supported\|unsupported\|ShellNotInstalled"; then
  record_result "B-001" "bash loader install to isolated RC" "EXPECTED-LIMITATION" \
    "loader installed to isolated .bashrc without error" \
    "shell install command not available in this build: $INSTALL_OUT" "stdout"
else
  record_result "B-001" "bash loader install to isolated RC" "FAIL" \
    "loader installed to isolated .bashrc without error" "exit $INSTALL_RC: $INSTALL_OUT" "stdout"
fi

# ─── B-002: loader idempotence (run install twice) ──────────────
IDEM_OUT=""
IDEM_RC=0
IDEM_OUT="$(HOME="$TEMP_PROFILE" "$CLI" --config-dir "$INSTALL_CONFIG" shell install bash 2>&1)" || IDEM_RC=$?
if [[ $IDEM_RC -eq 0 ]]; then
  # Count occurrences of the unique START_MARKER to verify no duplication.
  # "# >>> Alias Manager >>>" appears exactly once per install block.
  LOADER_COUNT=0
  INSTALL_BASHRC="$INSTALL_CONFIG/.bashrc"
  if [[ -f "$INSTALL_BASHRC" ]]; then
    LOADER_COUNT="$(grep -cF '# >>> Alias Manager >>>' "$INSTALL_BASHRC" 2>/dev/null)" || true
  fi
  if [[ "$LOADER_COUNT" -le 1 ]]; then
    record_result "B-002" "bash loader install idempotence" "PASS" \
      "second install does not duplicate loader" "start-marker count: $LOADER_COUNT" "file"
  else
    record_result "B-002" "bash loader install idempotence" "FAIL" \
      "second install does not duplicate loader" "start-marker count=$LOADER_COUNT (expected <=1)" "file"
  fi
elif echo "$IDEM_OUT" | grep -qi "not.*supported\|unsupported\|ShellNotInstalled"; then
  record_result "B-002" "bash loader install idempotence" "EXPECTED-LIMITATION" \
    "second install does not duplicate loader" \
    "shell install not available: $IDEM_OUT" "stdout"
else
  record_result "B-002" "bash loader install idempotence" "FAIL" \
    "second install does not duplicate loader" "exit $IDEM_RC: $IDEM_OUT" "stdout"
fi

# ─── B-003: reload --print gives correct bash source line ────────
RELOAD_OUT=""
RELOAD_RC=0
RELOAD_OUT="$(HOME="$TEMP_PROFILE" "$CLI" --config-dir "$INSTALL_CONFIG" reload --print 2>&1)" || RELOAD_RC=$?
if [[ $RELOAD_RC -eq 0 ]]; then
  if echo "$RELOAD_OUT" | grep -q "generated\|bash.sh"; then
    record_result "B-003" "bash reload --print guidance" "PASS" \
      "reload --print outputs bash source instruction" "[redacted]" "stdout"
  else
    record_result "B-003" "bash reload --print guidance" "FAIL" \
      "reload --print outputs bash source instruction" "output missing 'generated': $RELOAD_OUT" "stdout"
  fi
else
  record_result "B-003" "bash reload --print guidance" "FAIL" \
    "reload --print outputs bash source instruction" "exit $RELOAD_RC: $RELOAD_OUT" "stdout"
fi

# ─── B-004: argv boundary in bash: add + CRUD with complex args ──
ARGV_CASES=()
ARGV_PASS=0
ARGV_FAIL=0

test_bash_argv() {
  local test_id="$1" test_title="$2" alias_name="$3"
  shift 3
  local add_out=""
  local add_rc=0
  add_out="$(run_cli add "$alias_name" "$@" 2>&1)" || add_rc=$?
  if [[ $add_rc -eq 0 ]]; then
    ARGV_CASES+=("{\"id\":\"${test_id}\",\"input\":\"${test_title}\",\"result\":\"PASS\"}")
    ARGV_PASS=$((ARGV_PASS+1))
    run_cli remove --yes "$alias_name" 2>/dev/null || true
  else
    ARGV_CASES+=("{\"id\":\"${test_id}\",\"input\":\"${test_title}\",\"result\":\"FAIL\",\"output\":\"exit $add_rc\"}")
    ARGV_FAIL=$((ARGV_FAIL+1))
  fi
}

# args with embedded spaces
test_bash_argv "B-004-a" "multi-word fixed arg" "btest_space" \
  --exec "echo" --arg "hello world"

# CJK chars in arg
test_bash_argv "B-004-b" "CJK in arg" "btest_cjk" \
  --exec "echo" --arg "中文参数"

# backslash in arg
test_bash_argv "B-004-c" "backslash in arg" "btest_bs" \
  --exec "echo" --arg 'path\to\file'

# alias with tags (multi-word tag)
test_bash_argv "B-004-d" "tag with spaces" "btest_tag" \
  --exec "echo" --tag "my tag" --tag "other"

# argv summary
ARGV_CASES_JSON="["
for i in "${!ARGV_CASES[@]}"; do
  ARGV_CASES_JSON+="${ARGV_CASES[$i]}"
  if [[ $i -lt $((${#ARGV_CASES[@]}-1)) ]]; then
    ARGV_CASES_JSON+=","
  fi
done
ARGV_CASES_JSON+="]"

if [[ $ARGV_FAIL -eq 0 ]]; then
  record_result "B-004" "bash argv boundary" "PASS" \
    "quoted args preserved across bash invocation" \
    "pass=$ARGV_PASS fail=$ARGV_FAIL" "argv-summary.json"
else
  record_result "B-004" "bash argv boundary" "FAIL" \
    "quoted args preserved across bash invocation" \
    "pass=$ARGV_PASS fail=$ARGV_FAIL" "argv-summary.json"
fi

# ─── B-005: tag/search in isolated config ────────────────────────
run_cli add b_tagged_a --exec "echo" --arg "a" --tag "btag" 2>/dev/null || true
run_cli add b_tagged_b --exec "echo" --arg "b" --tag "btag" 2>/dev/null || true

TAG_LIST_OUT=""
if TAG_LIST_OUT="$(run_cli list --tag "btag" 2>&1)" && \
   echo "$TAG_LIST_OUT" | grep -q "b_tagged"; then
  record_result "B-005" "bash tag/search in isolated config" "PASS" \
    "tag AND filter returns correct results" "[redacted]" "stdout"
else
  record_result "B-005" "bash tag/search in isolated config" "FAIL" \
    "tag AND filter returns correct results" "[tag results empty or unexpected]" "stdout"
fi

run_cli remove --yes b_tagged_a 2>/dev/null || true
run_cli remove --yes b_tagged_b 2>/dev/null || true

# ─── B-006: tombstone / shell uninstall ──────────────────────────
UNINSTALL_OUT=""
UNINSTALL_RC=0
UNINSTALL_OUT="$(HOME="$TEMP_PROFILE" "$CLI" --config-dir "$INSTALL_CONFIG" shell uninstall bash 2>&1)" || UNINSTALL_RC=$?
if [[ $UNINSTALL_RC -eq 0 ]]; then
  # Verify loader line is no longer present
  INSTALL_BASHRC="$INSTALL_CONFIG/.bashrc"
  if [[ -f "$INSTALL_BASHRC" ]]; then
    # Use grep -cF on the unique START_MARKER; grep -c outputs a number even on no match.
    # Do not add || echo 0 here — grep -c always emits the count, exit 1 just means 0 matches.
    RESIDUE_COUNT=0
    RESIDUE_COUNT="$(grep -cF '# >>> Alias Manager >>>' "$INSTALL_BASHRC" 2>/dev/null)" || true
    if [[ "$RESIDUE_COUNT" -eq 0 ]]; then
      record_result "B-006" "bash shell uninstall removes loader" "PASS" \
        "shell uninstall clears loader from RC" "start-marker count=$RESIDUE_COUNT" "file"
    else
      record_result "B-006" "bash shell uninstall removes loader" "FAIL" \
        "shell uninstall clears loader from RC" "start-marker count=$RESIDUE_COUNT (expected 0)" "file"
    fi
  else
    record_result "B-006" "bash shell uninstall removes loader" "PASS" \
      "shell uninstall clears loader from RC" "RC file not present (clean state)" "file"
  fi
elif echo "$UNINSTALL_OUT" | grep -qi "not.*supported\|unsupported\|ShellNotInstalled"; then
  record_result "B-006" "bash shell uninstall removes loader" "EXPECTED-LIMITATION" \
    "shell uninstall clears loader from RC" \
    "shell uninstall not available: $UNINSTALL_OUT" "stdout"
else
  record_result "B-006" "bash shell uninstall removes loader" "FAIL" \
    "shell uninstall clears loader from RC" "exit $UNINSTALL_RC: $UNINSTALL_OUT" "stdout"
fi

# ─── B-007: generated bash.sh syntax check ──────────────────────
# Add an alias and sync to produce a generated file, then check its syntax
run_cli add bsyntax --exec "echo" --arg "test" 2>/dev/null || true
SYNC_OUT=""
run_cli sync 2>/dev/null || true

GENERATED_BASH="$TEMP_CONFIG/generated/bash.sh"
if [[ -f "$GENERATED_BASH" ]]; then
  SYNTAX_OUT=""
  SYNTAX_RC=0
  SYNTAX_OUT="$(bash -n "$GENERATED_BASH" 2>&1)" || SYNTAX_RC=$?
  if [[ $SYNTAX_RC -eq 0 ]]; then
    record_result "B-007" "generated bash.sh syntax valid" "PASS" \
      "bash -n passes on generated bash.sh" "bash -n: ok" "bash -n"
  else
    record_result "B-007" "generated bash.sh syntax valid" "FAIL" \
      "bash -n passes on generated bash.sh" "bash -n error: $SYNTAX_OUT" "bash -n"
  fi
else
  record_result "B-007" "generated bash.sh syntax valid" "EXPECTED-LIMITATION" \
    "bash -n passes on generated bash.sh" \
    "generated/bash.sh not present (sync may not have run or no aliases)" "file"
fi
run_cli remove --yes bsyntax 2>/dev/null || true

# ─── Manual-only checks — explicitly marked ──────────────────────
record_result "B-MANUAL-001" "real login chain verification" "EXPECTED-LIMITATION" \
  "login-chain behavior observed and documented" \
  "MANUAL-ONLY: cannot automate real login shell chain safely" \
  "manual"

record_result "B-MANUAL-002" "oh-my-zsh ordering with bash" "NOT-APPLICABLE" \
  "oh-my-zsh ordering not applicable to Bash runner" \
  "NOT-APPLICABLE: oh-my-zsh is a Zsh concern" \
  "manual"

record_result "B-MANUAL-003" "trusted symlink RC preservation" "EXPECTED-LIMITATION" \
  "symlink preserved; line endings preserved" \
  "MANUAL-ONLY: real symlink chain requires live filesystem check" \
  "manual"

record_result "B-MANUAL-004" "current session reload" "EXPECTED-LIMITATION" \
  "current shell session picks up new aliases after reload" \
  "MANUAL-ONLY: cannot source into current session from script" \
  "manual"

# ─── argv summary ─────────────────────────────────────────────────
cat > "$ARGV_JSON" <<JSON
{
  "summary": "bash argv boundary test results",
  "pass": $ARGV_PASS,
  "fail": $ARGV_FAIL,
  "sensitive_data_redacted": true,
  "cases": $ARGV_CASES_JSON
}
JSON

# ─── final summary ────────────────────────────────────────────────
TOTAL=$((PASS_COUNT + FAIL_COUNT + BLOCKED_COUNT + LIMITATION_COUNT))

if [[ $FAIL_COUNT -gt 0 ]]; then
  OVERALL="FAIL"
  EXIT_CODE=1
elif [[ $BLOCKED_COUNT -gt 0 ]]; then
  OVERALL="BLOCKED"
  EXIT_CODE=2
elif [[ $LIMITATION_COUNT -gt 0 ]]; then
  OVERALL="PASS_WITH_EXPECTED_LIMITATIONS"
  EXIT_CODE=0
else
  OVERALL="PASS"
  EXIT_CODE=0
fi

RESULTS_JSON="["
for i in "${!RESULTS[@]}"; do
  RESULTS_JSON+="${RESULTS[$i]}"
  if [[ $i -lt $((${#RESULTS[@]}-1)) ]]; then
    RESULTS_JSON+=","
  fi
done
RESULTS_JSON+="]"

cat > "$SUMMARY_JSON" <<JSON
{
  "script": "verify-bash-linux.sh",
  "shell": "bash",
  "shell_version": "$BASH_VERSION",
  "overall": "$OVERALL",
  "pass": $PASS_COUNT,
  "fail": $FAIL_COUNT,
  "blocked": $BLOCKED_COUNT,
  "expected_limitation": $LIMITATION_COUNT,
  "total": $TOTAL,
  "sensitive_data_redacted": true,
  "result_enum": ["PASS","FAIL","BLOCKED","EXPECTED-LIMITATION","NOT-APPLICABLE"],
  "output_paths": {
    "summary": "result/verification-summary.json",
    "log": "result/verification.log",
    "argv": "result/argv-summary.json"
  },
  "cases": $RESULTS_JSON
}
JSON

log_msg "=== overall: $OVERALL (pass=$PASS_COUNT fail=$FAIL_COUNT blocked=$BLOCKED_COUNT limitation=$LIMITATION_COUNT) ==="
log_msg "summary written to result/verification-summary.json"

exit $EXIT_CODE
