import React, { useEffect, useRef, useState } from 'react';
import { Linking, Text, View } from 'react-native';
import type { CloudDevice, LoginStart } from '../../domain';
import { useQuery, useRuntime } from '../../runtime';
import { audioUsage, bytes, dateLabel } from '../../format';
import { localizedFailure } from '../../errors';
import { Actions, Body, Button, Field, Loading, Notice, Row, Section, Toggle, confirm } from '../../components/ui';

export function AccountPanel() {
  const { snapshot, run, t, busy, theme, language } = useRuntime();
  const cloud = snapshot!.cloud;
  const [deviceName, setDeviceName] = useState(cloud.device_name || 'iPhone');
  const [login, setLogin] = useState<LoginStart | null>(cloud.login?.start ?? null);
  const [expired, setExpired] = useState(false);
  const [interval, setPollInterval] = useState(5);
  const loginDeadline = useRef(0);
  const [recovery, setRecovery] = useState<string | null>(null);
  const [unlock, setUnlock] = useState('');
  const [notice, setNotice] = useState<string | null>(null);
  const devices = useQuery<CloudDevice[]>('cloud.devices', {}, cloud.signed_in);
  useEffect(() => { if (cloud.signed_in) { setLogin(null); setExpired(false); } }, [cloud.signed_in]);
  useEffect(() => {
    if (!login || cloud.signed_in) return;
    if (!loginDeadline.current) loginDeadline.current = login.expires_at_ms ?? Date.now() + 15 * 60 * 1000;
    let alive = true;
    const timer = setTimeout(() => {
      if (Date.now() >= loginDeadline.current) { setLogin(null); setExpired(true); return; }
      void run<{ status: 'pending' | 'slow_down' | 'done' | 'expired' }>('login-poll', 'cloud.loginPoll').then((value) => {
        if (!alive) return;
        if (!value) { setPollInterval((previous) => Math.min(30, Math.max(5, previous))); setLogin((previous) => previous ? { ...previous } : null); return; }
        if (value.status === 'expired') { setLogin(null); setExpired(true); }
        else if (value.status === 'done') { setLogin(null); void devices.refresh(); }
        else if (value.status === 'slow_down') setPollInterval((previous) => previous + 5);
        else setLogin((previous) => previous ? { ...previous } : null);
      });
    }, interval * 1000);
    return () => { alive = false; clearTimeout(timer); };
  }, [cloud.signed_in, devices.refresh, interval, login, run]);
  const begin = async () => {
    setExpired(false); loginDeadline.current = 0;
    const start = await run<LoginStart>('login-start', 'cloud.loginStart', { device_name: deviceName.trim() || 'iPhone' });
    if (start) { setLogin(start); setPollInterval(Math.max(1, start.interval_secs)); loginDeadline.current = start.expires_at_ms ?? Date.now() + 15 * 60 * 1000; }
  };
  const showNewCode = (value: { recovery_code: string | null } | undefined) => { if (value?.recovery_code) setRecovery(value.recovery_code); };
  const createCode = () => { void run<{ recovery_code: string }>('recovery-create', 'cloud.recoveryCreate').then(showNewCode); };
  const statusKey = cloud.status === 'synced' ? 'account.sync.synced' : cloud.status === 'offline' ? 'account.sync.offline' : cloud.status === 'pending' ? 'account.sync.first_pending' : ['needs_recovery_code', 'vault_reset'].includes(cloud.status) ? 'mobile.cloud.recovery_required' : 'account.sync.first_pending';
  if (!cloud.signed_in) return <Section description={t('account.login.intro')}>
    {login ? <><Body>{t('account.login.browser_code')}</Body><Text selectable accessibilityLabel={t('common.name')} style={{ fontFamily: theme.font.semibold, fontSize: theme.size['display-title'], color: theme.colors.foreground, letterSpacing: 3 }}>{login.user_code}</Text><Button title={t('account.login.reopen_browser')} subtle onPress={() => { if (/^https:\/\//i.test(login.verification_uri)) void Linking.openURL(login.verification_uri).catch(() => setNotice(t('mobile.error'))); }} /><Button title={t('common.cancel')} subtle onPress={() => { void run('login-cancel', 'cloud.loginCancel').then(() => setLogin(null)); }} /></> : <>{expired && <Notice error>{t('account.login.expired')}</Notice>}<Field label={t('mobile.cloud.device_name')} value={deviceName} onChangeText={setDeviceName} autoCapitalize="words" /><Button title={t(expired ? 'account.login.start_again' : 'account.login.sign_in')} busy={busy.has('login-start')} onPress={() => void begin()} /></>}
    {notice && <Notice error>{notice}</Notice>}
  </Section>;
  return <>
    <Section title={t('account.account_title')}><Body>{cloud.account ? t('account.signed_in_as', { email: cloud.account.email }) : t('mobile.cloud.unknown_state')}</Body><Button title={t('account.logout.action')} subtle busy={busy.has('logout')} onPress={() => confirm(t('account.logout.title'), cloud.recovery_confirmed ? t('mobile.cloud.local_warning') : t('account.logout.unconfirmed'), t('account.logout.action'), t('common.cancel'), () => { void run('logout', 'cloud.logout').then(() => setRecovery(null)); })} /></Section>
    {cloud.account && <Section title={t('account.usage_title')}><Row title={t('dictation.processing.audio')} subtitle={t('account.usage.audio', { used: audioUsage(cloud.account.audio_seconds), limit: audioUsage(cloud.account.limit_audio_seconds) })} /><Row title={t('account.slot.text')} subtitle={t('account.usage.tokens', { used: cloud.account.tokens.toLocaleString(language), limit: cloud.account.limit_tokens.toLocaleString(language) })} /><Row title={t('account.sync.title')} subtitle={t('account.usage.storage', { used: bytes(cloud.account.storage_bytes), limit: bytes(cloud.account.limit_storage_bytes) })} /></Section>}
    <Section title={t('account.sync.title')} description={t('account.sync.privacy')}><Toggle title={t('account.sync.title')} value={cloud.sync_enabled} disabled={busy.has('sync-setting')} onChange={(enabled) => { void run<{ recovery_code: string | null }>('sync-setting', 'cloud.setSync', { enabled }).then(showNewCode); }} /><Notice>{cloud.syncing ? t('account.sync.busy') : t(statusKey)}</Notice>{cloud.notice && <Notice error>{cloud.brake ? t('account.sync.confirm_deletes_detail') : localizedFailure(language, { code: cloud.status, message: cloud.notice })}</Notice>}{cloud.sync_enabled && <Button title={t('account.sync.now')} subtle busy={cloud.syncing || busy.has('sync')} onPress={() => void run('sync', 'cloud.sync')} />}{cloud.brake && <Button title={t('account.sync.confirm_deletes')} subtle onPress={() => confirm(t('account.sync.confirm_deletes_title'), t('account.sync.confirm_deletes_detail'), t('account.sync.confirm_deletes'), t('common.cancel'), () => void run('sync-confirm-deletes', 'cloud.confirmDeletes'), true)} />}{cloud.status === 'needs_setup' && !recovery && <Button title={t('account.recovery.create')} subtle onPress={createCode} />}</Section>
    {recovery && <Section title={t('account.recovery.title')} description={t('account.recovery.description')}><Notice>{t('mobile.cloud.show_code')}</Notice><Body selectable style={{ fontFamily: theme.font.medium, fontSize: theme.size.title }}>{recovery}</Body><Actions><Button title={t('account.recovery.copy')} subtle onPress={() => { void run('recovery-copy', 'cloud.recoveryCopy', { code: recovery }).then(() => setNotice(t('mobile.cloud.recovery_copied'))); }} /><Button title={t('account.recovery.print')} subtle onPress={() => void run('recovery-print', 'cloud.recoveryPrint', { code: recovery })} /><Button title={t('account.recovery.saved')} onPress={() => { void run('recovery-confirm', 'cloud.recoveryConfirm').then((value) => { if (value !== undefined) { setRecovery(null); setNotice(t('mobile.cloud.confirmed')); } }); }} /></Actions></Section>}
    {['needs_recovery_code', 'vault_reset'].includes(cloud.status) && <Section title={t('account.recovery.unlock')} description={t('mobile.cloud.recovery_required')}><Field label={t('mobile.cloud.recovery_entry')} value={unlock} onChangeText={setUnlock} autoCorrect={false} autoCapitalize="none" secureTextEntry autoComplete="off" /><Button title={t('account.recovery.unlock')} busy={busy.has('recovery-unlock')} disabled={!unlock.trim()} onPress={() => { void run('recovery-unlock', 'cloud.recoveryUnlock', { code: unlock.trim() }).then((value) => { if (value !== undefined) setUnlock(''); }); }} /></Section>}
    {!recovery && <Section title={t('account.recovery.title')}><Notice>{t(cloud.recovery_confirmed ? 'mobile.cloud.confirmed' : 'account.recovery.unconfirmed')}</Notice><Button title={t('account.recovery.new')} subtle onPress={() => confirm(t('account.recovery.new_title'), t('account.recovery.new_detail'), t('account.recovery.new'), t('common.cancel'), createCode)} /><Button title={t('account.recovery.lost')} subtle destructive onPress={() => confirm(t('account.recovery.reset_title'), t('account.recovery.reset_detail'), t('account.recovery.reset'), t('common.cancel'), () => { void run<{ recovery_code: string }>('recovery-reset', 'cloud.recoveryReset').then(showNewCode); }, true)} /></Section>}
    <Section title={t('account.devices.title')}>
      <Field label={t('account.devices.name')} value={deviceName} onChangeText={setDeviceName} />
      <Button title={t('common.save')} subtle disabled={!deviceName.trim()} onPress={() => { const current = devices.value?.find((device) => device.current); if (current) void run('device-name', 'cloud.renameDevice', { id: current.id, name: deviceName.trim() }).then(() => devices.refresh()); }} />
      {devices.loading && !devices.value && <Loading />}{devices.error && <Notice error>{devices.error}</Notice>}
      {(devices.value ?? []).map((device) => <View key={device.id}><Row title={`${device.name}${device.current ? ` ${t('account.devices.current')}` : ''}`} subtitle={`${device.platform} · ${dateLabel(device.last_seen_iso, language)}`} /><Button title={t('account.devices.revoke')} subtle destructive onPress={() => confirm(t('account.devices.revoke_title'), t('account.devices.revoke_detail'), t('account.devices.revoke'), t('common.cancel'), () => { void run(`device-revoke-${device.id}`, 'cloud.revoke', { id: device.id }).then(() => devices.refresh()); }, true)} /></View>)}
    </Section>
    {notice && <Section><Notice>{notice}</Notice></Section>}
    <Section title={t('account.delete.title')}><Button title={t('account.delete.title')} subtle destructive busy={busy.has('delete-account')} onPress={() => confirm(t('account.delete.confirm_title'), t('account.delete.detail'), t('account.delete.title'), t('common.cancel'), () => { void run('delete-account', 'cloud.deleteAccount').then(() => setRecovery(null)); }, true)} /></Section>
  </>;
}
