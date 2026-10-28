import React, { useRef } from 'react';
import { FlatList, View } from 'react-native';
import { useRouter, usePreventRemove } from 'expo-router';
import { useRuntime } from '../runtime';
import { clock } from '../format';
import { Actions, Body, Button, Field, Notice, Page, Section, confirm } from '../components/ui';
import { Wave } from '../components/Wave';
import { useNoteDraft } from '../use-note-draft';
import { RecordingIdentity, stopThenSave } from '../recording-finish';
import type { RuntimeSnapshot } from '../domain';
import { ownedFailureText } from '../errors';

export default function Recording() {
  const { snapshot, busy, run, t, theme, language } = useRuntime();
  const identity = useRef(new RecordingIdentity());
  const meeting = identity.current.observe(snapshot?.meeting);
  const router = useRouter();
  const draft = useNoteDraft(meeting?.id, meeting?.notes ?? '', identity.current.key);
  const notes = draft.draft;
  const save = draft.save;
  usePreventRemove(draft.dirty, ({ repeat }) => confirm(t('mobile.notes.leave_title'), t('mobile.notes.leave_help'), t('common.delete'), t('common.cancel'), () => { draft.discard(); repeat(); }, true));
  const finish = async () => {
    const id = meeting?.id;
    let destination = id;
    let finalized = false;
    const result = await stopThenSave(async () => {
      const stopped = await run<RuntimeSnapshot>('meeting-stop', 'meetings.stop', { id });
      const finishedId = stopped?.meeting.last_finished_id;
      if (id && finishedId && identity.current.retarget(id, finishedId)) { destination = finishedId; draft.retarget(finishedId); }
      finalized = Boolean(stopped && !stopped.meeting.retained_id && !stopped.meeting.error);
      return stopped !== undefined;
    }, save);
    if (result.stopped && result.saved && finalized && !draft.getCurrent().dirty && destination) router.replace({ pathname: '/meeting/[id]', params: { id: destination } });
  };
  const recording = meeting?.phase === 'recording' || meeting?.phase === 'interrupted';
  return <Page title={meeting?.title || t('meetings.default_title')} subtitle={meeting?.phase === 'finishing' ? t('meetings.finishing_recording') : t(recording ? 'meetings.recording_clock' : 'meetings.recording_stopped', { time: clock(meeting?.elapsed_secs ?? 0) })} scroll={false} action={<Button title={t('common.close')} subtle onPress={() => { void save().then((saved) => { if (saved) router.back(); }); }} />}>
    <Section>
      <View style={{ backgroundColor: theme.colors['pill-background'], borderRadius: theme.radius.panel, padding: theme.space[4] }}><Wave active={recording} /></View>
      {meeting?.error && <Notice error>{ownedFailureText(language, meeting.error, meeting.error_code)}</Notice>}
      {meeting?.phase === 'finishing' && <Notice>{t('meetings.finishing_pending_many', { count: meeting.pending })}</Notice>}
      <Actions>{recording && <Button title={t('common.stop_recording')} icon="stop" busy={busy.has('meeting-stop')} onPress={() => void finish()} />}{snapshot?.meeting.retained_id === meeting?.id && meeting?.id && <Button title={t('meetings.retry_save')} busy={busy.has('meeting-retry-save')} onPress={() => void run<RuntimeSnapshot>('meeting-retry-save', 'meetings.retrySave', { id: meeting.id }).then((result) => { if (result) return save(); return false; })} />}{meeting?.phase === 'idle' && <Button title={t('common.back')} subtle onPress={() => router.back()} />}</Actions>
    </Section>
    <Section title={t('meetings.your_notes')}>
      <Field label={t('meetings.your_notes')} editable={Boolean(meeting?.id)} multiline value={notes} onChangeText={draft.setDraft} placeholder={t('meetings.live_notes_placeholder')} style={{ minHeight: 120, maxHeight: 220 }} />
      {draft.conflict && <Notice error>{t('mobile.notes.remote_change')}</Notice>}
      {draft.dirty && <Button title={t('common.save')} subtle busy={draft.saving} onPress={() => void save()} />}
      {draft.dirty && (draft.failed || draft.conflict) && <Button title={t('common.copy')} subtle onPress={() => void run('copy-recording-notes', 'dictation.copy', { text: notes })} />}
    </Section>
    <Body style={{ fontFamily: theme.font.semibold, marginBottom: theme.space[3] }}>{t('meetings.live_transcript')}</Body>
    <FlatList data={meeting?.transcript ?? []} keyExtractor={(segment, index) => `${segment.start_ms}-${index}`} contentContainerStyle={{ paddingBottom: theme.space[6] }} renderItem={({ item }) => <View style={{ marginBottom: theme.space[3] }}><Body muted style={{ fontSize: theme.size.label }}>{clock(item.start_ms / 1000)}</Body><Body>{item.text}</Body></View>} ListEmptyComponent={<Notice>{t('meetings.empty_transcript')}</Notice>} />
  </Page>;
}
