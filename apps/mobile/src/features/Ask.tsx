import React from 'react';
import { useRouter } from 'expo-router';
import { AskPanel } from '../components/AskPanel';
import { Button, Notice, Page, Section } from '../components/ui';
import { useRuntime } from '../runtime';

export default function Ask() {
  const { snapshot, t } = useRuntime();
  const router = useRouter();
  const ready = Boolean(snapshot?.settings.llm.provider_id);
  return <Page title={t('mobile.ask')} subtitle={t('meetings.ask_example_all')}>
    {!ready && <Section><Notice>{t('meetings.questions_setup_help')}</Notice><Button title={t('meetings.set_up_questions')} subtle onPress={() => router.push({ pathname: '/settings/[section]', params: { section: 'models' } })} /></Section>}
    {snapshot?.ready && <AskPanel />}
  </Page>;
}
