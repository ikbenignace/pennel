import React, { useEffect, useRef, useState } from 'react';
import { AppState, Image } from 'react-native';
import { useLocalSearchParams, usePreventRemove, useRouter } from 'expo-router';
import type { CloudLinkRole, CloudLinkView } from '../../../modules/pennel-mobile/src/types';
import { useRuntime } from '../../runtime';
import { failure } from '../../state';
import { localizedFailure } from '../../errors';
import { linkIsPending, restorableLink } from '../../sync-setup';
import { useHapticFeedback } from '../../haptics';
import { Actions, Body, Button, Field, Loading, Notice, Section, confirm } from '../../components/ui';

export function DeviceLinkPanel() {
  const { snapshot, run, store, t, busy, language } = useRuntime();
  const params = useLocalSearchParams<{ role?: string }>();
  const role: CloudLinkRole = params.role === 'trusted_device' ? 'trusted_device' : 'new_device';
  const router = useRouter();
  const [view, setView] = useState<CloudLinkView | null>(null);
  const [code, setCode] = useState('');
  const [manual, setManual] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [otherConnection, setOtherConnection] = useState(false);
  const completedRequest = useRef<string | null>(null);
  const ownedRequest = useRef<string | null>(null);
  const feedback = useHapticFeedback();
  const pending = view ? linkIsPending(view.phase) : false;
  useEffect(() => {
    setView(null); ownedRequest.current = null; completedRequest.current = null;
    let alive = true;
    void store.client.invoke<CloudLinkView>('cloud.linkView').then((value) => {
      if (!alive) return;
      setView(restorableLink(value, role));
      setOtherConnection(linkIsPending(value.phase) && value.role !== role);
    }).catch(() => {});
    return () => { alive = false; };
  }, [role, store]);
  usePreventRemove(pending, ({ repeat }) => confirm(t('mobile.sync.link_leave'), t('mobile.sync.link_leave_help'), t('mobile.sync.link_cancel'), t('mobile.sync.link_continue'), () => { void run<CloudLinkView>('link-cancel', 'cloud.linkCancel').then((value) => { if (value) { setView(value); repeat(); } }); }));
  useEffect(() => {
    if (!pending) return;
    let alive = true;
    const timer = setTimeout(() => {
      if (AppState.currentState !== 'active') { setView((current) => current ? { ...current } : null); return; }
      void store.client.invoke<CloudLinkView>('cloud.linkPoll').then((value) => {
        if (!alive) return;
        setNotice(null); setView(value); void store.refresh();
      }).catch((error) => { if (alive) { setNotice(localizedFailure(language, failure(error))); setView((current) => current ? { ...current } : null); } });
    }, 2000);
    return () => { alive = false; clearTimeout(timer); };
  }, [language, pending, store, view]);
  useEffect(() => {
    if (view?.phase !== 'complete' || !view.request_id || ownedRequest.current !== view.request_id || completedRequest.current === view.request_id) return;
    completedRequest.current = view.request_id;
    feedback('deviceConnected');
  }, [feedback, view?.phase, view?.request_id]);
  const payload = () => ({ role, device_name: snapshot?.cloud.device_name || 'iPhone', platform: 'ios' });
  const join = async (value: string) => {
    if (!value.trim()) return;
    const next = await run<CloudLinkView>('link-join', 'cloud.linkJoin', { ...payload(), code: value.trim() });
    if (next) { ownedRequest.current = next.request_id; setCode(''); setView(next); setNotice(null); }
  };
  const scan = async () => {
    setScanning(true);
    try {
      const scanned = await store.client.invoke<{ code: string }>('cloud.scanConnectionCode');
      await join(scanned.code);
    } catch (error) {
      const value = failure(error);
      if (value.code !== 'cancelled') { setManual(true); setNotice(localizedFailure(language, value)); }
    } finally { setScanning(false); }
  };
  const start = async () => {
    const next = await run<CloudLinkView>('link-start', 'cloud.linkStart', payload());
    if (next) { ownedRequest.current = next.request_id; setView(next); setNotice(null); }
  };
  const terminal = view && ['cancelled', 'expired', 'failed'].includes(view.phase);
  return <>
    {!view && <Section description={t('mobile.sync.code_help')}>
      {otherConnection && <><Notice>{t('mobile.sync.other_connection')}</Notice><Button title={t('mobile.sync.link_cancel')} onPress={() => { void run<CloudLinkView>('link-cancel', 'cloud.linkCancel').then((value) => { if (value) setOtherConnection(false); }); }} /></>}
      <Button title={t('mobile.sync.scan')} icon="qr-code-outline" disabled={otherConnection} busy={scanning || busy.has('link-join')} onPress={() => void scan()} />
      <Button title={t('mobile.sync.enter_code')} subtle disabled={otherConnection || scanning} onPress={() => setManual(true)} />
      {manual && <>
        <Field label={t('mobile.sync.code_entry')} value={code} onChangeText={setCode} autoCorrect={false} autoCapitalize="characters" autoComplete="off" maxLength={256} returnKeyType="go" onSubmitEditing={() => void join(code)} />
        <Button title={t('mobile.sync.join')} disabled={!code.trim()} busy={busy.has('link-join')} onPress={() => void join(code)} />
      </>}
      <Button title={t('mobile.sync.show_code')} subtle disabled={otherConnection || scanning} busy={busy.has('link-start')} onPress={() => void start()} />
    </Section>}
    {view && <Section title={t(`mobile.sync.${view.phase}`)}>
      {pending && !['awaiting_approval', 'waiting_peer'].includes(view.phase) && <Loading />}
      {view.phase === 'waiting_peer' && <>
        <Notice>{t('mobile.sync.show_help')}</Notice>
        {view.qr_image && <Image source={{ uri: view.qr_image }} accessibilityLabel={t('mobile.sync.qr_label')} style={{ alignSelf: 'center', width: '75%', aspectRatio: 1 }} />}
        <Body muted>{t('mobile.sync.code_entry')}</Body>
        <Body selectable accessibilityLabel={t('mobile.sync.code_entry')} accessibilityValue={{ text: view.connection_code?.split('').join(' ') }}>{view.connection_code}</Body>
        <Notice>{t('mobile.sync.code_private')}</Notice>
      </>}
      {view.peer_name && <Body>{t('mobile.sync.peer', { name: view.peer_name, platform: view.peer_platform || '' })}</Body>}
      {view.account_email && <Notice>{t('account.signed_in_as', { email: view.account_email })}</Notice>}
      {view.verification_code && <><Body>{t('mobile.sync.matches')}</Body><Body selectable>{view.verification_code}</Body></>}
      {view.phase === 'awaiting_approval' && role === 'trusted_device' && <Button title={t('mobile.sync.approve')} busy={busy.has('link-approve')} onPress={() => { void run<CloudLinkView>('link-approve', 'cloud.linkApprove').then((value) => { if (value) setView(value); }); }} />}
      {view.error_message && <Notice error>{localizedFailure(language, { code: view.error_code || 'operation_failed', message: view.error_message })}</Notice>}
      {view.phase === 'complete' && <><Body>{t('mobile.sync.connected_help')}</Body><Button title={t('mobile.sync.view_status')} onPress={() => router.replace({ pathname: '/settings/[section]', params: { section: 'sync' } })} /></>}
      {pending && <Button title={t('mobile.sync.link_cancel')} subtle busy={busy.has('link-cancel')} onPress={() => { void run<CloudLinkView>('link-cancel', 'cloud.linkCancel').then((value) => { if (value) setView(value); }); }} />}
      {terminal && <Actions><Button title={t('mobile.sync.start_again')} onPress={() => { setView(null); setNotice(null); }} /><Button title={t('mobile.sync.email')} subtle onPress={() => router.replace({ pathname: '/settings/[section]', params: { section: 'sync' } })} /></Actions>}
    </Section>}
    {notice && <Notice error>{notice}</Notice>}
  </>;
}
