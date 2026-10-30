import React, { useEffect, useRef, useState } from 'react';
import { usePreventRemove, useRouter } from 'expo-router';
import { View } from 'react-native';
import type { CloudDevice } from '../../domain';
import type { CloudRecoveryDisplay } from '../../../modules/pennel-mobile/src/types';
import { useQuery, useRuntime } from '../../runtime';
import { audioUsage, bytes, dateLabel } from '../../format';
import { Body, Button, Field, Loading, Notice, Row, Section, confirm } from '../../components/ui';
import { CloudSignIn } from './CloudSignIn';
import { RecoveryBackup } from './RecoveryBackup';
import { recoveryDisplayFor } from '../../sync-setup';

export function AccountPanel() {
  const { snapshot, run, t, busy, language } = useRuntime();
  const cloud = snapshot!.cloud;
  const router = useRouter();
  useEffect(() => { if (cloud.signed_in) void run('cloud-account', 'cloud.status'); }, [cloud.signed_in, run]);
  if (!cloud.signed_in) return <CloudSignIn />;
  return <>
    <Section title={t('account.account_title')}><Body>{cloud.account ? t('account.signed_in_as', { email: cloud.account.email }) : t('mobile.cloud.unknown_state')}</Body></Section>
    <Section>
      <Row title={t('navigation.sync')} subtitle={t('mobile.sync.intro')} onPress={() => router.push({ pathname: '/settings/[section]', params: { section: 'sync' } })} />
      <Row title={t('navigation.devices')} onPress={() => router.push({ pathname: '/settings/[section]', params: { section: 'devices' } })} />
      <Row title={t('navigation.recovery')} onPress={() => router.push({ pathname: '/settings/[section]', params: { section: 'recovery' } })} />
    </Section>
    {cloud.account && <Section title={t('account.usage_title')}>
      <Row title={t('dictation.processing.audio')} subtitle={t('account.usage.audio', { used: audioUsage(cloud.account.audio_seconds), limit: audioUsage(cloud.account.limit_audio_seconds) })} />
      <Row title={t('account.slot.text')} subtitle={t('account.usage.tokens', { used: cloud.account.tokens.toLocaleString(language), limit: cloud.account.limit_tokens.toLocaleString(language) })} />
      <Row title={t('account.sync.title')} subtitle={t('account.usage.storage', { used: bytes(cloud.account.storage_bytes), limit: bytes(cloud.account.limit_storage_bytes) })} />
    </Section>}
    <Section><Button title={t('account.logout.action')} subtle busy={busy.has('logout')} onPress={() => confirm(t('account.logout.title'), cloud.recovery_confirmed ? t('mobile.cloud.local_warning') : t('account.logout.unconfirmed'), t('account.logout.action'), t('common.cancel'), () => { void run('logout', 'cloud.logout'); })} /></Section>
    <Section title={t('account.delete.title')}><Button title={t('account.delete.title')} subtle destructive busy={busy.has('delete-account')} onPress={() => confirm(t('account.delete.confirm_title'), t('account.delete.detail'), t('account.delete.title'), t('common.cancel'), () => { void run('delete-account', 'cloud.deleteAccount'); }, true)} /></Section>
  </>;
}

export function DevicesPanel() {
  const { snapshot, run, t, language, busy } = useRuntime();
  const cloud = snapshot!.cloud;
  const [name, setName] = useState(cloud.device_name || 'iPhone');
  const devices = useQuery<CloudDevice[]>('cloud.devices', {}, cloud.signed_in);
  if (!cloud.signed_in) return <CloudSignIn />;
  return <Section title={t('account.devices.title')}>
    <Field label={t('account.devices.name')} value={name} onChangeText={setName} />
    <Button title={t('common.save')} subtle disabled={!name.trim()} busy={busy.has('device-name')} onPress={() => { const current = devices.value?.find((device) => device.current); if (current) void run('device-name', 'cloud.renameDevice', { id: current.id, name: name.trim() }).then(() => devices.refresh()); }} />
    {devices.loading && !devices.value && <Loading />}
    {devices.error && <><Notice error>{devices.error}</Notice><Button title={t('common.retry')} subtle onPress={() => void devices.refresh()} /></>}
    {(devices.value ?? []).map((device) => <View key={device.id}>
      <Row title={`${device.name}${device.current ? ` ${t('account.devices.current')}` : ''}`} subtitle={`${device.platform} · ${dateLabel(device.last_seen_iso, language)}`} />
      <Button title={t('account.devices.revoke')} subtle destructive onPress={() => confirm(t('account.devices.revoke_title'), t('account.devices.revoke_detail'), t('account.devices.revoke'), t('common.cancel'), () => { void run(`device-revoke-${device.id}`, 'cloud.revoke', { id: device.id }).then(() => devices.refresh()); }, true)} />
    </View>)}
  </Section>;
}

export function RecoveryPanel() {
  const { snapshot, run, store, t, busy } = useRuntime();
  const cloud = snapshot!.cloud;
  const [recovery, setRecovery] = useState<CloudRecoveryDisplay | null>(null);
  const shown = recoveryDisplayFor(cloud, recovery);
  const previousAccount = useRef(cloud.account_epoch);
  const [showReset, setShowReset] = useState(false);
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    if (!cloud.signed_in || previousAccount.current !== cloud.account_epoch) {
      previousAccount.current = cloud.account_epoch; setRecovery(null); setShowReset(false); setSaved(false);
    } else if (recovery && !recoveryDisplayFor(cloud, recovery)) setRecovery(null);
  }, [cloud.signed_in, cloud.account_epoch, cloud.recovery_display_token, recovery]);
  useEffect(() => {
    if (!cloud.signed_in || cloud.recovery_confirmed) return;
    let alive = true;
    void run<CloudRecoveryDisplay>('recovery-pending', 'cloud.pendingRecovery').then((value) => { if (alive) setRecovery(recoveryDisplayFor(store.getSnapshot().snapshot?.cloud, value)); });
    return () => { alive = false; };
  }, [run, cloud.recovery_confirmed, cloud.signed_in, cloud.account_epoch, cloud.recovery_display_token, store]);
  usePreventRemove(shown !== null && !saved, ({ repeat }) => confirm(t('mobile.sync.backup_leave'), t('mobile.sync.backup_leave_help'), t('mobile.sync.leave'), t('mobile.sync.stay'), () => { setRecovery(null); repeat(); }));
  const codeResult = (value: CloudRecoveryDisplay | undefined) => { setSaved(false); setRecovery(recoveryDisplayFor(store.getSnapshot().snapshot?.cloud, value)); };
  const create = async () => codeResult(await run<CloudRecoveryDisplay>('recovery-create', 'cloud.recoveryCreate'));
  const finish = async () => { if (!shown) return; const value = await run('recovery-confirm', 'cloud.recoveryConfirm', { confirmation_token: shown.confirmation_token }); if (value !== undefined) { setSaved(true); setRecovery(null); } };
  if (!snapshot!.cloud.signed_in) return <CloudSignIn />;
  return <>
    {shown ? <RecoveryBackup display={shown} onSaved={finish} onReplace={create} /> : <>
      <Section title={t('account.recovery.title')} description={t('mobile.sync.rotate_help')}>
        <Notice>{t(snapshot!.cloud.recovery_confirmed ? 'mobile.cloud.confirmed' : 'account.recovery.unconfirmed')}</Notice>
        <Button title={t('account.recovery.new')} subtle busy={busy.has('recovery-create')} onPress={() => confirm(t('account.recovery.new_title'), t('account.recovery.new_detail'), t('account.recovery.new'), t('common.cancel'), () => void create())} />
      </Section>
      <Section title={t('account.recovery.lost')} description={t('mobile.sync.recovery_lost_help')}>
        <Button title={t('mobile.sync.show_reset')} subtle onPress={() => setShowReset(true)} />
        {showReset && <Button title={t('account.recovery.reset')} subtle destructive busy={busy.has('recovery-reset')} onPress={() => confirm(t('account.recovery.reset_title'), t('account.recovery.reset_detail'), t('account.recovery.reset'), t('common.cancel'), () => { void run<CloudRecoveryDisplay>('recovery-reset', 'cloud.recoveryReset').then(codeResult); }, true)} />}
      </Section>
    </>}
  </>;
}
