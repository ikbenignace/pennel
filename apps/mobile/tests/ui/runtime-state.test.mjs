import assert from 'node:assert/strict';
import test from 'node:test';
import { RuntimeStore, LatestRequest } from '../../src/state.ts';

const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
function client(bootstrap = async () => ({ revision: 1, ready: true })) {
  let listener;
  return { bootstrap, getSnapshot: async () => ({ revision: 1, ready: true }), invoke: async () => null, subscribe: (next) => { listener = next; return () => { listener = null; }; }, emit: (value) => listener?.(value) };
}

test('a live snapshot cannot be overwritten by an older bootstrap response', async () => {
  const load = deferred(); const native = client(() => load.promise); const store = new RuntimeStore(native);
  const connection = store.connect();
  native.emit({ revision: 8, ready: true, meeting: { phase: 'recording' } });
  load.resolve({ revision: 2, ready: true, meeting: { phase: 'idle' } });
  await connection;
  assert.equal(store.getSnapshot().snapshot.revision, 8);
  assert.equal(store.getSnapshot().snapshot.meeting.phase, 'recording');
});

test('an unavailable native module produces an actionable connection error', async () => {
  const native = client(); native.subscribe = () => { throw new Error('A native development build is required.'); };
  const store = new RuntimeStore(native);
  await store.connect();
  assert.equal(store.getSnapshot().connecting, false);
  assert.equal(store.getSnapshot().snapshot, null);
  assert.equal(store.getSnapshot().error.message, 'A native development build is required.');
});

test('reconnecting ignores the previous connection while retaining native operations', async () => {
  const old = deferred(); let calls = 0;
  const native = client(() => ++calls === 1 ? old.promise : Promise.resolve({ revision: 12, ready: true, dictation: { phase: 'processing' } }));
  const store = new RuntimeStore(native); const first = store.connect();
  store.disconnect(); await store.connect(); old.resolve({ revision: 99, ready: true, dictation: { phase: 'idle' } }); await first;
  assert.equal(store.getSnapshot().snapshot.revision, 12);
  assert.equal(store.getSnapshot().snapshot.dictation.phase, 'processing');
});

test('unchanged native revisions preserve store identity and do not notify subscribers', async () => {
  const store = new RuntimeStore(client()); await store.connect();
  const previous = store.getSnapshot(); let notifications = 0;
  store.subscribe(() => { notifications += 1; });
  store.accept({ revision: 1, ready: true });
  assert.equal(store.getSnapshot(), previous);
  assert.equal(notifications, 0);
});

test('an equal revision still completes reconnection without replacing the retained snapshot', async () => {
  const native = client(); const store = new RuntimeStore(native); await store.connect();
  const snapshot = store.getSnapshot().snapshot;
  store.disconnect(); await store.connect();
  assert.equal(store.getSnapshot().connecting, false);
  assert.equal(store.getSnapshot().snapshot, snapshot);
});

test('an unchanged refresh preserves an operation error until it is deliberately cleared', async () => {
  const native = client(); const store = new RuntimeStore(native); await store.connect();
  native.invoke = async () => { throw { code: 'storage', message: 'Storage unavailable.' }; };
  await assert.rejects(store.run('save', 'meetings.notes'));
  const error = store.getSnapshot().error;
  await store.refresh();
  assert.equal(store.getSnapshot().error, error);
  store.clearError(); assert.equal(store.getSnapshot().error, null);
});

test('duplicate operations are rejected without dispatching another native command', async () => {
  const operation = deferred(); const native = client(); let calls = 0;
  native.invoke = () => { calls += 1; return operation.promise; };
  const store = new RuntimeStore(native); await store.connect();
  const first = store.run('start', 'meetings.start');
  await assert.rejects(store.run('start', 'meetings.start'), (error) => error.code === 'operation_busy');
  operation.resolve({ id: 'owned-recording' }); await first;
  assert.equal(calls, 1); assert.equal(store.getSnapshot().busy.size, 0);
});

test('failed operations clear busy state and retain actionable native errors', async () => {
  const native = client(); native.invoke = async () => { throw { code: 'quota_exceeded', message: 'Audio quota reached.', recoverable: true }; };
  const store = new RuntimeStore(native); await store.connect();
  await assert.rejects(store.run('recognition', 'recognition.start'));
  assert.deepEqual(store.getSnapshot().error, { code: 'quota_exceeded', message: 'Audio quota reached.', recoverable: true });
  assert.equal(store.getSnapshot().busy.size, 0);
});

test('stale query results and stale query errors cannot replace a newer result', async () => {
  const gate = new LatestRequest(); const old = deferred(); const rejected = deferred(); const shown = [];
  const first = gate.run(() => old.promise, (value) => shown.push(value));
  const second = gate.run(() => rejected.promise, (value) => shown.push(value));
  await gate.run(async () => 'latest', (value) => shown.push(value));
  old.resolve('old'); rejected.reject(new Error('old network failure'));
  await Promise.all([first, second]);
  assert.deepEqual(shown, ['latest']);
});
