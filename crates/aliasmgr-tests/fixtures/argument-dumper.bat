@echo off
setlocal DisableDelayedExpansion
powershell -NoProfile -NonInteractive -Command "$args = $env:ALIASMGR_BATCH_ARGS; [Console]::Out.WriteLine($args)"
