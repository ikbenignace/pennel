import assert from 'node:assert/strict';
import test from 'node:test';
import { NoteSession } from '../../src/note-session.ts';
import { saveThenRename } from '../../src/meeting-rename.ts';

test('a failed note save prevents rename and keeps the unsaved draft available', async () => {
  const draft = new NoteSession('Saved notes', async () => false);
  draft.edit('Unsaved notes');
  let renamed = false;
  const result = await saveThenRename(draft.save, async () => { renamed = true; return 'new-folder'; });
  assert.equal(result, undefined);
  assert.equal(renamed, false);
  assert.equal(draft.getSnapshot().draft, 'Unsaved notes');
  assert.equal(draft.getSnapshot().dirty, true);
});

test('rename waits for an in-flight autosave and the edits queued behind it', async () => {
  let finishFirst;
  const saved = [];
  const draft = new NoteSession('Original', async (notes) => {
    if (saved.length === 0) await new Promise((resolve) => { finishFirst = resolve; });
    saved.push(notes);
    return true;
  });
  draft.edit('First edit');
  const autosave = draft.save();
  draft.edit('Latest edit');
  let renamed = false;
  const rename = saveThenRename(draft.save, async () => { renamed = true; return 'renamed-folder'; });
  assert.equal(renamed, false);
  finishFirst();
  await autosave;
  assert.equal(await rename, 'renamed-folder');
  assert.deepEqual(saved, ['First edit', 'Latest edit']);
  assert.equal(draft.getSnapshot().dirty, false);
});
