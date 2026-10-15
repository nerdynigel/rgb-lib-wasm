// Execute the production inline-JS CAS with controlled IndexedDB events.
// Transaction doubles only: this is not browser/IndexedDB durability evidence.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
const source = readFileSync(new URL('../src/wallet/idb_store.rs', import.meta.url), 'utf8');
const start = source.indexOf('export function commitEncodedSnapshot');
const js = source.slice(start, source.indexOf('"#)]', start));
const { commitEncodedSnapshot } = await import(`data:text/javascript,${encodeURIComponent(js)}`);

function fixture({ value = 'old', exists = true, fresh = false, putFails = false } = {}) {
  let created = 0, put = 0, closed = 0, aborted = 0;
  const request = {};
  const store = {
    count() { return { result: exists ? 1 : 0 }; },
    get() { const get = { result: value }; queueMicrotask(() => get.onsuccess()); return get; },
    put() { put++; if (putFails) throw new Error('fixture put failure'); },
  };
  const tx = {
    objectStore() { return store; },
    abort() { aborted++; queueMicrotask(() => tx.onabort?.()); },
  };
  const upgrade = { abort() { aborted++; } };
  const db = {
    version: 1, objectStoreNames: { contains: () => true },
    createObjectStore() { created++; },
    transaction() { return tx; }, close() { closed++; },
  };
  globalThis.indexedDB = { open() {
    queueMicrotask(() => {
      request.result = db;
      if (fresh) {
        request.transaction = upgrade;
        request.onupgradeneeded({ oldVersion: 0 });
        if (aborted) { request.onerror(); return; }
      }
      request.onsuccess();
    });
    return request;
  } };
  return { tx, stats: () => ({ created, put, closed, aborted }) };
}
const tick = () => new Promise(resolve => setImmediate(resolve));

test('existing-record CAS refuses missing database before schema creation', async () => {
  const f = fixture({ fresh: true, exists: false });
  await assert.rejects(commitEncodedSnapshot('db', 'snapshots', 'key', 'old', 'new', 1));
  assert.equal(f.stats().created, 0);
  assert.equal(f.stats().put, 0);
});
test('matching exact record resolves only after transaction completion', async () => {
  const f = fixture(); let completed = false;
  const pending = commitEncodedSnapshot('db', 'snapshots', 'key', 'old', 'new', 1)
    .then(() => { completed = true; });
  await tick();
  assert.equal(f.stats().put, 1); assert.equal(completed, false);
  f.tx.oncomplete(); await pending; assert.equal(f.stats().closed, 1);
});
test('stale ciphertext refuses without put even with otherwise valid record', async () => {
  const f = fixture({ value: 'newer' });
  await assert.rejects(commitEncodedSnapshot('db', 'snapshots', 'key', 'old', 'new', 1));
  assert.equal(f.stats().put, 0); assert.equal(f.stats().aborted, 1);
});
test('insert-only refuses existing record', async () => {
  const f = fixture();
  await assert.rejects(commitEncodedSnapshot('db', 'snapshots', 'key', null, 'new', 1));
  assert.equal(f.stats().put, 0);
});
test('put failure refuses rather than acknowledging persistence', async () => {
  const f = fixture({ putFails: true });
  await assert.rejects(commitEncodedSnapshot('db', 'snapshots', 'key', 'old', 'new', 1));
  assert.equal(f.stats().aborted, 1);
});
test('transaction abort after successful put refuses persistence', async () => {
  const f = fixture();
  const pending = commitEncodedSnapshot('db', 'snapshots', 'key', 'old', 'new', 1);
  await tick(); assert.equal(f.stats().put, 1);
  f.tx.onabort(); await assert.rejects(pending);
});
test('explicit fresh insert may create schema and waits for completion', async () => {
  const f = fixture({ fresh: true, exists: false });
  const pending = commitEncodedSnapshot('db', 'snapshots', 'key', null, 'new', 1);
  await tick(); assert.equal(f.stats().created, 1); assert.equal(f.stats().put, 1);
  f.tx.oncomplete(); await pending;
});
