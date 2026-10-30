import React from 'react';
import { useRuntime } from '../../runtime';
import { Actions, Body, Button, Notice, Section } from '../../components/ui';
import type { RecoveryDisplay } from '../../sync-setup';

export function RecoveryBackup({ display, onSaved, onReplace }: { display: RecoveryDisplay | null; onSaved: () => Promise<void>; onReplace: () => Promise<void> }) {
  const { run, t, busy, theme } = useRuntime();
  return <Section title={t('mobile.sync.backup_title')} description={t('mobile.sync.backup_help')}>
    {display ? <>
      <Body selectable accessibilityLabel={t('mobile.cloud.recovery_entry')} accessibilityValue={{ text: display.recovery_code.split('').join(' ') }} style={{ fontFamily: theme.font.medium, fontSize: theme.size.title }}>{display.recovery_code}</Body>
      <Actions>
        <Button title={t('account.recovery.copy')} subtle onPress={() => void run('recovery-copy', 'cloud.recoveryCopy', { confirmation_token: display.confirmation_token })} />
        <Button title={t('account.recovery.print')} subtle onPress={() => void run('recovery-print', 'cloud.recoveryPrint', { confirmation_token: display.confirmation_token })} />
      </Actions>
      <Button title={t('mobile.sync.backup_saved')} busy={busy.has('recovery-confirm')} onPress={() => void onSaved()} />
    </> : <>
      <Notice>{t('mobile.sync.backup_lost_display')}</Notice>
      <Button title={t('mobile.sync.backup_replace')} busy={busy.has('recovery-create')} onPress={() => void onReplace()} />
    </>}
  </Section>;
}
