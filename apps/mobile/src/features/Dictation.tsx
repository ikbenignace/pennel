import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useFocusEffect, useRouter } from 'expo-router';
import { FlatList, Pressable, Text, View } from 'react-native';
import type { HistoryEntry, RuntimeSnapshot } from '../domain';
import { useQuery, useRuntime } from '../runtime';
import { clock, dateLabel } from '../format';
import { Actions, Body, Button, Empty, Loading, Notice, Page, Row, Section, confirm } from '../components/ui';
import { Wave } from '../components/Wave';
import { ownedFailureText } from '../errors';
import { ActiveCleanupVerification } from './CleanupVerification';
import { workflowText } from '../workflow-copy';
import { useHapticFeedback } from '../haptics';

export default function Dictation() {
  const { snapshot, busy, run, t, theme, language } = useRuntime();
  const history = useQuery<HistoryEntry[]>('dictation.history', { limit: 200 }, Boolean(snapshot?.ready));
  const phase = snapshot?.dictation.phase ?? 'idle';
  const router = useRouter();
  const haptic = useHapticFeedback();
  const copy = (key: Parameters<typeof workflowText>[1]) => workflowText(language, key);
  const [now, setNow] = useState(Date.now());
  const [keyboardHelp, setKeyboardHelp] = useState(false);
  const [copied, setCopied] = useState(false);
  const held = useRef(false);
  const heldStart = useRef<Promise<unknown> | null>(null);
  const released = useRef(false);
  const session = snapshot?.session;
  useEffect(() => { if (session?.phase !== 'ready') return; setNow(Date.now()); const timer = setInterval(() => setNow(Date.now()), 1000); return () => clearInterval(timer); }, [session?.phase]);
  useFocusEffect(useCallback(() => { void history.refresh(); }, [history.refresh]));
  useEffect(() => { if (phase === 'idle') void history.refresh(); }, [history.refresh, phase]);
  const start = async () => {
    setCopied(false);
    const result = await run<RuntimeSnapshot>('dictation', 'dictation.start');
    if (result?.dictation.phase === 'listening') haptic('recordingStarted');
    else if (result === undefined) haptic('error');
    return result;
  };
  const stop = async () => {
    const result = await run<RuntimeSnapshot>('dictation', 'dictation.stop');
    if (result !== undefined) haptic(result.dictation.error ? 'error' : 'recordingStopped');
    return result;
  };
  const finishHold = () => {
    if (!held.current || released.current) return;
    released.current = true;
    void heldStart.current?.then((value) => { if (value !== undefined) return stop(); return undefined; });
  };
  useEffect(() => () => { if (held.current && !released.current) { released.current = true; void heldStart.current?.then((value) => { if (value !== undefined) return stop(); return undefined; }); } }, []);
  const copyResult = async (text: string) => { const result = await run('copy', 'dictation.copy', { text }); if (result !== undefined) { setCopied(true); haptic('resultDelivered'); } };
  const sessionTitle = session?.phase === 'ready' ? t('mobile.session.ready', { time: clock(Math.max(0, ((session.expires_at_ms ?? now) - now) / 1000)) }) : session?.phase === 'listening' ? t('dictation.pad.listening') : session?.phase === 'processing' ? t('dictation.pad.processing') : t('mobile.session.off');
  const refreshHistory = async (method: 'dictation.retry' | 'dictation.deleteHistory', payload: Record<string, unknown>) => { await run(`history-${String(payload.id ?? '')}`, method, payload); await history.refresh(); };
  const header = <>
    <Section description={t('mobile.dictation.help')}>
      <Pressable accessibilityRole="button" accessibilityLabel={t(phase === 'listening' ? 'dictation.pad.stop' : phase === 'processing' ? 'dictation.pad.processing' : 'mobile.dictate')} accessibilityHint={copy('resultDestination')} accessibilityState={{ disabled: !snapshot?.ready || phase === 'processing', busy: phase === 'processing' }} disabled={!snapshot?.ready || phase === 'processing'} delayLongPress={220} onPressIn={() => { held.current = false; released.current = false; }} onLongPress={() => { if (phase !== 'idle') return; held.current = true; heldStart.current = start(); }} onPressOut={finishHold} onPress={() => { if (held.current) return; void (phase === 'listening' ? stop() : start()); }} style={({ pressed }) => ({ borderRadius: theme.radius.panel, backgroundColor: theme.colors['pill-background'], padding: theme.space[6], alignItems: 'center', gap: theme.space[3], opacity: pressed ? 0.85 : 1 })}>
        <Wave active={phase === 'listening'} level={snapshot?.dictation.level ?? 0} />
        <Text style={{ fontFamily: theme.font.medium, fontSize: theme.size.section, color: theme.colors['pill-text'] }}>{t(phase === 'listening' ? 'dictation.pad.listening' : phase === 'processing' ? 'dictation.pad.processing' : 'mobile.dictate')}</Text>
      </Pressable>
      {(phase === 'listening' || phase === 'processing') && <Button title={t('common.cancel')} subtle onPress={() => void run('dictation-cancel', 'dictation.cancel').then((result) => { if (result !== undefined) haptic('recordingCancelled'); })} />}
      {(phase === 'listening' || phase === 'processing') && <ActiveCleanupVerification />}
      {snapshot?.dictation.error && <Notice error>{ownedFailureText(language, snapshot.dictation.error, snapshot.dictation.error_code)}</Notice>}
      {snapshot?.dictation.text ? <><Body selectable>{snapshot.dictation.text}</Body><Button title={t('common.copy')} icon="copy-outline" subtle onPress={() => void copyResult(snapshot.dictation.text!)} /></> : null}
      {copied && <Notice>{copy('resultCopied')}</Notice>}
    </Section>
    <Section title={copy('keyboardTitle')}>
      <Row title={sessionTitle} subtitle={session?.error ? ownedFailureText(language, session.error, session.error_code) : undefined} />
      <Button title={t(session && session.phase !== 'off' ? 'mobile.session.stop' : 'mobile.session.start')} subtle busy={busy.has('session')} disabled={!snapshot?.ready} onPress={() => void run('session', session && session.phase !== 'off' ? 'session.stop' : 'session.start')} />
      {session?.phase === 'off' && <Notice>{copy('keyboardInactive')}</Notice>}
      <Button title={copy('keyboardSetup')} subtle icon="keypad-outline" onPress={() => setKeyboardHelp((shown) => !shown)} />
      {keyboardHelp && <><Notice>{copy('keyboardHelp')}</Notice><Notice>{t('mobile.session.help')}</Notice><Notice>{t('mobile.keyboard.setup')}</Notice><Notice>{copy(session?.full_access_seen ? 'keyboardLastVerified' : 'keyboardNotVerified')}</Notice><Button title={t('dictation.pad.settings')} subtle icon="settings-outline" onPress={() => router.push({ pathname: '/settings/[section]', params: { section: 'dictation' } })} /></>}
      {(session?.phase === 'listening' || session?.phase === 'processing') && phase === 'idle' && <ActiveCleanupVerification />}
    </Section>
    <Body accessibilityRole="header" style={{ fontFamily: theme.font.semibold, marginBottom: theme.space[3] }}>{t('dictation.history.title')}</Body>
    {history.loading && !history.value && <Loading />}
    {history.error && <><Notice error>{history.error}</Notice><Button title={t('common.retry')} subtle onPress={() => void history.refresh()} /></>}
  </>;
  return <Page title={t('navigation.dictation')} scroll={false}>
    <FlatList data={history.value ?? []} keyExtractor={(entry) => String(entry.id)} ListHeaderComponent={header} keyboardShouldPersistTaps="handled" contentInsetAdjustmentBehavior="automatic" contentContainerStyle={{ paddingBottom: theme.space[10] }} initialNumToRender={8} maxToRenderPerBatch={8} windowSize={7} renderItem={({ item: entry }) => <View style={{ paddingVertical: theme.space[3], gap: theme.space[2] }}>
      <Body muted style={{ fontSize: theme.size.label }}>{dateLabel(new Date(entry.created_at_ms).toISOString(), language)}</Body>
      <Body selectable>{entry.text || entry.raw}</Body>
      {entry.error && <Notice error>{ownedFailureText(language, entry.error)}</Notice>}
      {entry.status === 'unsaved' && <Notice>{t('dictation.history.temporary')}</Notice>}
      <Actions><Button title={t('common.copy')} subtle onPress={() => void copyResult(entry.text || entry.raw)} /><Button title={t(entry.status === 'unsaved' ? 'dictation.history.save_again' : 'common.retry')} subtle disabled={!entry.has_audio && entry.status !== 'unsaved'} busy={busy.has(`history-${entry.id}`)} onPress={() => void refreshHistory('dictation.retry', { id: entry.id })} /><Button title={t('common.delete')} subtle destructive onPress={() => confirm(t('dictation.history.discard_title'), t('dictation.history.discard_help'), t('common.delete'), t('common.cancel'), () => void refreshHistory('dictation.deleteHistory', { id: entry.id }), true)} /></Actions>
    </View>} ListEmptyComponent={!history.loading && !history.error ? <Empty title={t('dictation.history.empty')} /> : undefined} />
  </Page>;
}
