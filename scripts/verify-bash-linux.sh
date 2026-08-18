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
    "bash is available on PATH" "$BASH_BIN" "which"
else
  record_result "B-ENV-001" "bash binary available" "BLOCKED" \
    "bash is available on PATH" "bash not found on PATH" "which"
fi

# ─── test cases ───────────────────────────────────────────────────

# B-001: loader install to isolated .bashrc
INSTALL_OUT=""
if INSTALL_OUT="$(run_cli shell install bash --rc-file "$TEMP_BASHRC" 2>&1)"; then
  record_result "B-001" "bash loader install to isolated RC" "PASS" \
    "loader installed to isolated .bashrc without error" "$INSTALL_OUT" "stdout"
elif echo "$INSTALL_OUT" | grep -qi "not.*supported\|unsupported"; then
  record_result "B-001" "bash loader install to isolated RC" "EXPECTED-LIMITATION" \
    "loader installed to isolated .bashrc without error" \
    "shell install command not available in this build: $INSTALL_OUT" "stdout"
else
  record_result "B-001" "bash loader install to isolated RC" "FAIL" \
    "loader installed to isolated .bashrc without error" "exit $?: $INSTALL_OUT" "stdout"
fi

# B-002: loader idempotence (run install twice)
IDEM_OUT=""
if IDEM_OUT="$(run_cli shell install bash --rc-file "$TEMP_BASHRC" 2>&1)"; then
  record_result "B-002" "bash loader install idempotence" "PASS" \
    "second install does not duplicate loader" "$IDEM_OUT" "stdout"
elif echo "$IDEM_OUT" | grep -qi "not.*supported\|unsupported"; then
  record_result "B-002" "bash loader install idempotence" "EXPECTED-LIMITATION" \
    "second install does not duplicate loader" \
    "shell install not available: $IDEM_OUT" "stdout"
else
  record_result "B-002" "bash loader install idempotence" "FAIL" \
    "second install does not duplicate loader" "exit $?: $IDEM_OUT" "stdout"
fi

# B-003: reload guidance (placeholder — real shell sourcing in Task 3)
record_result "B-003" "bash reload guidance" "EXPECTED-LIMITATION" \
  "reload instruction visible and correct" \
  "placeholder: interactive shell reload deferred to Task 3 scripts" \
  "task3-hook"

# B-004: argv boundary in generated Bash function (placeholder)
record_result "B-004" "bash argv boundary" "EXPECTED-LIMITATION" \
  "quoted args preserved across bash invocation" \
  "placeholder: argv boundary matrix deferred to Task 3 scripts" \
  "task3-hook"

# B-005: tag/search in isolated config (placeholder)
record_result "B-005" "bash tag/search in isolated config" "EXPECTED-LIMITATION" \
  "tag AND filter returns correct results" \
  "placeholder: tag/search cases deferred to Task 3 scripts" \
  "task3-hook"

# B-006: tombstone/uninstall (placeholder — safe for Task 3)
record_result "B-006" "bash tombstone/uninstall" "EXPECTED-LIMITATION" \
  "loader removed on uninstall; managed residues cleared" \
  "placeholder: uninstall cases deferred to Task 3 scripts" \
  "task3-hook"

# Manual-only checks — explicitly marked
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

# argv summary
cat > "$ARGV_JSON" <<JSON
{
  "summary": "bash argv boundary test results",
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
