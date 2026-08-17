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

test("Task 20 command surfaces are core-backed rather than empty placeholders", async () => {
  const commands = await readFile(new URL("../../src-tauri/src/commands.rs", import.meta.url), "utf8");
  assert.match(commands, /doctor_status[\s\S]*AppPaths/);
  assert.match(commands, /generated_preview[\s\S]*generated_path/);
  assert.match(commands, /import_preview[\s\S]*import_(json|toml)/);
  assert.match(commands, /config_save[\s\S]*save/);
  assert.doesNotMatch(commands, /doctor_status\(\)[\s\S]*Ok\(Vec::new\(\)\)/);
  assert.doesNotMatch(commands, /generated_preview\([^)]*\)[\s\S]*Ok\(String::new\(\)\)/);
});

test("Task 20 commands are registered with the Tauri builder", async () => {
  const main = await readFile(new URL("../../src-tauri/src/main.rs", import.meta.url), "utf8");
  for (const command of ["doctor_status", "generated_preview", "reload_command", "import_preview", "import_confirm", "config_get", "config_save", "uninstall_preview", "uninstall_confirm", "overridden_definitions"]) {
    assert.match(main, new RegExp(`commands::${command}`));
  }
});

const componentNames = ["DoctorPanel", "ImportPanel", "SettingsPanel", "UninstallPanel"];
for (const name of componentNames) {
  test(`${name} exists in the GUI component tree`, async () => {
    const source = await readFile(new URL(`./components/${name}.tsx`, import.meta.url), "utf8");
    assert.match(source, new RegExp(name));
  });
}

