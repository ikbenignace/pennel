import React from 'react';
import { View } from 'react-native';
import { useReducedMotion, useRuntime } from '../runtime';

export function Wave({ active, level = 0 }: { active: boolean; level?: number }) {
  const { theme } = useRuntime();
  const reducedMotion = useReducedMotion();
  const meter = reducedMotion ? 0 : Math.min(1, Math.max(0, level));
  const heights = [0.35, 0.7, 1, 0.7, 0.35];
  return <View accessible={false} style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: theme.space[1], height: theme.space[10] }}>{heights.map((height, index) => <View key={index} style={{ width: theme.space[1], height: active ? 10 + height * (12 + meter * 18) : 5 + height * 8, borderRadius: theme.radius.pill, backgroundColor: theme.colors.wave }} />)}</View>;
}
