import React from 'react';
import { useRouter } from 'expo-router';
import { Page, Row, Section } from '../components/ui';
import { useRuntime } from '../runtime';

export const settingsSections = ['sync', 'dictation', 'meetings', 'recording', 'dictionary', 'styles', 'appearance', 'feedback', 'account', 'providers', 'models'] as const;
export default function Settings() {
  const { t } = useRuntime();
  const router = useRouter();
  const rows = (sections: readonly string[]) => sections.map((section) => <Row key={section} title={t(`navigation.${section}`)} onPress={() => router.push({ pathname: '/settings/[section]', params: { section } })} />);
  return <Page title={t('common.settings')}>
    <Section>{rows(['sync'])}</Section>
    <Section title={t('mobile.sync.settings_tasks')}>{rows(['dictation', 'meetings', 'recording', 'dictionary', 'styles'])}</Section>
    <Section title={t('mobile.sync.settings_personal')}>{rows(['appearance', 'feedback', 'account'])}</Section>
    <Section title={t('mobile.sync.settings_advanced')}>{rows(['providers', 'models'])}</Section>
  </Page>;
}
