import React, { useEffect, useMemo, useRef, useState } from 'react';
import { FlatList, Modal, Pressable, Share, Text, View } from 'react-native';
import { Stack, useLocalSearchParams, useRouter, usePreventRemove } from 'expo-router';
import type { MeetingDetail, MeetingTemplate, SpeakerJob } from '../domain';
import { useQuery, useRuntime } from '../runtime';
import { meetingDetail, speakerJob } from '../normalize';
import { clock, dateLabel, recognitionActions } from '../format';
import { resolveSource } from '../source-navigation';
import { useNoteDraft } from '../use-note-draft';
import { saveThenRename } from '../meeting-rename';
import { localizedFailure } from '../errors';
import { templateLabel } from '../owned-labels';
import { Actions, Body, Button, Choice, Empty, Field, Loading, Notice, Page, Row, Section, confirm } from '../components/ui';
import { TextDialog } from '../components/TextDialog';
import { Markdown } from '../components/Markdown';
import { AskPanel } from '../components/AskPanel';

type Pane = 'notes' | 'transcript' | 'questions';
type Editor = { type: 'rename' } | { type: 'speaker'; voice: number; initial: string } | null;
export default function Meeting() {
  const params = useLocalSearchParams<{ id: string; source?: string }>();
  const id = Array.isArray(params.id) ? params.id[0] : params.id;
  const { snapshot, run, busy, t, theme, language } = useRuntime();
  const router = useRouter();
  const query = useQuery<MeetingDetail>('meetings.read', { id }, Boolean(id && snapshot?.ready));
  const templates = useQuery<MeetingTemplate[]>('meetings.templates', {}, Boolean(snapshot?.ready));
  const detail = useMemo(() => query.value?.info ? meetingDetail(query.value) : null, [query.value]);
  const [pane, setPane] = useState<Pane>('notes');
  const draft = useNoteDraft(id, detail?.notes ?? '');
  const notes = draft.draft;
  const [template, setTemplate] = useState(snapshot?.settings.default_template ?? 'general');
  const [editor, setEditor] = useState<Editor>(null);
  const [viewConflict, setViewConflict] = useState<{ name: string; text: string } | null>(null);
  const [currentJob, setCurrentJob] = useState<SpeakerJob | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const transcriptList = useRef<FlatList>(null);
  const openedSource = useRef<string | null>(null);
  const dirty = draft.dirty;
  const remoteChanged = draft.conflict;
  const disableRemoval = usePreventRemove(dirty, ({ repeat }) => confirm(t('mobile.notes.leave_title'), t('mobile.notes.leave_help'), t('common.delete'), t('common.cancel'), () => { draft.discard(); repeat(); }, true));
  useEffect(() => {
    const token = typeof params.source === 'string' ? params.source : null;
    if (!token || !detail || token === openedSource.current) return;
    openedSource.current = token;
    const source = resolveSource(token, detail);
    if (source.changed || !source.passage) { setNotice(t('meetings.source_changed')); return; }
    setPane(source.passage.kind === 'transcript' ? 'transcript' : 'notes');
    if (source.transcriptIndex !== null) {
      const index = source.transcriptIndex;
      setTimeout(() => transcriptList.current?.scrollToIndex({ index, animated: true, viewPosition: 0.2 }), 300);
    }
  }, [detail, params.source, t]);
  const jobs = (snapshot?.recognition_jobs ?? []).map(speakerJob);
  const job = currentJob ? jobs.find((job) => job.id === currentJob.id) ?? currentJob : jobs.find((job) => job.meeting_id === id);
  const jobStage = job?.stage;
  useEffect(() => { if (jobStage === 'completed') void query.refresh(); }, [jobStage, query.refresh]);
  useEffect(() => {
    if (!job || !['preparing', 'uploading', 'recognizing', 'applying'].includes(job.stage)) return;
    const timer = setInterval(() => { void run<SpeakerJob>(`job-poll-${job.id}`, 'recognition.get', { job_id: job.id }).then((value) => { if (value) setCurrentJob(speakerJob(value)); }); }, 1500);
    return () => clearInterval(timer);
  }, [job?.id, jobStage, run]);
  const save = async () => {
    if (!detail || remoteChanged) return;
    if (await draft.save()) { setNotice(t('mobile.saved')); await query.refresh(); }
  };
  const summarize = async () => { const result = await run('summary', 'meetings.summarize', { id, template }); if (result !== undefined) await query.refresh(); };
  const recognize = () => confirm(t('mobile.speakers.consent_title'), t('mobile.speakers.consent'), t('mobile.speakers.allow'), t('common.cancel'), () => { void run<SpeakerJob>('recognition-start', 'recognition.start', { id, consent: true }).then((value) => { if (value) setCurrentJob(speakerJob(value)); }); });
  const resume = () => {
    if (!job) return;
    const perform = () => { void run<SpeakerJob>('recognition-resume', 'recognition.resume', { job_id: job.id, confirm_unknown: job.stage === 'response_unknown' }).then((value) => { if (value) setCurrentJob(speakerJob(value)); }); };
    if (job.stage === 'response_unknown') confirm(t('mobile.speakers.retry_title'), t('mobile.speakers.retry_help'), t('common.retry'), t('common.cancel'), perform);
    else perform();
  };
  const exportMeeting = async (format: 'formatted' | 'markdown' | 'transcript' | 'share') => {
    const result = await run<{ title: string; markdown: string; html: string }>('export', 'meetings.export', { id, include_transcript: format === 'transcript' });
    if (!result) return;
    if (format === 'share') { try { await Share.share({ title: result.title, message: result.markdown }); } catch { setNotice(t('mobile.error')); } }
    else { await run('copy', 'dictation.copy', { text: result.markdown, html: format === 'formatted' ? result.html : null }); setNotice(t('meetings.copied')); }
  };
  if (!detail) return <Page title={t('navigation.meetings')} inset={false}>{query.loading ? <Loading /> : query.error ? <><Notice error>{query.error}</Notice><Button title={t('common.retry')} subtle onPress={() => void query.refresh()} /></> : <Empty title={t('meetings.not_found')} />}</Page>;
  const voices = [...new Set(detail.transcript.flatMap((segment) => segment.voice === null ? [] : [segment.voice]))].sort((a, b) => a - b);
  const names = new Map(detail.speakers.map((speaker) => [speaker.voice, speaker.name]));
  const canPlay = detail.audio_tracks.some((track) => track.available);
  const active = Boolean(snapshot && snapshot.meeting.id === id && snapshot.meeting.phase !== 'idle');
  const jobBusy = job && ['preparing', 'uploading', 'recognizing', 'applying'].includes(job.stage);
  const speakerEnabled = snapshot?.cloud.signed_in && !active && canPlay && !jobBusy;
  return <Page title={detail.info.title} subtitle={`${dateLabel(detail.info.start_iso, language)} · ${t('meetings.duration_minutes', { count: detail.info.duration_min })}`} inset={false} scroll={pane !== 'transcript'}>
    <Stack.Screen options={{ title: detail.info.title }} />
    {notice && <Section><Notice>{notice}</Notice></Section>}
    {detail.interruption && <Section><Notice error>{detail.interruption}</Notice></Section>}
    {(detail.conflicts ?? []).length > 0 && <Section description={t('meetings.conflict_notice')}>{detail.conflicts?.map((conflict) => <Row key={conflict.name} title={conflict.name} onPress={() => setViewConflict({ name: conflict.name, text: conflict.text ?? '' })} />)}</Section>}
    <View accessibilityRole="tablist" style={{ flexDirection: 'row', backgroundColor: theme.colors.subtle, borderRadius: theme.radius.segmented, marginBottom: theme.space[5] }}>{(['notes', 'transcript', 'questions'] as const).map((value) => <Pressable key={value} accessibilityRole="tab" accessibilityState={{ selected: pane === value }} onPress={() => setPane(value)} style={{ flex: 1, minHeight: 44, padding: theme.space[3], backgroundColor: pane === value ? theme.colors.selection : 'transparent', borderRadius: theme.radius.segmented }}><Text style={{ fontFamily: theme.font.medium, fontSize: theme.size.content, color: theme.colors.foreground, textAlign: 'center' }}>{t(`meetings.${value}`)}</Text></Pressable>)}</View>
    {pane === 'notes' && <>
      <Section title={t('meetings.your_notes')}><Field label={t('meetings.your_notes')} multiline value={notes} onChangeText={draft.setDraft} placeholder={t('meetings.notes_placeholder')} style={{ minHeight: 180 }} />{remoteChanged && <><Notice error>{t('mobile.notes.remote_change')}</Notice><Button title={t('mobile.refresh')} subtle onPress={() => confirm(t('mobile.notes.leave_title'), t('mobile.notes.leave_help'), t('mobile.refresh'), t('common.cancel'), draft.discard)} /></>}{dirty && <Button title={t('common.save')} busy={draft.saving} disabled={remoteChanged} onPress={() => void save()} />}</Section>
      <Section title={t('mobile.summary')}>{detail.summary_outdated && <Notice>{t('meetings.summary_outdated')}</Notice>}{detail.summary ? <Markdown text={detail.summary} /> : <Body muted>{t('meetings.empty_summary')}</Body>}<Choice label={t('meetings.template')} value={template} options={(templates.value ?? []).map((value) => ({ value: value.id, label: templateLabel(value, language) }))} onChange={setTemplate} /><Button title={t(detail.summary ? 'meetings.regenerate' : 'meetings.summarize')} busy={busy.has('summary')} disabled={active || !snapshot?.settings.llm.provider_id} onPress={() => void summarize()} />{!snapshot?.settings.llm.provider_id && <Button title={t('meetings.set_up_summaries')} subtle onPress={() => router.push({ pathname: '/settings/[section]', params: { section: 'models' } })} />}</Section>
      <Section title={t('meetings.share')}><Actions><Button title={t('meetings.copy_formatted')} subtle onPress={() => void exportMeeting('formatted')} /><Button title={t('meetings.copy_markdown')} subtle onPress={() => void exportMeeting('markdown')} /><Button title={t('meetings.copy_with_transcript')} subtle onPress={() => void exportMeeting('transcript')} /><Button title={t('meetings.share_action')} subtle icon="share-outline" onPress={() => void exportMeeting('share')} /></Actions></Section>
      <Section><Button title={t('meetings.rename_action')} subtle onPress={() => setEditor({ type: 'rename' })} /><Button title={t('meetings.delete_action')} subtle destructive disabled={active} onPress={() => confirm(t('meetings.delete_title', { title: detail.info.title }), t('meetings.delete_help'), t('common.delete'), t('common.cancel'), () => { void draft.save().then((saved) => saved ? run('delete-meeting', 'meetings.delete', { id }) : undefined).then((value) => { if (value !== undefined) { draft.discard(); disableRemoval(); router.replace('/(tabs)'); } }); }, true)} /></Section>
    </>}
    {pane === 'transcript' && <>
      <Section title={t('mobile.speakers.choice')}>
        <Button title={t('mobile.speakers.manual')} busy={busy.has('recognition-start')} disabled={!speakerEnabled} onPress={recognize} />
        {!snapshot?.cloud.signed_in && <Button title={t('meetings.speaker_cloud_sign_in')} subtle onPress={() => router.push({ pathname: '/settings/[section]', params: { section: 'account' } })} />}
        {!canPlay && <Notice>{t('meetings.no_saved_audio')}</Notice>}
        {job && <><Notice error={job.stage === 'failed' || job.stage === 'response_unknown'}>{t(`mobile.speakers.${job.stage}`, { count: job.confirmed_chunks })}</Notice>{job.error && <Notice error>{localizedFailure(language, job.error)}</Notice>}<Actions>{recognitionActions(job).cancel && <Button title={t(job.error?.code === 'speaker_cleanup' ? 'mobile.speakers.cleanup' : 'common.cancel')} subtle onPress={() => void run<SpeakerJob>('recognition-cancel', 'recognition.cancel', { job_id: job.id }).then((value) => { if (value) setCurrentJob(speakerJob(value)); })} />}{recognitionActions(job).resume && <Button title={t('mobile.speakers.resume')} subtle busy={busy.has('recognition-resume')} onPress={resume} />}</Actions></>}
        {voices.map((voice) => <Row key={voice} title={names.get(voice) || t('mobile.speakers.unknown', { number: voice })} onPress={() => setEditor({ type: 'speaker', voice, initial: names.get(voice) ?? '' })} trailing={detail.voices?.find((item) => item.voice === voice)?.can_remember && names.get(voice) ? <Button title={t('mobile.speakers.remember')} subtle onPress={() => confirm(t('mobile.speakers.remember'), t('mobile.speakers.remember_help'), t('common.save'), t('common.cancel'), () => void run('remember-speaker', 'recognition.renameSpeaker', { id, voice, name: names.get(voice), remember: true }).then(() => query.refresh()))} /> : undefined} />)}
        {detail.transcript.some((segment) => segment.failed) && <Button title={t('common.retry')} subtle busy={busy.has('retry-transcription')} onPress={() => void run('retry-transcription', 'meetings.retryTranscription', { id }).then(() => query.refresh())} />}
        {canPlay && <Button title={t('mobile.playback.stop')} subtle onPress={() => void run('playback', 'meetings.stopPlayback')} />}
      </Section>
      <FlatList ref={transcriptList} data={detail.transcript} keyExtractor={(segment, index) => `${segment.start_ms}-${index}`} onScrollToIndexFailed={({ index, averageItemLength }) => { transcriptList.current?.scrollToOffset({ offset: averageItemLength * index, animated: true }); }} renderItem={({ item }) => <Pressable accessibilityRole="button" accessibilityLabel={`${clock(item.start_ms / 1000)} ${item.text}`} accessibilityHint={canPlay ? t('meetings.play_from_here') : t('meetings.no_saved_audio')} disabled={!canPlay} onPress={() => void run('playback', 'meetings.play', { id, speaker: item.speaker, start_ms: item.start_ms })} style={{ gap: theme.space[1], paddingBottom: theme.space[5] }}><Body muted style={{ fontSize: theme.size.label }}>{clock(item.start_ms / 1000)} · {item.voice !== null ? names.get(item.voice) || t('mobile.speakers.unknown', { number: item.voice }) : t(`meetings.speaker_${item.speaker}`)}</Body><Body selectable style={item.failed ? { color: theme.colors.accent } : undefined}>{item.failed ? t('meetings.transcription_failed') : item.text}</Body></Pressable>} ListEmptyComponent={<Empty title={t('meetings.empty_transcript')} />} />
    </>}
    {pane === 'questions' && <AskPanel meetingId={id} />}
    <TextDialog visible={editor !== null} title={t(editor?.type === 'rename' ? 'meetings.rename_title' : 'meetings.voice_name')} label={t(editor?.type === 'rename' ? 'meetings.meeting_title_label' : 'mobile.speakers.name')} initial={editor?.type === 'speaker' ? editor.initial : detail.info.title} help={editor?.type === 'speaker' ? t('mobile.speakers.name_help') : undefined} onClose={() => setEditor(null)} onSave={async (value) => {
      if (!editor) return false;
      if (editor.type === 'rename') { if (!value.trim()) return false; const renamed = await saveThenRename(draft.save, () => run<string>('rename-meeting', 'meetings.rename', { id, title: value.trim() })); if (renamed !== undefined) { router.setParams({ id: renamed }); return true; } }
      else { const saved = await run('rename-speaker', 'recognition.renameSpeaker', { id, voice: editor.voice, name: value.trim(), remember: false }); if (saved !== undefined) { await query.refresh(); return true; } }
      return false;
    }} />
    <Modal visible={viewConflict !== null} presentationStyle="pageSheet" animationType="slide" onRequestClose={() => setViewConflict(null)}><Page title={viewConflict?.name}><Body selectable>{viewConflict?.text}</Body><Actions><Button title={t('common.close')} subtle onPress={() => setViewConflict(null)} /><Button title={t('common.delete')} subtle destructive onPress={() => confirm(t('meetings.delete_conflict_title'), t('meetings.file_will_delete', { name: viewConflict?.name ?? '' }), t('common.delete'), t('common.cancel'), () => { void run('conflict-delete', 'meetings.conflictDelete', { id, name: viewConflict?.name }).then(() => { setViewConflict(null); return query.refresh(); }); }, true)} /></Actions></Page></Modal>
  </Page>;
}
