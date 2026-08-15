import test from "node:test";
import assert from "node:assert/strict";
import { NAV_ITEMS, statusSummaryText, setTauriInvoke, tauriInvoke } from "./lib";

test("approved sidebar navigation is ordered and complete", () => {
  assert.deepEqual(NAV_ITEMS.map((item) => item.label), ["Aliases", "Sync", "Doctor", "Settings"]);
});

test("status drawer summary covers loading, error, and detected shell", () => {
  assert.equal(statusSummaryText(null, false), "loading…");
  assert.equal(statusSummaryText(null, true), "status unavailable");
  assert.equal(statusSummaryText({ version: "0.1.0", detected_shell: "bash", detection_source: "shell_env", config_directory: "/tmp" }, false), "v0.1.0 · bash");
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
