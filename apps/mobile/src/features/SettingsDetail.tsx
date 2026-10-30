import React from 'react';
import { Stack, useLocalSearchParams } from 'expo-router';
import { Page } from '../components/ui';
import { useRuntime } from '../runtime';
import { AccountPanel, DevicesPanel, RecoveryPanel } from './settings/AccountPanel';
import { SyncPanel } from './settings/SyncPanel';
import { DeviceLinkPanel } from './settings/DeviceLinkPanel';
import { FeedbackPanel } from './settings/FeedbackPanel';
import { AppearancePanel, DictionaryPanel, DictationPanel, MeetingsPanel, ModelsPanel, ProvidersPanel, RecordingPanel, StylesPanel } from './settings/Panels';

export default function SettingsDetail() {
  const { section } = useLocalSearchParams<{ section: string }>();
  const { snapshot, t } = useRuntime();
  const key = Array.isArray(section) ? section[0] : section;
  const panel = key === 'providers' ? <ProvidersPanel /> : key === 'models' ? <ModelsPanel /> : key === 'dictation' ? <DictationPanel /> : key === 'meetings' ? <MeetingsPanel /> : key === 'recording' ? <RecordingPanel /> : key === 'dictionary' ? <DictionaryPanel /> : key === 'styles' ? <StylesPanel /> : key === 'appearance' ? <AppearancePanel /> : key === 'account' ? <AccountPanel /> : key === 'sync' ? <SyncPanel /> : key === 'connection' ? <DeviceLinkPanel /> : key === 'devices' ? <DevicesPanel /> : key === 'recovery' ? <RecoveryPanel /> : key === 'feedback' ? <FeedbackPanel /> : null;
  return <Page inset={false}><Stack.Screen options={{ title: t(`navigation.${key}`) }} />{snapshot?.ready && panel}</Page>;
}
