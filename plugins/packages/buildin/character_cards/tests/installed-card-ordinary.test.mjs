import assert from "node:assert/strict";
import test from "node:test";
import { createDiskHarness } from "./disk-files.mjs";
import { loadModule, plain } from "./runtime.mjs";

/** Opens the production database repository against the generic SQLite host adapter. */
async function open(disk) { return loadModule("src/storage/database.ts", disk.globals).DatabaseCharacterRepository.open(); }
/** Selects only actual storage transactions from the generic host audit. */
function commits(disk) { return disk.calls.filter(call => call.method === "storage.transaction"); }
/** Builds one complete new-card draft from the installation card without reusing its identity. */
function draft(state, name, isDefault) {
  return { ...plain(state.cards[0]), id: "", name, isDefault };
}
/** Builds one legacy replicated card and its real owner space. */
function legacyCard(state, id, createdAt, isDefault) {
  const ownerKey = "character:" + id;
  return {
    card: { ...plain(state.cards[0]), id, name: "Legacy " + id, isDefault, createdAt, updatedAt: createdAt },
    owner: { ...plain(state.owners[0]), ownerKey, userDocumentPath: "owners/" + encodeURIComponent(ownerKey) + "/USER.md" },
  };
}

test("installation seeds an ordinary card with no default flag", async t => {
  const disk = await createDiskHarness(t), repository = await open(disk);
  const state = plain(disk.readState());
  assert.equal(state.cards.length, 1);
  assert.equal(state.cards[0].id, "default");
  assert.equal(state.cards[0].isDefault, false);
  const cards = plain(await repository.run(session => session.listCharacters()));
  assert.deepEqual(cards.map(card => [card.id, card.isDefault]), [["default", false]]);
});

test("new cards cannot become defaults and the seeded card follows ordinary deletion rules", async t => {
  const disk = await createDiskHarness(t), repository = await open(disk);
  const state = plain(disk.readState());
  const created = plain(await repository.run(session => session.createCharacter(draft(state, "Second", true))));
  assert.notEqual(created.id, "default");
  assert.equal(created.isDefault, false);
  await repository.run(session => session.writeActive({ CharacterCard: { id: created.id } }));
  await repository.run(session => session.deleteCharacter("default"));
  assert.deepEqual(plain(await repository.run(session => session.listCharacters())).map(card => card.id), [created.id]);
  assert.equal(plain(disk.readState()).owners.some(owner => owner.ownerKey === "character:default"), false);
  const reopened = await open(disk);
  assert.deepEqual(plain(await reopened.run(session => session.listCharacters())).map(card => card.id), [created.id]);
  await assert.rejects(reopened.run(session => session.deleteCharacter(created.id)), /Select another character/);
});

test("legacy replicated cards may retain their historical default flag without blocking open", async t => {
  const disk = await createDiskHarness(t); await open(disk);
  const state = plain(disk.readState());
  const legacy = legacyCard(state, "1", state.cards[0].createdAt - 1000, true);
  disk.replaceState({ ...state, cards: [...state.cards, legacy.card], owners: [...state.owners, legacy.owner] });
  disk.clearCalls();
  const reopened = await open(disk);
  assert.equal(commits(disk).length, 0);
  const cards = plain(await reopened.run(session => session.listCharacters()));
  assert.deepEqual(cards.map(card => [card.id, card.isDefault]).sort(), [["1", true], ["default", false]]);
});
