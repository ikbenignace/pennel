import React, { useCallback, useEffect, useState } from 'react';
import { useFocusEffect, useRouter } from 'expo-router';
import { Pressable, View } from 'react-native';
import Ionicons from '@expo/vector-icons/Ionicons';
import { useQuery, useRuntime } from '../runtime';
import type { MeetingInfo, RuntimeSnapshot } from '../domain';
import { clock, dateLabel, meetingGroups } from '../format';
import { Body, Button, Empty, Field, Loading, Notice, Page, Row, Section } from '../components/ui';

export default function Meetings() {
  const { snapshot, connecting, run, busy, t, theme, language, store } = useRuntime();
  const [query, setQuery] = useState('');
  const [title, setTitle] = useState('');
  const [search, setSearch] = useState('');
  const router = useRouter();
  const meetings = useQuery<MeetingInfo[]>('meetings.list', { query: search }, Boolean(snapshot?.ready));
  useEffect(() => { const timer = setTimeout(() => setSearch(query.trim()), 180); return () => clearTimeout(timer); }, [query]);
  useFocusEffect(useCallback(() => { void meetings.refresh(); void store.refresh(); }, [meetings.refresh, store]));
  const phase = snapshot?.meeting.phase;
  useEffect(() => { if (phase === 'idle') void meetings.refresh(); }, [meetings.refresh, phase]);
  const active = Boolean(snapshot?.meeting.id && phase !== 'idle');
  const retainedId = snapshot?.meeting.retained_id;
  const start = async () => { if (active) { router.push('/recording'); return; } if (retainedId) return; const value = await run('meeting-start', 'meetings.start', { title: title.trim() }); if (value !== undefined) { setTitle(''); router.push('/recording'); } };
  const shown = (meetings.value ?? []).filter((meeting) => meeting.id !== snapshot?.meeting.id || !active);
  return <Page title={t('navigation.meetings')} action={<Pressable accessibilityRole="button" accessibilityLabel={t('mobile.refresh')} onPress={() => void meetings.refresh()} style={{ padding: theme.space[2], minHeight: 44, minWidth: 44, alignItems: 'center', justifyContent: 'center' }}><Ionicons name="refresh-outline" color={theme.colors.foreground} size={22} /></Pressable>}>
    {connecting && !snapshot && <Loading />}
    {!snapshot && !connecting && <Section><Notice error>{t('mobile.unavailable')}</Notice><Button title={t('common.retry')} subtle onPress={() => void store.connect()} /></Section>}
    <Section title={t('meetings.new_meeting')} description={t('mobile.recording.help')}>
      {!active && <Field label={t('meetings.meeting_title_label')} placeholder={t('meetings.meeting_title_placeholder')} value={title} onChangeText={setTitle} returnKeyType="go" onSubmitEditing={() => void start()} />}
      <Button title={active ? t('meetings.return_recording') : t('common.start_recording')} icon={active ? 'pulse' : 'add'} busy={busy.has('meeting-start')} disabled={!snapshot?.ready || Boolean(retainedId)} onPress={() => void start()} />
      {active && <Row title={snapshot?.meeting.title || t('meetings.default_title')} subtitle={`${phase === 'finishing' ? t('meetings.finishing') : t('meetings.recording_active')} · ${clock(snapshot?.meeting.elapsed_secs ?? 0)}`} onPress={() => router.push('/recording')} trailing={<View style={{ width: 8, height: 8, borderRadius: 4, backgroundColor: theme.colors.accent }} />} />}
    </Section>
    {retainedId && <Section title={t('meetings.retry_save')} description={t('mobile.recording.unsaved')}><Button title={t('meetings.retry_save')} busy={busy.has('meeting-retry-save')} onPress={() => void run<RuntimeSnapshot>('meeting-retry-save', 'meetings.retrySave', { id: retainedId }).then((result) => { if (result) return meetings.refresh(); })} /></Section>}
    <Field label={t('meetings.search_label')} value={query} onChangeText={setQuery} clearButtonMode="while-editing" autoCorrect={false} returnKeyType="search" />
    <View style={{ height: theme.space[4] }} />
    {meetings.loading && !meetings.value && <Loading />}
    {meetings.error && <Section><Notice error>{meetings.error}</Notice><Button title={t('common.retry')} subtle onPress={() => void meetings.refresh()} /></Section>}
    {!meetings.loading && !shown.length && <Empty title={t(search ? 'meetings.none_found' : 'meetings.empty_meetings')} description={!search ? t('meetings.empty_meetings_help') : undefined} />}
    {meetingGroups(shown).map((group) => <Section key={group.key} title={t(`meetings.group_${group.key}`)}>{group.meetings.map((meeting) => <Row key={meeting.id} title={meeting.title} subtitle={`${dateLabel(meeting.start_iso, language)} · ${t('meetings.duration_minutes', { count: meeting.duration_min })}${meeting.sync_state ? ` · ${t(`meetings.sync.${meeting.sync_state}`)}` : ''}`} onPress={() => router.push({ pathname: '/meeting/[id]', params: { id: meeting.id } })} />)}</Section>)}
  </Page>;
}
