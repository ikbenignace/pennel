import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useFocusEffect, useRouter } from 'expo-router';
import { Pressable, Text, View } from 'react-native';
import type { HistoryEntry } from '../domain';
import { useQuery, useRuntime } from '../runtime';
import { clock, dateLabel } from '../format';
import { Actions, Body, Button, Empty, Loading, Notice, Page, Row, Section, confirm } from '../components/ui';
import { Wave } from '../components/Wave';

export default function Dictation() {
  const { snapshot, busy, run, t, theme, language } = useRuntime();
  const history = useQuery<HistoryEntry[]>('dictation.history', { limit: 200 }, Boolean(snapshot?.ready));
  const phase = snapshot?.dictation.phase ?? 'idle';
  const router = useRouter();
  const [now, setNow] = useState(Date.now());
  const held = useRef(false);
  const heldStart = useRef<Promise<unknown> | null>(null);
  const released = useRef(false);
  useEffect(() => { const timer = setInterval(() => setNow(Date.now()), 1000); return () => clearInterval(timer); }, []);
  useFocusEffect(useCallback(() => { void history.refresh(); }, [history.refresh]));
  useEffect(() => { if (phase === 'idle') void history.refresh(); }, [history.refresh, phase]);
  const start = () => run('dictation', 'dictation.start');
  const stop = () => run('dictation', 'dictation.stop');
  const finishHold = () => {
    if (!held.current || released.current) return;
    released.current = true;
    void heldStart.current?.then((value) => { if (value !== undefined) return stop(); return undefined; });
  };
  useEffect(() => () => { if (held.current && !released.current) { released.current = true; void heldStart.current?.then((value) => { if (value !== undefined) return stop(); return undefined; }); } }, []);
  const session = snapshot?.session;
  const sessionTitle = session?.phase === 'ready' ? t('mobile.session.ready', { time: clock(Math.max(0, ((session.expires_at_ms ?? now) - now) / 1000)) }) : session?.phase === 'listening' ? t('dictation.pad.listening') : session?.phase === 'processing' ? t('dictation.pad.processing') : t('mobile.session.off');
  const refreshHistory = async (method: 'dictation.retry' | 'dictation.deleteHistory', payload: Record<string, unknown>) => { await run(`history-${String(payload.id ?? '')}`, method, payload); await history.refresh(); };
  return <Page title={t('navigation.dictation')}>
    <Section title={t('mobile.session')} description={t('mobile.session.help')}>
      <Row title={sessionTitle} subtitle={session?.error ?? undefined} />
      <Button title={t(session && session.phase !== 'off' ? 'mobile.session.stop' : 'mobile.session.start')} subtle busy={busy.has('session')} disabled={!snapshot?.ready} onPress={() => void run('session', session && session.phase !== 'off' ? 'session.stop' : 'session.start')} />
      <Notice>{t('mobile.keyboard.goal')}</Notice>
    </Section>
    <Section description={t('mobile.dictation.help')}>
      <Pressable accessibilityRole="button" accessibilityLabel={t(phase === 'listening' ? 'dictation.pad.stop' : phase === 'processing' ? 'dictation.pad.processing' : 'mobile.dictate')} accessibilityHint={t('mobile.dictation.help')} accessibilityState={{ disabled: !snapshot?.ready || phase === 'processing', busy: phase === 'processing' }} disabled={!snapshot?.ready || phase === 'processing'} delayLongPress={220} onPressIn={() => { held.current = false; released.current = false; }} onLongPress={() => { if (phase !== 'idle') return; held.current = true; heldStart.current = start(); }} onPressOut={finishHold} onPress={() => { if (held.current) return; void (phase === 'listening' ? stop() : start()); }} style={({ pressed }) => ({ borderRadius: theme.radius.panel, backgroundColor: theme.colors['pill-background'], padding: theme.space[6], alignItems: 'center', gap: theme.space[3], opacity: pressed ? 0.85 : 1 })}>
        <Wave active={phase === 'listening' || phase === 'processing'} level={snapshot?.dictation.level ?? 0} />
        <Text style={{ fontFamily: theme.font.medium, fontSize: theme.size.section, color: theme.colors['pill-text'] }}>{t(phase === 'listening' ? 'dictation.pad.listening' : phase === 'processing' ? 'dictation.pad.processing' : 'mobile.dictate')}</Text>
      </Pressable>
      {(phase === 'listening' || phase === 'processing') && <Button title={t('common.cancel')} subtle onPress={() => void run('dictation-cancel', 'dictation.cancel')} />}
      {snapshot?.dictation.error && <Notice error>{snapshot.dictation.error}</Notice>}
      {snapshot?.dictation.text ? <><Body selectable>{snapshot.dictation.text}</Body><Button title={t('common.copy')} icon="copy-outline" subtle onPress={() => void run('copy', 'dictation.copy', { text: snapshot.dictation.text })} /></> : null}
    </Section>
    <Section title={t('dictation.history.title')}>
      {history.loading && !history.value && <Loading />}
      {history.error && <><Notice error>{history.error}</Notice><Button title={t('common.retry')} subtle onPress={() => void history.refresh()} /></>}
      {!history.loading && history.value?.length === 0 && <Empty title={t('dictation.history.empty')} />}
      {(history.value ?? []).map((entry) => <View key={entry.id} style={{ paddingVertical: theme.space[3], gap: theme.space[2] }}>
        <Body muted style={{ fontSize: theme.size.label }}>{dateLabel(new Date(entry.created_at_ms).toISOString(), language)}</Body>
        <Body selectable>{entry.text || entry.raw}</Body>
        {entry.error && <Notice error>{entry.error}</Notice>}
        {entry.status === 'unsaved' && <Notice>{t('dictation.history.temporary')}</Notice>}
        <Actions><Button title={t('common.copy')} subtle onPress={() => void run('copy', 'dictation.copy', { text: entry.text || entry.raw })} /><Button title={t(entry.status === 'unsaved' ? 'dictation.history.save_again' : 'common.retry')} subtle disabled={!entry.has_audio && entry.status !== 'unsaved'} busy={busy.has(`history-${entry.id}`)} onPress={() => void refreshHistory('dictation.retry', { id: entry.id })} /><Button title={t('common.delete')} subtle destructive onPress={() => confirm(t('dictation.history.discard_title'), t('dictation.history.discard_help'), t('common.delete'), t('common.cancel'), () => void refreshHistory('dictation.deleteHistory', { id: entry.id }), true)} /></Actions>
      </View>)}
    </Section>
    <Button title={t('dictation.pad.settings')} subtle icon="settings-outline" onPress={() => router.push({ pathname: '/settings/[section]', params: { section: 'dictation' } })} />
  </Page>;
}
