import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const app = await readFile(new URL("./App.tsx", import.meta.url), "utf8");

test("Task 20 panel command names are represented", () => {
  for (const command of ["doctor_status", "import_preview", "config_get", "uninstall_preview"]) {
    assert.match(app, new RegExp(command));
  }
});

test("Task 20 keeps destructive GUI actions explicit", () => {
  assert.match(app, /Confirm purge/);
  assert.match(app, /Confirm import/);
});

const componentNames = ["DoctorPanel", "ImportPanel", "SettingsPanel", "UninstallPanel"];
for (const name of componentNames) {
  test(`${name} exists in the GUI component tree`, async () => {
    const source = await readFile(new URL(`./components/${name}.tsx`, import.meta.url), "utf8");
    assert.match(source, new RegExp(name));
  });
}

