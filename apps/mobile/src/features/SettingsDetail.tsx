import React from 'react';
import { Stack, useLocalSearchParams } from 'expo-router';
import { Page } from '../components/ui';
import { useRuntime } from '../runtime';
import { AccountPanel } from './settings/AccountPanel';
import { AppearancePanel, DictionaryPanel, DictationPanel, MeetingsPanel, ModelsPanel, ProvidersPanel, RecordingPanel, StylesPanel } from './settings/Panels';

export default function SettingsDetail() {
  const { section } = useLocalSearchParams<{ section: string }>();
  const { snapshot, t } = useRuntime();
  const key = Array.isArray(section) ? section[0] : section;
  const panel = key === 'providers' ? <ProvidersPanel /> : key === 'models' ? <ModelsPanel /> : key === 'dictation' ? <DictationPanel /> : key === 'meetings' ? <MeetingsPanel /> : key === 'recording' ? <RecordingPanel /> : key === 'dictionary' ? <DictionaryPanel /> : key === 'styles' ? <StylesPanel /> : key === 'appearance' ? <AppearancePanel /> : key === 'account' ? <AccountPanel /> : null;
  return <Page title={t(`navigation.${key}`)} inset={false}><Stack.Screen options={{ title: t(`navigation.${key}`) }} />{snapshot?.ready && panel}</Page>;
}
