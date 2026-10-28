import assert from 'node:assert/strict';
import test from 'node:test';
import { clock, meetingGroups, recognitionActions } from '../../src/format.ts';
import { NoteSession } from '../../src/note-session.ts';
import { rememberSource, resolveSource } from '../../src/source-navigation.ts';

test('notes preserve exact text and reject an edit based on a stale version', async () => {
  const writes = [];
  const session = new NoteSession('base', async (notes, expected) => { writes.push({ notes, expected }); return true; });
  const draft = 'My note\n\n- follow up\n';
  session.edit(draft); assert.equal(await session.save(), true);
  assert.deepEqual(writes, [{ notes: draft, expected: 'base' }]);
  session.edit('My pending changes'); session.adoptRemote('Changed on the Mac');
  assert.equal(await session.save(), false);
  assert.equal(writes.length, 1);
  assert.equal(session.getSnapshot().draft, 'My pending changes');
  assert.equal(session.getSnapshot().conflict, true);
});

test('notes typed during an in-flight save are queued with the newly saved baseline', async () => {
  let finish;
  const first = new Promise((resolve) => { finish = resolve; });
  const writes = [];
  const session = new NoteSession('base', async (notes, expected) => { writes.push({ notes, expected }); return writes.length === 1 ? first : true; });
  session.edit('first edit'); const save = session.save();
  session.edit('latest edit'); finish(true); assert.equal(await save, true);
  assert.deepEqual(writes, [{ notes: 'first edit', expected: 'base' }, { notes: 'latest edit', expected: 'first edit' }]);
  assert.equal(session.getSnapshot().dirty, false);
  assert.equal(session.getSnapshot().baseline, 'latest edit');
});

test('failed note saving retains the draft and can be retried explicitly', async () => {
  let available = false;
  const session = new NoteSession('base', async () => available);
  session.edit('saved later'); assert.equal(await session.save(), false);
  assert.equal(session.getSnapshot().draft, 'saved later');
  assert.equal(session.getSnapshot().failed, true);
  available = true; assert.equal(await session.save(), true);
  assert.equal(session.getSnapshot().failed, false); assert.equal(session.getSnapshot().dirty, false);
});

test('unknown server outcome requires confirmation and finalization cannot be cancelled', () => {
  assert.deepEqual(recognitionActions({ stage: 'response_unknown', recoverable: true }), { cancel: true, resume: true, confirmRetry: true });
  assert.deepEqual(recognitionActions({ stage: 'interrupted', recoverable: true }), { cancel: true, resume: true, confirmRetry: false });
  assert.deepEqual(recognitionActions({ stage: 'applying', recoverable: false }), { cancel: false, resume: false, confirmRetry: false });
  assert.deepEqual(recognitionActions({ stage: 'cancelled', recoverable: false }), { cancel: false, resume: false, confirmRetry: false });
});

test('source navigation validates the cited passage without putting its text in a route token', () => {
  const passage = { meeting_id: 'one', title: 'Owned fixture', kind: 'transcript', text: 'A verified decision.', start_ms: 800 };
  const token = rememberSource(passage, 100);
  assert.equal(token.includes(passage.text), false);
  const detail = { info: { id: 'one' }, notes: '', summary: null, transcript: [{ text: 'A verified decision.', start_ms: 800 }] };
  assert.equal(resolveSource(token, detail, 200).changed, false);
  assert.equal(resolveSource(token, { ...detail, transcript: [{ text: 'An edited decision.' }] }, 200).changed, true);
  assert.equal(resolveSource(token, { ...detail, info: { id: 'different' } }, 200).changed, true);
  assert.equal(resolveSource(token, detail, 31 * 60 * 1000).changed, true);
});

test('meeting groups use local day boundaries and stay newest first', () => {
  const now = new Date(2026, 9, 27, 12);
  const meeting = (id, date) => ({ id, title: id, start_iso: date.toISOString(), duration_min: 5, has_summary: false });
  const groups = meetingGroups([meeting('older', new Date(2026, 9, 1)), meeting('morning', new Date(2026, 9, 27, 9)), meeting('yesterday', new Date(2026, 9, 26, 18)), meeting('afternoon', new Date(2026, 9, 27, 13))], now);
  assert.deepEqual(groups.map((group) => [group.key, group.meetings.map((meeting) => meeting.id)]), [['today', ['afternoon', 'morning']], ['week', ['yesterday']], ['earlier', ['older']]]);
  assert.equal(clock(-5), '0:00'); assert.equal(clock(3723), '62:03');
});
