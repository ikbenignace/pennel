import React, { useEffect, useRef, useState } from 'react';
import { AppState, Linking, Text } from 'react-native';
import type { LoginStart } from '../../domain';
import { useRuntime } from '../../runtime';
import { Body, Button, Notice, Section } from '../../components/ui';

export function CloudSignIn() {
  const { snapshot, run, store, t, busy, theme } = useRuntime();
  const cloud = snapshot!.cloud;
  const [login, setLogin] = useState<LoginStart | null>(cloud.login?.start ?? null);
  const [expired, setExpired] = useState(false);
  const [interval, setIntervalSeconds] = useState(5);
  const [unavailable, setUnavailable] = useState(false);
  const deadline = useRef(0);
  useEffect(() => { if (cloud.signed_in) { setLogin(null); setExpired(false); } }, [cloud.signed_in]);
  useEffect(() => {
    if (!login || cloud.signed_in) return;
    if (!deadline.current) deadline.current = login.expires_at_ms ?? Date.now() + 15 * 60 * 1000;
    let alive = true;
    const timer = setTimeout(() => {
      if (Date.now() >= deadline.current) { setLogin(null); setExpired(true); return; }
      if (AppState.currentState !== 'active') { setLogin((value) => value ? { ...value } : null); return; }
      void store.client.invoke<{ status: 'pending' | 'slow_down' | 'done' | 'expired' }>('cloud.loginPoll').then((value) => {
        if (!alive) return;
        setUnavailable(false);
        if (value.status === 'expired') { setLogin(null); setExpired(true); }
        else if (value.status === 'done') { setLogin(null); void store.refresh(); }
        else {
          if (value.status === 'slow_down') setIntervalSeconds((previous) => Math.min(30, previous + 5));
          setLogin((previous) => previous ? { ...previous } : null);
        }
      }).catch(() => { if (alive) { setUnavailable(true); setLogin((previous) => previous ? { ...previous } : null); } });
    }, interval * 1000);
    return () => { alive = false; clearTimeout(timer); };
  }, [cloud.signed_in, interval, login, store]);
  const begin = async () => {
    setExpired(false); setUnavailable(false); deadline.current = 0;
    const value = await run<LoginStart>('login-start', 'cloud.loginStart', { device_name: cloud.device_name || 'iPhone' });
    if (!value) return;
    deadline.current = value.expires_at_ms ?? Date.now() + 15 * 60 * 1000;
    setIntervalSeconds(Math.max(1, value.interval_secs)); setLogin(value);
  };
  return <Section description={t('account.login.intro')}>
    {login ? <>
      <Body>{t('mobile.sync.browser_wait')}</Body>
      <Body muted>{t('mobile.sync.sign_in_code')}</Body>
      <Text selectable accessibilityLabel={t('mobile.sync.sign_in_code')} accessibilityValue={{ text: login.user_code.split('').join(' ') }} style={{ fontFamily: theme.font.semibold, fontSize: theme.size.title, color: theme.colors.foreground }}>{login.user_code}</Text>
      {unavailable && <Notice error>{t('account.sync.offline')}</Notice>}
      <Button title={t('account.login.reopen_browser')} subtle onPress={() => { if (/^https:\/\//i.test(login.verification_uri)) void Linking.openURL(login.verification_uri).catch(() => setUnavailable(true)); }} />
      <Button title={t('common.cancel')} subtle onPress={() => { void run('login-cancel', 'cloud.loginCancel').then((value) => { if (value !== undefined) setLogin(null); }); }} />
    </> : <>
      {expired && <Notice error>{t('account.login.expired')}</Notice>}
      <Button title={t(expired ? 'account.login.start_again' : 'mobile.sync.email')} busy={busy.has('login-start')} onPress={() => void begin()} />
      <Notice>{t('mobile.sync.email_help')}</Notice>
    </>}
  </Section>;
}
