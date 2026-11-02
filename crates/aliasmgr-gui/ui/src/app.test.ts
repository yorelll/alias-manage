import test from "node:test";
import assert from "node:assert/strict";
import { NAV_ITEMS, navLabel, statusSummaryText, setTauriInvoke, tauriInvoke } from "./lib.ts";

test("approved sidebar navigation is ordered and complete in both languages", () => {
  assert.deepEqual(NAV_ITEMS.map((item) => item.id), ["aliases", "sync", "doctor", "settings"]);
  assert.deepEqual(NAV_ITEMS.map((item) => navLabel("zh", item.id)), ["别名", "同步", "诊断", "设置"]);
  assert.deepEqual(NAV_ITEMS.map((item) => navLabel("en", item.id)), ["Aliases", "Sync", "Doctor", "Settings"]);
});

test("status drawer summary covers loading, error, and detected shell", () => {
  assert.equal(statusSummaryText(null, false), "正在加载…");
  assert.equal(statusSummaryText(null, true), "状态不可用");
  assert.equal(statusSummaryText({ version: "0.1.0", detected_shell: "bash", detection_source: "shell_env", config_directory: "/tmp" }, false), "v0.1.0 · bash");
  assert.equal(statusSummaryText(null, false, "en"), "Loading…");
  assert.equal(statusSummaryText(null, true, "en"), "status unavailable");
});

test("startup command adapter passes the command and propagates status", async () => {
  const calls: string[] = [];
  const status = { version: "0.1.0", detected_shell: null, detection_source: "installed", config_directory: "/tmp" };
  setTauriInvoke(async (command) => { calls.push(command); return status; });
  assert.deepEqual(await tauriInvoke("startup_status"), status);
  assert.deepEqual(calls, ["startup_status"]);
});

test("startup command rejection remains representable as unavailable status", async () => {
  setTauriInvoke(async () => { throw new Error("unavailable"); });
  await assert.rejects(() => tauriInvoke("startup_status"), /unavailable/);
});
