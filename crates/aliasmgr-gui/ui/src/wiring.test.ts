import test from "node:test";
import assert from "node:assert/strict";
import {
  NEW_ALIAS_ID,
  createAliasPayload,
  emptyAliasDraft,
  mergeEditedAlias,
  searchAliasesArgs,
  toggleAliasEnabled,
  toggleTag,
  usesFilteredSearch,
} from "./lib.ts";

function baseAlias() {
  return {
    id: "11111111-1111-1111-1111-111111111111",
    name: "gs",
    description: "git status",
    executable: "git",
    target_type: "native_executable",
    fixed_args: ["status"],
    pass_args: false,
    working_directory: null,
    environment: {},
    shells: ["bash"],
    enabled: true,
    tags: ["git"],
    revision: 4,
    path_mode: "relative_to_working_directory",
    path_origin: "cli_argument",
  };
}

test("tag facet toggling adds and removes without mutating the input", () => {
  const selected = ["git"];
  assert.deepEqual(toggleTag(selected, "work"), ["git", "work"]);
  assert.deepEqual(toggleTag(selected, "git"), []);
  assert.deepEqual(selected, ["git"]);
});

test("empty draft uses the create placeholder and absolute gui-picked defaults", () => {
  const draft = emptyAliasDraft();
  assert.equal(draft.id, NEW_ALIAS_ID);
  assert.equal(draft.path_mode, "absolute");
  assert.equal(draft.path_origin, "gui_file_picker");
  assert.equal(draft.enabled, true);
  assert.deepEqual(draft.shells, ["bash"]);
});

test("create payload resets the id placeholder and normalizes tags", () => {
  const draft = { ...baseAlias(), id: "stale-id", tags: [" git ", "git", "", "work"] };
  const payload = createAliasPayload(draft);
  assert.equal(payload.id, NEW_ALIAS_ID);
  assert.deepEqual(payload.tags, ["git", "work"]);
  assert.equal(payload.name, "gs");
  assert.equal(payload.executable, "git");
});

test("editing merges onto the stored identity and path provenance", () => {
  const original = baseAlias();
  const edited = {
    ...baseAlias(),
    id: NEW_ALIAS_ID,
    revision: 999,
    name: "gst",
    executable: "./git",
    tags: ["git", " vcs "],
    path_mode: null,
    path_origin: null,
  };
  const merged = mergeEditedAlias(original, edited);
  assert.equal(merged.id, original.id);
  assert.equal(merged.revision, original.revision);
  assert.equal(merged.path_mode, "relative_to_working_directory");
  assert.equal(merged.path_origin, "cli_argument");
  assert.equal(merged.name, "gst");
  assert.equal(merged.executable, "./git");
  assert.deepEqual(merged.tags, ["git", "vcs"]);
  assert.equal(original.name, "gs");
});

test("enable toggle flips only the enabled flag", () => {
  const alias = baseAlias();
  const toggled = toggleAliasEnabled(alias);
  assert.equal(toggled.enabled, false);
  assert.equal(alias.enabled, true);
  assert.equal(toggled.id, alias.id);
  assert.equal(toggled.revision, alias.revision);
});

test("search args match the Tauri request envelope", () => {
  assert.deepEqual(searchAliasesArgs("git", true, 50, ["work", "git"]), {
    request: { query: "git", fuzzy: true, limit: 50, tag_filter: ["work", "git"] },
  });
});

test("filtered search is only used when a query or tag filter is present", () => {
  assert.equal(usesFilteredSearch("", []), false);
  assert.equal(usesFilteredSearch("   ", []), false);
  assert.equal(usesFilteredSearch("git", []), true);
  assert.equal(usesFilteredSearch("", ["work"]), true);
});
