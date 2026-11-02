import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  DEFAULT_LANGUAGE,
  LANGUAGES,
  htmlLang,
  isLanguage,
  normalizeLanguage,
  t,
  translationIssues,
  translationKeys,
  untranslatedKeys,
} from "./i18n.ts";

test("Chinese is the default language and unknown values fall back to it", () => {
  assert.equal(DEFAULT_LANGUAGE, "zh");
  assert.equal(normalizeLanguage(undefined), "zh");
  assert.equal(normalizeLanguage("fr"), "zh");
  assert.equal(normalizeLanguage(""), "zh");
  assert.equal(normalizeLanguage("en"), "en");
  assert.equal(isLanguage("zh"), true);
  assert.equal(isLanguage("EN"), false);
  assert.equal(htmlLang("zh"), "zh-CN");
  assert.equal(htmlLang("en"), "en");
});

test("both dictionaries define every key with non-empty copy", () => {
  assert.deepEqual(translationIssues(), []);
  assert.ok(translationKeys().length > 100, "dictionary should cover the whole GUI");
});

test("no key is accidentally left untranslated", () => {
  assert.deepEqual(untranslatedKeys(), []);
});

test("the language selector offers Chinese and English", () => {
  assert.deepEqual(LANGUAGES.map((option) => option.value), ["zh", "en"]);
  assert.equal(t("zh", LANGUAGES[0].labelKey), "中文");
  assert.equal(t("en", LANGUAGES[1].labelKey), "English");
});

test("representative GUI strings switch with the selected language", () => {
  assert.equal(t("zh", "nav.aliases"), "别名");
  assert.equal(t("en", "nav.aliases"), "Aliases");
  assert.equal(t("zh", "aliases.title"), "别名");
  assert.equal(t("en", "aliases.title"), "Aliases");
  assert.equal(t("zh", "doctor.title"), "诊断");
  assert.equal(t("en", "doctor.title"), "Doctor");
  assert.equal(t("zh", "settings.save"), "保存设置");
  assert.equal(t("en", "settings.save"), "Save settings");
  assert.equal(t("zh", "table.confirmDelete"), "确认删除");
  assert.equal(t("en", "table.confirmDelete"), "Confirm delete");
  assert.equal(t("zh", "uninstall.confirmUninstall"), "确认卸载");
  assert.equal(t("en", "uninstall.confirmUninstall"), "Confirm uninstall");
  assert.equal(t("zh", "doctor.code.stale.title"), "生成文件已过期");
  assert.equal(t("en", "doctor.code.stale.title"), "Generated file out of date");
});

test("explanations for reload, target types, doctor, and the config directory exist in both languages", () => {
  for (const language of ["zh", "en"] as const) {
    for (const key of [
      "generated.reloadExplanation",
      "generated.reloadHover",
      "generated.reloadExplanationPowerShell",
      "generated.reloadExplanationPosix",
      "generated.reloadTitlePowerShell",
      "generated.reloadTitlePosix",
      "generated.reloadRequired",
      "generated.loaderMissing",
      "generated.missingFile",
      "generated.permissionFile",
      "wizard.targetTypeHint",
      "wizard.targetWhitespaceScript",
      "wizard.targetWhitespaceNative",
      "doctor.purpose",
      "doctor.purposeHover",
      "doctor.actionSync",
      "doctor.actionInstallLoader",
      "doctor.reloadAfterSync",
      "doctor.reloadAfterInstall",
      "settings.configDirHintBefore",
      "settings.configDirHintAfter",
      "settings.configDirExample",
      "settings.copyExample",
      "wizard.argvPreview",
      "wizard.argvUserArgs",
      "sync.error.permission",
      "sync.error.missing",
      "sync.error.io",
      "sync.error.config",
      "sync.error.database",
      "sync.error.locked",
      "sync.error.unsafe",
      "sync.error.failed",
      "sync.error.render",
      "loader.error.permission",
      "loader.error.missing",
      "loader.error.io",
      "loader.error.unsafe",
      "loader.error.unsupported",
      "loader.error.config",
      "loader.error.failed",
      "doctor.code.loader_unreadable.title",
      "doctor.code.loader_unreadable.message",
      "doctor.code.loader_unreadable.action",
      "doctor.code.loader_permission.title",
      "doctor.code.loader_io.title",
      "doctor.code.loader_unsafe.title",
      "doctor.code.config_invalid.title",
      "doctor.code.loader_shared_path.title",
      "generated.loaderError",
      "doctor.installAlreadyPresent",
      "doctor.installIncomplete",
    ] as const) {
      assert.ok(t(language, key).trim().length > 0, `${language}:${key} is empty`);
    }
  }
  assert.match(t("en", "generated.reloadExplanation"), /New shells/);
  assert.match(t("en", "generated.reloadExplanation"), /does not modify the database/);
  assert.match(t("zh", "generated.reloadExplanation"), /新 Shell/);
  assert.match(t("zh", "generated.reloadExplanation"), /不会修改数据库/);
  assert.match(t("en", "generated.reloadExplanationPowerShell"), /dot-source operator/);
  assert.match(t("zh", "generated.reloadExplanationPowerShell"), /点源/);
  assert.match(t("en", "doctor.purpose"), /does not repair anything automatically/);
  assert.match(t("zh", "doctor.purpose"), /不会自动修复/);
});

test("interpolation replaces named placeholders without touching literal braces", () => {
  assert.equal(t("en", "table.editAria", { name: "gs" }), "Edit gs");
  assert.equal(t("zh", "table.editAria", { name: "gs" }), "编辑 gs");
  assert.equal(t("en", "aliases.resultLimit", { limit: 200 }), "Showing first 200 results. Refine your search to see more.");
  assert.equal(t("zh", "aliases.resultLimit", { limit: 200 }), "仅显示前 200 条结果。请缩小搜索范围以查看更多。");
  assert.equal(t("en", "table.editAria"), "Edit {name}", "missing params leave the placeholder visible");
  assert.match(t("en", "wizard.fixedArgsHint"), /\{\{args\}\}/);
  assert.match(t("zh", "wizard.fixedArgsHint"), /\{\{args\}\}/);
  assert.match(t("en", "wizard.fixedArgsHint"), /\{\{\{\{args\}\}\}\}/);
  // Doubled braces are literal even when the caller passes an `args` param.
  assert.equal(t("en", "wizard.fixedArgsHint", { args: "X" }), t("en", "wizard.fixedArgsHint"));
  assert.equal(t("zh", "wizard.fixedArgsHint", { args: "X", name: "Y" }), t("zh", "wizard.fixedArgsHint"));
});

test("App loads the persisted language at startup and keeps Chinese on failure", async () => {
  const app = await readFile(new URL("./App.tsx", import.meta.url), "utf8");
  assert.match(app, /DEFAULT_LANGUAGE/);
  assert.match(app, /tauriInvoke\("config_get"\)/);
  assert.match(app, /normalizeSettings\(value as Settings\)\.language/);
  assert.match(app, /document\.documentElement\.lang = htmlLang\(language\)/);
  assert.match(app, /I18nContext\.Provider/);
  assert.match(app, /\.catch\(\(\) => undefined\)/);
});
