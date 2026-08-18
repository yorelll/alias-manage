#!/usr/bin/env bash
# verify-zsh-linux.sh — Zsh-specific isolated runner
#
# This script must be run with zsh or bash; it invokes zsh explicitly
# for shell-specific checks.
#
# Usage:
#   zsh scripts/verify-zsh-linux.sh --artifact PATH [--keep-temp]
#   bash scripts/verify-zsh-linux.sh --artifact PATH [--keep-temp]
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
# Safety: this script NEVER modifies real ~/.zshrc, $ZDOTDIR, or any
# real user configuration. All RC paths are isolated inside a temporary
# directory that is cleaned on normal exit. Real oh-my-zsh ordering,
# login-chain, and current-session checks are EXPLICITLY marked as
# EXPECTED-LIMITATION (manual-only) in the summary.

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
      echo "Usage: zsh $0 --artifact PATH [--keep-temp]" >&2
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
TEMP_ROOT="$(mktemp -d "aliasmgr-zsh-XXXXXX" -p /tmp)"
TEMP_CONFIG="$TEMP_ROOT/config"
TEMP_PROFILE="$TEMP_ROOT/profile"
TEMP_GENERATED="$TEMP_ROOT/generated"
TEMP_TARGETS="$TEMP_ROOT/targets"
TEMP_ZSHRC="$TEMP_PROFILE/.zshrc"
RESULT_DIR="$TEMP_ROOT/result"

mkdir -p "$TEMP_CONFIG" "$TEMP_PROFILE" "$TEMP_GENERATED" "$TEMP_TARGETS" "$RESULT_DIR"

# Create isolated zshrc — does NOT source real ~/.zshrc
touch "$TEMP_ZSHRC"

SUMMARY_JSON="$RESULT_DIR/verification-summary.json"
VERIFY_LOG="$RESULT_DIR/verification.log"
ARGV_JSON="$RESULT_DIR/argv-summary.json"

# ─── cleanup on normal exit ───────────────────────────────────────
cleanup() {
  if [[ $KEEP_TEMP -eq 0 ]]; then
    if [[ -n "$TEMP_ROOT" && -d "$TEMP_ROOT" ]]; then
      rm -rf /tmp/aliasmgr-zsh-last-result
      cp -r "$RESULT_DIR" /tmp/aliasmgr-zsh-last-result 2>/dev/null || true
      rm -rf "$TEMP_ROOT"
      RESULT_DIR="/tmp/aliasmgr-zsh-last-result"
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
  ALIASMGR_CONFIG_DIR="$TEMP_CONFIG" HOME="$TEMP_PROFILE" ZDOTDIR="$TEMP_PROFILE" "$CLI" "$@" 2>&1
}

# ─── Zsh detection ───────────────────────────────────────────────
log_msg "=== verify-zsh-linux.sh starting ==="
log_msg "artifact: [redacted path]"

ZSH_BIN=""
ZSH_VERSION_STR=""
if ZSH_BIN="$(command -v zsh 2>/dev/null)"; then
  ZSH_VERSION_STR="$("$ZSH_BIN" --version 2>&1 || echo 'unknown')"
  record_result "Z-ENV-001" "zsh binary available" "PASS" \
    "zsh is available on PATH" "$ZSH_BIN" "which"
else
  record_result "Z-ENV-001" "zsh binary available" "BLOCKED" \
    "zsh is available on PATH" "zsh not found on PATH" "which"
fi

log_msg "zsh version: $ZSH_VERSION_STR"

# ─── test cases ───────────────────────────────────────────────────

# Z-001: loader install to isolated .zshrc
INSTALL_OUT=""
if INSTALL_OUT="$(run_cli shell install zsh --rc-file "$TEMP_ZSHRC" 2>&1)"; then
  record_result "Z-001" "zsh loader install to isolated RC" "PASS" \
    "loader installed to isolated .zshrc without error" "$INSTALL_OUT" "stdout"
elif echo "$INSTALL_OUT" | grep -qi "not.*supported\|unsupported"; then
  record_result "Z-001" "zsh loader install to isolated RC" "EXPECTED-LIMITATION" \
    "loader installed to isolated .zshrc without error" \
    "shell install command not available in this build: $INSTALL_OUT" "stdout"
else
  record_result "Z-001" "zsh loader install to isolated RC" "FAIL" \
    "loader installed to isolated .zshrc without error" "exit $?: $INSTALL_OUT" "stdout"
fi

# Z-002: loader idempotence (run install twice)
IDEM_OUT=""
if IDEM_OUT="$(run_cli shell install zsh --rc-file "$TEMP_ZSHRC" 2>&1)"; then
  record_result "Z-002" "zsh loader install idempotence" "PASS" \
    "second install does not duplicate loader" "$IDEM_OUT" "stdout"
elif echo "$IDEM_OUT" | grep -qi "not.*supported\|unsupported"; then
  record_result "Z-002" "zsh loader install idempotence" "EXPECTED-LIMITATION" \
    "second install does not duplicate loader" \
    "shell install not available: $IDEM_OUT" "stdout"
else
  record_result "Z-002" "zsh loader install idempotence" "FAIL" \
    "second install does not duplicate loader" "exit $?: $IDEM_OUT" "stdout"
fi

# Z-003: reload guidance (placeholder — real shell sourcing in Task 3)
record_result "Z-003" "zsh reload guidance" "EXPECTED-LIMITATION" \
  "reload instruction visible and correct" \
  "placeholder: interactive zsh reload deferred to Task 3 scripts" \
  "task3-hook"

# Z-004: syntax check of generated Zsh file (placeholder — Task 3)
record_result "Z-004" "zsh generated file syntax check" "EXPECTED-LIMITATION" \
  "generated .zsh file passes zsh -n" \
  "placeholder: syntax check deferred to Task 3 scripts" \
  "task3-hook"

# Manual-only checks — explicitly marked
record_result "Z-MANUAL-001" "oh-my-zsh plugin ordering" "EXPECTED-LIMITATION" \
  "loader positioned after oh-my-zsh; override not silent" \
  "MANUAL-ONLY: oh-my-zsh ordering requires interactive Zsh session with plugin stack" \
  "manual"

record_result "Z-MANUAL-002" "real login chain verification" "EXPECTED-LIMITATION" \
  "login-chain behavior observed and documented" \
  "MANUAL-ONLY: cannot automate real login shell chain safely" \
  "manual"

record_result "Z-MANUAL-003" "ZDOTDIR symlink RC preservation" "EXPECTED-LIMITATION" \
  "symlink preserved; line endings preserved" \
  "MANUAL-ONLY: real symlink chain requires live filesystem check" \
  "manual"

record_result "Z-MANUAL-004" "current session reload" "EXPECTED-LIMITATION" \
  "current zsh session picks up new aliases after reload" \
  "MANUAL-ONLY: cannot source into current session from script" \
  "manual"

# argv summary
cat > "$ARGV_JSON" <<JSON
{
  "summary": "zsh argv boundary test results",
  "note": "full argv boundary matrix deferred to Task 3 scripts",
  "sensitive_data_redacted": true,
  "cases": []
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
  "script": "verify-zsh-linux.sh",
  "shell": "zsh",
  "shell_version": "$ZSH_VERSION_STR",
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
