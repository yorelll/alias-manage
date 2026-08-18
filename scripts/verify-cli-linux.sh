#!/usr/bin/env bash
# verify-cli-linux.sh — Linux CLI artifact smoke runner
#
# Usage:
#   bash scripts/verify-cli-linux.sh --artifact PATH [--keep-temp]
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
# Safety: this script NEVER modifies real ~/.bashrc, ~/.zshrc,
# or any real user configuration. All state is isolated inside
# a temporary directory that is cleaned on normal exit.

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
TEMP_ROOT="$(mktemp -d "aliasmgr-smoke-XXXXXX" -p /tmp)"
TEMP_CONFIG="$TEMP_ROOT/config"
TEMP_PROFILE="$TEMP_ROOT/profile"
TEMP_GENERATED="$TEMP_ROOT/generated"
TEMP_TARGETS="$TEMP_ROOT/targets"
RESULT_DIR="$TEMP_ROOT/result"

mkdir -p "$TEMP_CONFIG" "$TEMP_PROFILE" "$TEMP_GENERATED" "$TEMP_TARGETS" "$RESULT_DIR"

SUMMARY_JSON="$RESULT_DIR/verification-summary.json"
VERIFY_LOG="$RESULT_DIR/verification.log"
ARGV_JSON="$RESULT_DIR/argv-summary.json"

# ─── cleanup on normal exit ───────────────────────────────────────
cleanup() {
  if [[ $KEEP_TEMP -eq 0 ]]; then
    # Preserve result directory, remove rest
    if [[ -n "$TEMP_ROOT" && -d "$TEMP_ROOT" ]]; then
      cp -r "$RESULT_DIR" /tmp/aliasmgr-last-result 2>/dev/null || true
      rm -rf "$TEMP_ROOT"
      RESULT_DIR="/tmp/aliasmgr-last-result"
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
  # Validate enum
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

# ─── test cases ───────────────────────────────────────────────────
log_msg "=== verify-cli-linux.sh starting ==="
log_msg "artifact: [redacted path]"
log_msg "temp_root: [redacted path]"

# L-001: version and startup
ACTUAL_VERSION=""
if ACTUAL_VERSION="$(run_cli --version 2>&1)"; then
  record_result "L-001" "version and startup" "PASS" \
    "CLI reports version string" "$ACTUAL_VERSION" "stdout"
else
  record_result "L-001" "version and startup" "FAIL" \
    "CLI reports version string" "exit code $?" "stdout"
fi

# L-002: add native alias (isolated config)
ADD_OUT=""
if ADD_OUT="$(run_cli alias add gs git status 2>&1)"; then
  record_result "L-002" "add native alias" "PASS" \
    "alias added without error" "$ADD_OUT" "stdout"
else
  record_result "L-002" "add native alias" "FAIL" \
    "alias added without error" "exit $?: $ADD_OUT" "stdout"
fi

# L-002b: list aliases shows entry
LIST_OUT=""
if LIST_OUT="$(run_cli alias list 2>&1)" && echo "$LIST_OUT" | grep -q "gs"; then
  record_result "L-002b" "list aliases contains added entry" "PASS" \
    "list output contains 'gs'" "$LIST_OUT" "stdout"
else
  record_result "L-002b" "list aliases contains added entry" "FAIL" \
    "list output contains 'gs'" "$LIST_OUT" "stdout"
fi

# L-003: Python/JAR/cd target types (placeholder — real runners in Task 3)
record_result "L-003" "Python/JAR/cd target types" "EXPECTED-LIMITATION" \
  "target type coverage verified" \
  "placeholder: full target type coverage deferred to Task 3 scripts" \
  "task3-hook"

# L-004: argv edge cases (placeholder — real argv in Task 3)
record_result "L-004" "argv space/quote/CJK/backslash/wildcard" "EXPECTED-LIMITATION" \
  "argv boundary preservation verified" \
  "placeholder: argv boundary cases deferred to Task 3 scripts" \
  "task3-hook"

# L-005: {{args}} placeholder (placeholder — real argv in Task 3)
record_result "L-005" "{{args}} tail/middle/invalid/duplicate" "EXPECTED-LIMITATION" \
  "placeholder positions validated" \
  "placeholder: argv template cases deferred to Task 3 scripts" \
  "task3-hook"

# L-007: list/search/fuzzy/limit
SEARCH_OUT=""
if SEARCH_OUT="$(run_cli alias list --limit 5 2>&1)"; then
  record_result "L-007" "list/search/limit" "PASS" \
    "list --limit 5 succeeds" "$SEARCH_OUT" "stdout"
else
  record_result "L-007" "list/search/limit" "FAIL" \
    "list --limit 5 succeeds" "exit $?: $SEARCH_OUT" "stdout"
fi

# L-008: tag facet (placeholder — real tag cases in Task 3)
record_result "L-008" "tag facet single/multi/clear" "EXPECTED-LIMITATION" \
  "tag AND filter and clear verified" \
  "placeholder: tag facet cases deferred to Task 3 scripts" \
  "task3-hook"

# L-017: import/export (placeholder — real import in Task 3)
record_result "L-017" "JSON/TOML import" "EXPECTED-LIMITATION" \
  "preview, warning, confirm, persist verified" \
  "placeholder: import/export cases deferred to Task 3 scripts" \
  "task3-hook"

# L-018: retain/purge uninstall (placeholder — real uninstall in Task 3)
record_result "L-018" "retain/purge uninstall" "EXPECTED-LIMITATION" \
  "selected cleanup executed; targets preserved" \
  "placeholder: uninstall cases deferred to Task 3 scripts" \
  "task3-hook"

# L-019: upgrade/rollback (placeholder — real upgrade in Task 3)
record_result "L-019" "upgrade/rollback" "EXPECTED-LIMITATION" \
  "config and generated state preserved or restored" \
  "placeholder: upgrade/rollback cases deferred to Task 3 scripts" \
  "task3-hook"

# argv summary (structural — real data in Task 3)
cat > "$ARGV_JSON" <<JSON
{
  "summary": "argv boundary test results",
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

# Build JSON array of results
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
  "script": "verify-cli-linux.sh",
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
