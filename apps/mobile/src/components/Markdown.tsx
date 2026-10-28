import React from 'react';
import { View } from 'react-native';
import { useRuntime } from '../runtime';
import { Body } from './ui';

export function Markdown({ text }: { text: string }) {
  const { theme } = useRuntime();
  return <View style={{ gap: theme.space[3] }}>{text.split(/\n\s*\n/).map((paragraph, index) => {
    const heading = paragraph.match(/^#{1,3}\s+(.+)$/);
    return <Body key={index} selectable accessibilityRole={heading ? 'header' : undefined} style={heading ? { fontFamily: theme.font.semibold, marginTop: theme.space[3], fontSize: theme.size.title } : undefined}>{heading ? heading[1] : paragraph.replace(/^[-*] /gm, '• ')}</Body>;
  })}</View>;
}
