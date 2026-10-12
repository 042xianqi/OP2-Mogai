import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {DeviceView} from '../web/device-view.ts';
import {simulatorViewState} from '../web/device-state.ts';
import {ScenePreview} from '../web/scene-renderer.ts';

async function fixture() {
  globalThis.window ??= {};
  const {default: create} = await import('../generated/ui.mjs');
  const ui = await create({wasmBinary: await readFile(new URL('../generated/ui.wasm', import.meta.url))});
  assert.equal(ui._simulator_init(), 1);
  const actions = [], receipts = [];
  ui.onAction = action => actions.push(action);
  const view = new DeviceView(ui, new ScenePreview(), (...result) => receipts.push(result));
  const inspect = () => JSON.parse(ui.ccall('operit_ui_debug_snapshot', 'string', [], []));
  return {ui, view, inspect, actions, receipts};
}

test('browser adapter forwards history cursors to shared C UI lazy paging', async () => {
  const {ui, view, actions, inspect} = await fixture();
  view.update(simulatorViewState({ready: true, device: {sessionId: 'one', paired: true,
    chat: {connected: true, hasOlder: true, hasNewer: false,
      messages: Array.from({length: 12}, (_, i) => ({sender: 'user', text: '消息 ' + i}))}}}));
  assert(inspect().chatScrollLine > 0);
  for (let i = 0; i < 8; i++) ui.ccall('operit_ui_debug_swipe', 'number', ['string'], ['down']);
  assert(actions.includes('edge_history_older'), 'history flag must reach C, not stop at browser state');
  actions.length = 0;
  view.update(simulatorViewState({ready: true, device: {sessionId: 'one', paired: true,
    chat: {connected: true, hasOlder: false, hasNewer: false}}}));
  for (let i = 0; i < 8; i++) ui.ccall('operit_ui_debug_swipe', 'number', ['string'], ['down']);
  assert.deepEqual(actions, []);
});

test('stopped device clears C pairing, approval, chat and plugin state without old data', async () => {
  const {ui, view, inspect} = await fixture();
  view.update(simulatorViewState({ready: true, device: {sessionId: 'one', paired: true,
    pairingCode: '123456', spaceJoinPrompt: '申请加入空间',
    plugins: {items: [{id: 'old', name: '旧插件'}]},
    chat: {connected: true, messages: [{sender: 'user', text: 'OLD_MESSAGE'}]}}}));
  ui.ccall('operit_ui_set_chat_draft', null, ['string'], ['保留草稿']);
  assert.equal(inspect().page, 'Pairing');
  view.update(simulatorViewState({ready: false, device: null}));
  const stopped = inspect();
  assert.equal(stopped.pairingCode, '');
  assert(!['Pairing', 'Space', 'edge_scene'].includes(stopped.page));
  assert(!stopped.nodes.some(node => ['edge_space_approve', 'edge_space_reject'].includes(node.id)));
  assert(!JSON.stringify(stopped).includes('OLD_MESSAGE'));
  assert(!JSON.stringify(stopped).includes('旧插件'));
  assert.equal(ui.ccall('operit_ui_debug_tap', 'number', ['string'], ['sidebar_toggle']), 1);
  assert.equal(ui.ccall('operit_ui_debug_tap', 'number', ['string'], ['sidebar_settings']), 1);
  assert.equal(inspect().nodes.find(node => node.id === 'link_status').text, '未配对');
  ui._operit_ui_set_connection(1, 1);
  ui.ccall('operit_ui_submit_chat', null, [], []);
  assert.equal(ui.ccall('operit_ui_chat_draft', 'string', [], []), '保留草稿');
});

test('each browser sees a receipt once and accepts reused receipt ids in a new process', async () => {
  const first = await fixture(), second = await fixture();
  const state = simulatorViewState({ready: true, device: {sessionId: 'one',
    chatSendResult: {id: 1, sendId: 'browser-send', ok: true}}});
  for (const browser of [first, second]) {
    browser.view.update(state); browser.view.update(state);
    assert.deepEqual(browser.receipts, [[true, undefined, 'browser-send']]);
    browser.view.update({...state, sessionId: 'two'});
    assert.equal(browser.receipts.length, 2);
  }
});

test('stop invalidates pending scene fetch and a restarted process can reuse a scene revision', async t => {
  const original = globalThis.fetch;
  t.after(() => { globalThis.fetch = original; });
  let resolve;
  let calls = 0;
  globalThis.fetch = () => { calls++; return new Promise(done => { resolve = done; }); };
  const {ui, view, inspect} = await fixture();
  const active = sessionId => ({sessionId, running: true, scene: {active: true, revision: 1}});
  view.update(active('one'));
  assert.equal(calls, 1);
  view.update({running: false});
  resolve({ok: true, json: async () => ({active: true, revision: 1, elapsed: 0, packs: [], layers: []})});
  await new Promise(done => setImmediate(done));
  assert.notEqual(inspect().page, 'edge_scene');
  view.update(active('two'));
  assert.equal(calls, 2);
  resolve({ok: true, json: async () => ({active: true, revision: 1, elapsed: 0, packs: [], layers: []})});
  await new Promise(done => setImmediate(done));
  assert.equal(inspect().page, 'edge_scene');
  // Miss the stopped poll: identical summary still fetches the new process's scene.
  view.update(active('three'));
  assert.equal(calls, 3);
  view.update({running: false});
  assert.notEqual(inspect().page, 'edge_scene');
});

test('scene preview retries a transient null snapshot at the same revision', async t => {
  const original = globalThis.fetch;
  t.after(() => { globalThis.fetch = original; });
  let calls = 0;
  globalThis.fetch = async () => { calls++; return {ok: true, json: async () => null}; };
  const {view} = await fixture();
  const state = {sessionId: 'one', running: true, scene: {active: true, revision: 1}};
  view.update(state);
  await new Promise(done => setImmediate(done));
  view.update(state);
  assert.equal(calls, 2);
});
