import React, { useState } from 'react';
import { Text, View } from 'react-native';
import { useRouter } from 'expo-router';
import type { ProviderKind, ProviderKindInfo, Settings } from '../domain';
import { useRuntime } from '../runtime';
import { Actions, Body, Button, Choice, Loading, Notice, Page, Row, Section } from '../components/ui';

type SpeechChoice = 'apple' | 'cloud' | 'own';
export default function Onboarding() {
  const { snapshot, run, busy, t, theme } = useRuntime();
  const router = useRouter();
  const [step, setStep] = useState(0);
  const [speech, setSpeech] = useState<SpeechChoice>('apple');
  const [microphone, setMicrophone] = useState(false);
  const settings = snapshot?.settings;
  const saveSettings = (next: Settings) => run<Settings>('onboarding-settings', 'settings.update', { settings: next });
  const finish = async (status: 'completed' | 'skipped') => {
    if (!settings) return;
    const result = await saveSettings({ ...settings, onboarding: { ...settings.onboarding, status } });
    if (result !== undefined) router.replace('/(tabs)');
  };
  const chooseSpeech = async (): Promise<boolean> => {
    if (!settings) return false;
    if (speech === 'own') return Boolean(settings.providers.find((provider) => provider.id === settings.dictation_stt.provider_id && !['apple_speech', 'pennel_cloud'].includes(provider.kind)));
    const kind: ProviderKind = speech === 'cloud' ? 'pennel_cloud' : 'apple_speech';
    let current = settings;
    let provider = current.providers.find((provider) => provider.kind === kind);
    if (!provider) {
      const added = await run<Settings>('onboarding-provider', 'providers.add', { kind });
      if (!added) return false;
      current = added;
      provider = current.providers.find((provider) => provider.kind === kind);
    }
    if (!provider) return false;
    const info = snapshot?.kinds.find((info) => info.kind === kind) as (ProviderKindInfo & { default_stt_model?: string }) | undefined;
    const model = info?.default_stt_model ?? (kind === 'pennel_cloud' ? 'pennel-auto' : 'apple');
    return (await saveSettings({ ...current, dictation_stt: { provider_id: provider.id, model }, meeting_stt: { provider_id: provider.id, model } })) !== undefined;
  };
  const next = async () => { if (step === 2 && !await chooseSpeech()) return; setStep((step) => Math.min(4, step + 1)); };
  if (!snapshot || !settings) return <Page title="Pennel"><Loading /></Page>;
  const currentOwn = settings.providers.find((provider) => provider.id === settings.dictation_stt.provider_id && !['apple_speech', 'pennel_cloud'].includes(provider.kind));
  const canContinue = step !== 2 || (speech === 'cloud' ? snapshot.cloud.signed_in : speech === 'own' ? Boolean(currentOwn) : snapshot.kinds.find((kind) => kind.kind === 'apple_speech')?.availability.status === 'available');
  return <Page title="Pennel">
    <View accessibilityLabel={`${step + 1} / 5`} style={{ flexDirection: 'row', gap: theme.space[2], marginBottom: theme.space[8] }}>{[0, 1, 2, 3, 4].map((value) => <View key={value} style={{ flex: 1, height: theme.space[1], borderRadius: theme.radius.pill, backgroundColor: value <= step ? theme.colors.foreground : theme.colors.selection }} />)}</View>
    {step === 0 && <Section><Text accessibilityRole="header" style={{ fontFamily: theme.font.display, fontSize: theme.size['display-title'], color: theme.colors.foreground }}>{t('mobile.onboarding.title')}</Text><Body>{t('mobile.onboarding.intro')}</Body><Choice label={t('interface.language')} value={settings.ui_language} options={[{ value: 'system', label: t('appearance.system') }, { value: 'en', label: 'English' }, { value: 'nl', label: 'Nederlands' }]} onChange={(ui_language) => void saveSettings({ ...settings, ui_language: ui_language as Settings['ui_language'] })} /></Section>}
    {step === 1 && <Section title={t('mobile.onboarding.microphone')}><Body>{t('mobile.recording.help')}</Body><Button title={t('mobile.onboarding.microphone')} busy={busy.has('microphone-permission')} onPress={() => { void run<{ granted: boolean }>('microphone-permission', 'permissions.microphone').then((result) => setMicrophone(result?.granted === true)); }} />{microphone && <Notice>{t('mobile.saved')}</Notice>}{snapshot.capabilities.microphone?.reason && <Notice>{snapshot.capabilities.microphone.reason}</Notice>}<Button title={t('mobile.open_settings')} subtle onPress={() => void run('open-settings', 'system.openSettings')} /></Section>}
    {step === 2 && <Section title={t('mobile.onboarding.private')}>
      <Choice label={t('common.provider')} value={speech} options={[{ value: 'apple', label: t('mobile.onboarding.apple') }, { value: 'cloud', label: t('mobile.onboarding.cloud') }, { value: 'own', label: t('mobile.onboarding.own') }]} onChange={setSpeech} />
      <Notice>{t(`mobile.onboarding.${speech}_help`)}</Notice>
      {speech === 'apple' && snapshot.kinds.find((kind) => kind.kind === 'apple_speech')?.availability.reason && <Notice>{snapshot.kinds.find((kind) => kind.kind === 'apple_speech')?.availability.reason}</Notice>}
      {speech === 'cloud' && <Button title={t('account.login.sign_in')} subtle onPress={() => router.push({ pathname: '/settings/[section]', params: { section: 'account' } })} />}
      {speech === 'own' && <Button title={t('navigation.providers')} subtle onPress={() => router.push({ pathname: '/settings/[section]', params: { section: 'models' } })} />}
    </Section>}
    {step === 3 && <Section title={t('mobile.onboarding.keyboard')} description={t('mobile.onboarding.keyboard_help')}><Notice>{t('mobile.keyboard.setup')}</Notice><Button title={t('mobile.open_settings')} onPress={() => void run('open-settings', 'system.openSettings')} /><Notice>{t(snapshot.session.full_access_seen ? 'mobile.keyboard.seen' : 'mobile.keyboard.unseen')}</Notice><Notice>{t('mobile.keyboard.goal')}</Notice></Section>}
    {step === 4 && <Section title={t('mobile.onboarding.quick_start')} description={t('mobile.onboarding.quick_start_help')}><Row title={t('mobile.quick_start')} onPress={() => router.push({ pathname: '/settings/[section]', params: { section: 'dictation' } })} /><Notice>{t('mobile.onboarding.private')}</Notice></Section>}
    <Actions>{step > 0 && <Button title={t('common.back')} subtle onPress={() => setStep((step) => Math.max(0, step - 1))} />}{step < 4 ? <Button title={t('mobile.onboarding.next')} disabled={!canContinue} busy={busy.has('onboarding-provider') || busy.has('onboarding-settings')} onPress={() => void next()} /> : <Button title={t('mobile.onboarding.finish')} busy={busy.has('onboarding-settings')} onPress={() => void finish('completed')} />}</Actions>
    <Button title={t('mobile.onboarding.skip')} subtle onPress={() => void finish('skipped')} />
  </Page>;
}
