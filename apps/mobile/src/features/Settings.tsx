import React from 'react';
import { useRouter } from 'expo-router';
import { Page, Row, Section } from '../components/ui';
import { useRuntime } from '../runtime';

export const settingsSections = ['providers', 'models', 'dictation', 'meetings', 'recording', 'dictionary', 'styles', 'appearance', 'account'] as const;
export default function Settings() {
  const { t } = useRuntime();
  const router = useRouter();
  return <Page title={t('common.settings')}><Section>{settingsSections.map((section) => <Row key={section} title={t(`navigation.${section}`)} onPress={() => router.push({ pathname: '/settings/[section]', params: { section } })} />)}</Section></Page>;
}
