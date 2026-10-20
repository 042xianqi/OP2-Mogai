import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';

/** Executes the production browser snapshot conversion over isolated in-memory OPFS handles. */
function fixture(records, bytes) {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const indexPath = 'opfs/runtime/identities/identity-test/operit_runtime_storage.index';
  const dataPath = 'opfs/runtime/identities/identity-test/operit_runtime_storage.data';
  const data = handle(bytes);
  const index = handle(encoder.encode(JSON.stringify({ records })));
  const sources = new Map([[indexPath, { handle: index }], [dataPath, { handle: data }]]);
  const outputs = new Map();
  const progress = [];
  const worker = readFileSync(new URL('../../apps/flutter/app/web/runtime/src/operit_data_rescue_worker.ts', import.meta.url), 'utf8');
  const start = worker.indexOf('function snapshotPortableRuntime(');
  const end = worker.indexOf('/** Captures database schemas', start);
  assert.ok(start >= 0 && end > start);
  const code = stripTypeScriptTypes('async ' + worker.slice(start, end), { mode: 'transform' });
  const convert = new Function('sources', 'resource', 'snapshot', 'read', 'decoder', 'encoder', 'preferenceStoragePath', 'portablePreferences', 'write', 'snapshotBytes', 'reportProgress', 'advance', 'metadataBytes', 'metadataLimit', `${code}; return snapshotPortableRuntime;`)(
    sources,
    async (_root, path) => {
      const output = handle(new Uint8Array());
      outputs.set(path, output);
      return { createSyncAccessHandle: async () => output };
    }, {},
    (source, offset, length) => source.handle.bytes.slice(offset, offset + length),
    decoder, encoder,
    key => key === 'operit2.runtime.runtime/config/preferences/model_configs.preferences.json' ? 'runtime/config/preferences/model_configs.preferences.json' : null,
    (path, content, identity) => {
      assert.equal(identity, 'identity-test');
      assert.equal(path, 'runtime/config/preferences/model_configs.preferences.json');
      assert.equal(decoder.decode(content), 'CIPHERTEXT');
      return encoder.encode('{"api_key":"owned"}');
    },
    (output, offset, content) => output.write(content, { at: offset }),
    async (path, content) => sources.set(path, { handle: handle(content) }),
    value => progress.push(structuredClone(value)),
    (value, amount) => { value.completed += amount; progress.push(structuredClone(value)); },
    source => source.handle.bytes.slice(),
    16 * 1024 * 1024,
  );
  return { convert, sources, data, index, dataPath, indexPath, outputs, progress };
}

/** Implements the exact synchronous OPFS operations used by the production conversion. */
function handle(content) {
  return {
    bytes: content.slice(), closed: false,
    /** Returns the persisted byte count. */
    getSize() { return this.bytes.length; },
    /** Writes bytes while retaining earlier output. */
    write(content, { at }) {
      const grown = new Uint8Array(Math.max(this.bytes.length, at + content.length));
      grown.set(this.bytes); grown.set(content, at); this.bytes = grown;
      return content.length;
    },
    /** Truncates the temporary output. */
    truncate(size) { this.bytes = this.bytes.slice(0, size); },
    /** Represents a successful durability flush. */
    flush() {},
    /** Records resource release without modifying its bytes. */
    close() { this.closed = true; },
  };
}

/** Verifies ciphertext and deleted records are absent while live non-preference bytes remain exact. */
test('browser rescue compacts packed storage with plaintext preferences and preserves live data', async () => {
  const encoder = new TextEncoder();
  const bytes = encoder.encode('DELETED-CIPHERTEXTattachment');
  const preference = 'operit2.runtime.runtime/config/preferences/model_configs.preferences.json';
  const file = 'operit2.runtime.runtime/attachments/example.bin';
  const instance = fixture([[preference, { offset: 8, byteLength: 10 }], [file, { offset: 18, byteLength: 10 }]], bytes);
  await instance.convert();
  const output = instance.sources.get(instance.dataPath).handle.bytes;
  assert.equal(new TextDecoder().decode(output), '{"api_key":"owned"}attachment');
  const index = JSON.parse(new TextDecoder().decode(instance.sources.get(instance.indexPath).handle.bytes));
  assert.deepEqual(index.records, [[preference, { offset: 0, byteLength: 19 }], [file, { offset: 19, byteLength: 10 }]]);
  assert.deepEqual(instance.data.bytes, bytes);
  assert.equal(instance.data.closed, true);
  assert.equal(instance.index.closed, true);
  assert.deepEqual(instance.progress.map(value => value.completed), [0, 10, 20]);
  assert.ok(instance.progress.every(value => value.phase === "snapshotRuntime" && value.total === 20 && value.unit === "bytes"));
});

/** Rejects corrupt metadata instead of exporting an invalid packed runtime image. */
test('browser rescue reports an invalid record range and closes its temporary output', async () => {
  const instance = fixture([['operit2.runtime.runtime/attachments/example.bin', { offset: 4, byteLength: 10 }]], new Uint8Array(5));
  await assert.rejects(instance.convert(), /Invalid runtime OPFS range/);
  assert.equal(instance.outputs.has(instance.dataPath), false);
  assert.equal(instance.data.closed, false);
});
