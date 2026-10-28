import { Capsule, HStack, Link, Spacer, Text, VStack } from '@expo/ui/swift-ui';
import {
  accessibilityHidden,
  activityBackgroundTint,
  font,
  foregroundStyle,
  frame,
  lineLimit,
  monospacedDigit,
  opacity,
  padding,
} from '@expo/ui/swift-ui/modifiers';
import type { LiveActivityEnvironment, LiveActivityLayout } from 'expo-widgets';

import type { PennelActivityProps } from './types';

export function PennelLiveActivity(props: PennelActivityProps, environment: LiveActivityEnvironment): LiveActivityLayout {
  'widget';
  const theme = props.appearance;
  const stale = environment.isStale === true;
  const dimmed = stale || environment.isLuminanceReduced === true;
  const status = stale ? props.labels.unavailable : props.labels.status;
  const showClock = !stale && props.clock.mode !== 'none';
  const wave = (
    <HStack spacing={theme.barSpacing} modifiers={[accessibilityHidden(), opacity(dimmed ? 0.4 : 1)]}>
      {props.wave_heights.map((height, index) => (
        <Capsule key={index} modifiers={[frame({ width: theme.barWidth, height }), foregroundStyle(theme.wave)]} />
      ))}
    </HStack>
  );
  const clock = props.clock.mode === 'remaining' ? (
    <Text
      timerInterval={{ lower: new Date(props.clock.start_at_ms), upper: new Date(props.clock.end_at_ms) }}
      countsDown
      modifiers={[font({ family: theme.fontFamily, size: theme.compactFontSize, textStyle: 'caption' }), monospacedDigit(), foregroundStyle(theme.foreground)]}
    />
  ) : (
    <Text
      date={new Date(props.clock.start_at_ms)}
      dateStyle="timer"
      modifiers={[font({ family: theme.fontFamily, size: theme.compactFontSize, textStyle: 'caption' }), monospacedDigit(), foregroundStyle(theme.foreground)]}
    />
  );
  const statusText = (
    <Text modifiers={[font({ family: theme.fontFamily, size: theme.fontSize, textStyle: 'body' }), foregroundStyle(theme.foreground), lineLimit(2)]}>
      {status}
    </Text>
  );
  const open = (
    <Link
      label={props.labels.open}
      destination={props.open_url}
      modifiers={[font({ family: theme.fontFamily, size: theme.labelFontSize, textStyle: 'callout' }), foregroundStyle(theme.wave)]}
    />
  );
  return {
    banner: (
      <HStack spacing={theme.spacing} modifiers={[padding({ all: theme.inset }), activityBackgroundTint(theme.background)]}>
        {wave}
        <VStack alignment="leading" spacing={theme.spacing}>
          {statusText}
          {showClock ? clock : null}
        </VStack>
        <Spacer />
        {open}
      </HStack>
    ),
    bannerSmall: (
      <VStack alignment="leading" spacing={theme.spacing} modifiers={[padding({ all: theme.inset }), activityBackgroundTint(theme.background)]}>
        {statusText}
        {showClock ? clock : null}
      </VStack>
    ),
    compactLeading: wave,
    compactTrailing: showClock ? clock : (
      <Text modifiers={[font({ family: theme.fontFamily, size: theme.compactFontSize, textStyle: 'caption' }), foregroundStyle(theme.foreground), lineLimit(1)]}>
        {stale ? 'Pennel' : props.labels.compact}
      </Text>
    ),
    minimal: wave,
    expandedLeading: wave,
    expandedCenter: statusText,
    expandedTrailing: showClock ? clock : null,
    expandedBottom: open,
  };
}
