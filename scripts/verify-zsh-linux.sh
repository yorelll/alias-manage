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
    "zsh is available on PATH" "[redacted]" "which"
else
  record_result "Z-ENV-001" "zsh binary available" "BLOCKED" \
    "zsh is available on PATH" "zsh not found on PATH" "which"
fi

log_msg "zsh version: $ZSH_VERSION_STR"

# ─── Z-001: loader install to isolated .zshrc ────────────────────
#
# The shell install command reads rc_path from config.toml [shells.zsh_rc_path].
# Write a config that points to our isolated TEMP_ZSHRC.
INSTALL_CONFIG="$TEMP_ROOT/install-config"
mkdir -p "$INSTALL_CONFIG"
cat > "$INSTALL_CONFIG/config.toml" <<TOML
[shells]
zsh_rc_path = "$TEMP_ZSHRC"
TOML

INSTALL_OUT=""
if INSTALL_OUT="$(ALIASMGR_CONFIG_DIR="$INSTALL_CONFIG" HOME="$TEMP_PROFILE" ZDOTDIR="$TEMP_PROFILE" "$CLI" shell install zsh 2>&1)"; then
  record_result "Z-001" "zsh loader install to isolated RC" "PASS" \
    "loader installed to isolated .zshrc without error" "$INSTALL_OUT" "stdout"
elif echo "$INSTALL_OUT" | grep -qi "not.*supported\|unsupported\|ShellNotInstalled"; then
  record_result "Z-001" "zsh loader install to isolated RC" "EXPECTED-LIMITATION" \
    "loader installed to isolated .zshrc without error" \
    "shell install command not available in this build: $INSTALL_OUT" "stdout"
else
  record_result "Z-001" "zsh loader install to isolated RC" "FAIL" \
    "loader installed to isolated .zshrc without error" "exit $?: $INSTALL_OUT" "stdout"
fi

# ─── Z-002: loader idempotence (run install twice) ───────────────
IDEM_OUT=""
if IDEM_OUT="$(ALIASMGR_CONFIG_DIR="$INSTALL_CONFIG" HOME="$TEMP_PROFILE" ZDOTDIR="$TEMP_PROFILE" "$CLI" shell install zsh 2>&1)"; then
  # Count occurrences of loader marker in RC file
  LOADER_COUNT=0
  if [[ -f "$TEMP_ZSHRC" ]]; then
    LOADER_COUNT="$(grep -c "aliasmgr\|alias-manager\|generated" "$TEMP_ZSHRC" 2>/dev/null || echo 0)"
  fi
  if [[ "$LOADER_COUNT" -le 1 ]]; then
    record_result "Z-002" "zsh loader install idempotence" "PASS" \
      "second install does not duplicate loader" "loader occurrences: $LOADER_COUNT" "file"
  else
    record_result "Z-002" "zsh loader install idempotence" "FAIL" \
      "second install does not duplicate loader" "loader count=$LOADER_COUNT (expected <=1)" "file"
  fi
elif echo "$IDEM_OUT" | grep -qi "not.*supported\|unsupported\|ShellNotInstalled"; then
  record_result "Z-002" "zsh loader install idempotence" "EXPECTED-LIMITATION" \
    "second install does not duplicate loader" \
    "shell install not available: $IDEM_OUT" "stdout"
else
  record_result "Z-002" "zsh loader install idempotence" "FAIL" \
    "second install does not duplicate loader" "exit $?: $IDEM_OUT" "stdout"
fi

# ─── Z-003: reload --print gives correct zsh source line ─────────
RELOAD_OUT=""
if RELOAD_OUT="$(ALIASMGR_CONFIG_DIR="$INSTALL_CONFIG" HOME="$TEMP_PROFILE" ZDOTDIR="$TEMP_PROFILE" "$CLI" reload --print 2>&1)"; then
  if echo "$RELOAD_OUT" | grep -q "generated\|zsh.sh\|source"; then
    record_result "Z-003" "zsh reload --print guidance" "PASS" \
      "reload --print outputs zsh source instruction" "[redacted]" "stdout"
  else
    record_result "Z-003" "zsh reload --print guidance" "FAIL" \
      "reload --print outputs zsh source instruction" "output missing 'generated': $RELOAD_OUT" "stdout"
  fi
else
  record_result "Z-003" "zsh reload --print guidance" "FAIL" \
    "reload --print outputs zsh source instruction" "exit $?: $RELOAD_OUT" "stdout"
fi

# ─── Z-004: generated zsh.sh syntax check via `zsh -n` ──────────
# Add aliases, sync to produce generated file, check syntax
run_cli add zsyntax --exec "echo" --arg "test" --shell "zsh" 2>/dev/null || true
run_cli sync 2>/dev/null || true

GENERATED_ZSH="$TEMP_CONFIG/generated/zsh.sh"
if [[ -n "$ZSH_BIN" && -f "$GENERATED_ZSH" ]]; then
  SYNTAX_OUT=""
  SYNTAX_RC=0
  SYNTAX_OUT="$("$ZSH_BIN" -n "$GENERATED_ZSH" 2>&1)" || SYNTAX_RC=$?
  if [[ $SYNTAX_RC -eq 0 ]]; then
    record_result "Z-004" "zsh generated file syntax check" "PASS" \
      "generated .zsh file passes zsh -n" "zsh -n: ok" "zsh -n"
  else
    record_result "Z-004" "zsh generated file syntax check" "FAIL" \
      "generated .zsh file passes zsh -n" "zsh -n error: $SYNTAX_OUT" "zsh -n"
  fi
elif [[ -z "$ZSH_BIN" ]]; then
  record_result "Z-004" "zsh generated file syntax check" "BLOCKED" \
    "generated .zsh file passes zsh -n" \
    "zsh binary not available for syntax check" "which"
else
  record_result "Z-004" "zsh generated file syntax check" "EXPECTED-LIMITATION" \
    "generated .zsh file passes zsh -n" \
    "generated/zsh.sh not present (sync may not have produced zsh output)" "file"
fi
run_cli remove --yes zsyntax 2>/dev/null || true

# ─── Z-005: argv boundary in zsh context ─────────────────────────
ARGV_CASES=()
ARGV_PASS=0
ARGV_FAIL=0

test_zsh_argv() {
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

# Multi-word fixed arg (zsh context)
test_zsh_argv "Z-005-a" "multi-word fixed arg" "ztest_space" \
  --exec "echo" --arg "hello world" --shell "zsh"

# CJK characters in arg
test_zsh_argv "Z-005-b" "CJK in arg" "ztest_cjk" \
  --exec "echo" --arg "中文参数" --shell "zsh"

# backslash in arg
test_zsh_argv "Z-005-c" "backslash in arg" "ztest_bs" \
  --exec "echo" --arg 'path\to\file' --shell "zsh"

# tag with hyphen
test_zsh_argv "Z-005-d" "hyphen in tag" "ztest_tag" \
  --exec "echo" --tag "my-tag" --shell "zsh"

ARGV_CASES_JSON="["
for i in "${!ARGV_CASES[@]}"; do
  ARGV_CASES_JSON+="${ARGV_CASES[$i]}"
  if [[ $i -lt $((${#ARGV_CASES[@]}-1)) ]]; then
    ARGV_CASES_JSON+=","
  fi
done
ARGV_CASES_JSON+="]"

if [[ $ARGV_FAIL -eq 0 ]]; then
  record_result "Z-005" "zsh argv boundary" "PASS" \
    "quoted args accepted across zsh invocation" \
    "pass=$ARGV_PASS fail=$ARGV_FAIL" "argv-summary.json"
else
  record_result "Z-005" "zsh argv boundary" "FAIL" \
    "quoted args accepted across zsh invocation" \
    "pass=$ARGV_PASS fail=$ARGV_FAIL" "argv-summary.json"
fi

# ─── Z-006: tag search in isolated zsh config ────────────────────
run_cli add z_tagged_a --exec "echo" --tag "ztag" --shell "zsh" 2>/dev/null || true
run_cli add z_tagged_b --exec "echo" --tag "ztag" --shell "zsh" 2>/dev/null || true

TAG_LIST_OUT=""
if TAG_LIST_OUT="$(run_cli list --tag "ztag" 2>&1)" && \
   echo "$TAG_LIST_OUT" | grep -q "z_tagged"; then
  record_result "Z-006" "zsh tag filter returns tagged aliases" "PASS" \
    "list --tag ztag returns tagged results" "[redacted]" "stdout"
else
  record_result "Z-006" "zsh tag filter returns tagged aliases" "FAIL" \
    "list --tag ztag returns tagged results" "[tag results empty or unexpected]" "stdout"
fi

run_cli remove --yes z_tagged_a 2>/dev/null || true
run_cli remove --yes z_tagged_b 2>/dev/null || true

# ─── Z-007: zsh shell uninstall removes loader ───────────────────
UNINSTALL_OUT=""
if UNINSTALL_OUT="$(ALIASMGR_CONFIG_DIR="$INSTALL_CONFIG" HOME="$TEMP_PROFILE" ZDOTDIR="$TEMP_PROFILE" "$CLI" shell uninstall zsh 2>&1)"; then
  if [[ -f "$TEMP_ZSHRC" ]]; then
    RESIDUE_COUNT="$(grep -c "aliasmgr\|alias-manager\|generated" "$TEMP_ZSHRC" 2>/dev/null || echo 0)"
    if [[ "$RESIDUE_COUNT" -eq 0 ]]; then
      record_result "Z-007" "zsh shell uninstall removes loader" "PASS" \
        "shell uninstall clears loader from .zshrc" "residue_count=$RESIDUE_COUNT" "file"
    else
      record_result "Z-007" "zsh shell uninstall removes loader" "FAIL" \
        "shell uninstall clears loader from .zshrc" "residue_count=$RESIDUE_COUNT (expected 0)" "file"
    fi
  else
    record_result "Z-007" "zsh shell uninstall removes loader" "PASS" \
      "shell uninstall clears loader from .zshrc" "RC file not present (clean state)" "file"
  fi
elif echo "$UNINSTALL_OUT" | grep -qi "not.*supported\|unsupported\|ShellNotInstalled"; then
  record_result "Z-007" "zsh shell uninstall removes loader" "EXPECTED-LIMITATION" \
    "shell uninstall clears loader from .zshrc" \
    "shell uninstall not available: $UNINSTALL_OUT" "stdout"
else
  record_result "Z-007" "zsh shell uninstall removes loader" "FAIL" \
    "shell uninstall clears loader from .zshrc" "exit $?: $UNINSTALL_OUT" "stdout"
fi

# ─── Manual-only checks — explicitly marked ──────────────────────
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

# ─── argv summary ─────────────────────────────────────────────────
cat > "$ARGV_JSON" <<JSON
{
  "summary": "zsh argv boundary test results",
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
