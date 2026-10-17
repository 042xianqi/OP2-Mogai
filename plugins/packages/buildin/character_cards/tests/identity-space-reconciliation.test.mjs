import assert from "node:assert/strict";
import test from "node:test";
import { createDiskHarness } from "./disk-files.mjs";
import { loadModule, plain } from "./runtime.mjs";

/** Opens the production database repository against the generic SQLite host adapter. */
async function open(disk) { return loadModule("src/storage/database.ts", disk.globals).DatabaseCharacterRepository.open(); }
/** Selects only actual storage transactions from the generic host audit. */
function commits(disk) { return disk.calls.filter(call => call.method === "storage.transaction"); }
/** Supplies an explicit memory creation command without fabricating a persisted memory. */
function memory(title) { return { title, content: "Persisted memory", contentType: "text/plain", source: "manual", credibility: 0.8, importance: 0.6, folderPath: null, tags: [] }; }

test("nextId above every stored identity stays untouched", () => {
  const { normalizeIdentitySpace } = loadModule("src/storage/state.ts");
  const state = { nextId: "9", cards: [{ id: "character-2" }], owners: [{ memories: [{ id: "5" }] }], conversationGroups: [{ id: "3" }] };
  assert.equal(normalizeIdentitySpace(state), false);
  assert.equal(state.nextId, "9");
});
test("a merged record at or above nextId raises it past the highest identity", () => {
  const { normalizeIdentitySpace } = loadModule("src/storage/state.ts");
  const state = {
    nextId: "1",
    cards: [{ id: "character-12" }], groups: [{ id: "group-4" }], tags: [{ id: "tag-3" }], stores: [{ id: "shared-2" }],
    conversationGroups: [{ id: "7" }],
    owners: [{ memories: [{ id: "5" }], links: [{ id: "9" }], chunks: [{ id: "2" }], candidates: [{ id: "3" }], embeddings: [{ id: "8" }], rebuildTask: { id: "4" } }],
  };
  assert.equal(normalizeIdentitySpace(state), true);
  assert.equal(state.nextId, "13");
});
test("an equal boundary still raises nextId and malformed counters stay for the validator", () => {
  const { normalizeIdentitySpace } = loadModule("src/storage/state.ts");
  const equal = { nextId: "10", owners: [{ memories: [{ id: "10" }] }] };
  assert.equal(normalizeIdentitySpace(equal), true);
  assert.equal(equal.nextId, "11");
  const malformed = { nextId: "not-a-decimal", owners: [{ memories: [{ id: "1" }] }] };
  assert.equal(normalizeIdentitySpace(malformed), false);
  assert.equal(malformed.nextId, "not-a-decimal");
  const empty = { nextId: "1", owners: [{ memories: [] }] };
  assert.equal(normalizeIdentitySpace(empty), false);
});

test("open raises nextId above a record merged from another device and persists it", async t => {
  const disk = await createDiskHarness(t), first = await open(disk);
  const created = plain(await first.run(session => session.createMemory("character:default", memory("Merged"))));
  assert.equal(created.id, "1");
  const state = plain(disk.readState());
  assert.equal(state.nextId, "2");
  disk.replaceState({ ...state, nextId: "1" });
  disk.clearCalls();
  const reopened = await open(disk);
  assert.equal(commits(disk).length, 1);
  const second = plain(await reopened.run(session => session.createMemory("character:default", memory("Second"))));
  assert.equal(second.id, "2");
  disk.clearCalls();
  await open(disk);
  assert.equal(commits(disk).length, 0);
});