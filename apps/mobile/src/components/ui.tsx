import React from 'react';
import { ActivityIndicator, Alert, KeyboardAvoidingView, Platform, Pressable, ScrollView, StyleSheet, Switch, Text, TextInput, View, type TextInputProps, type TextProps, type ViewStyle } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import Ionicons from '@expo/vector-icons/Ionicons';
import { useRuntime } from '../runtime';
import { localizedFailure } from '../errors';

export function Body({ children, muted = false, style, ...props }: TextProps & { muted?: boolean }) {
  const { theme } = useRuntime();
  return <Text {...props} style={[{ color: muted ? theme.colors.muted : theme.colors.foreground, fontFamily: theme.font.regular, fontSize: theme.size.section, lineHeight: theme.space[6] }, style]}>{children}</Text>;
}

export function Page({ title, subtitle, children, action, scroll = true, inset = true }: { title?: string; subtitle?: string; children: React.ReactNode; action?: React.ReactNode; scroll?: boolean; inset?: boolean }) {
  const { theme, error, store, t, language } = useRuntime();
  const content = <>
    {title && <View style={{ paddingBottom: theme.space[6], flexDirection: 'row', alignItems: 'center', gap: theme.space[3] }}><View style={{ flex: 1 }}><Text accessibilityRole="header" style={{ fontFamily: theme.font.display, fontSize: theme.size['display-title'], color: theme.colors.foreground }}>{title}</Text>{subtitle && <Body muted style={{ marginTop: theme.space[2] }}>{subtitle}</Body>}</View>{action}</View>}
    {error && <View accessibilityRole="alert" style={{ padding: theme.space[3], marginBottom: theme.space[4], backgroundColor: theme.colors.subtle, borderRadius: theme.radius.control }}><Body style={{ color: theme.colors.accent }}>{localizedFailure(language, error)}</Body><Button title={t('common.close')} subtle onPress={store.clearError} /></View>}
    {children}
  </>;
  return <SafeAreaView edges={inset ? ['top', 'left', 'right'] : ['left', 'right']} style={{ flex: 1, backgroundColor: theme.colors.surface }}><KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : undefined} style={{ flex: 1 }}>{scroll ? <ScrollView keyboardShouldPersistTaps="handled" contentContainerStyle={{ padding: theme.space[5], paddingBottom: theme.space[10] }} automaticallyAdjustKeyboardInsets>{content}</ScrollView> : <View style={{ flex: 1, padding: theme.space[5] }}>{content}</View>}</KeyboardAvoidingView></SafeAreaView>;
}

export function Section({ title, description, children }: { title?: string; description?: string; children?: React.ReactNode }) {
  const { theme } = useRuntime();
  return <View style={{ marginBottom: theme.space[7], gap: theme.space[3] }}>{title && <Text accessibilityRole="header" style={{ fontFamily: theme.font.semibold, fontSize: theme.size.section, color: theme.colors.foreground }}>{title}</Text>}{children}{description && <Body muted style={{ fontSize: theme.size.content, lineHeight: theme.space[5] }}>{description}</Body>}</View>;
}

export function Button({ title, onPress, disabled = false, busy = false, subtle = false, destructive = false, icon }: { title: string; onPress: () => void; disabled?: boolean; busy?: boolean; subtle?: boolean; destructive?: boolean; icon?: React.ComponentProps<typeof Ionicons>['name'] }) {
  const { theme } = useRuntime();
  const inactive = disabled || busy;
  return <Pressable accessible accessibilityRole="button" accessibilityLabel={title} accessibilityState={{ disabled: inactive, busy }} disabled={inactive} onPress={onPress} style={({ pressed }) => ({ minHeight: 44, paddingHorizontal: theme.space[4], paddingVertical: theme.space[3], borderRadius: theme.radius.control, backgroundColor: inactive ? theme.colors.disabled : subtle ? pressed ? theme.colors.hover : 'transparent' : theme.colors.button, opacity: pressed && !subtle ? 0.8 : 1, flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: theme.space[2] })}>{busy && <ActivityIndicator color={subtle ? theme.colors.muted : theme.colors['button-text']} />}{icon && <Ionicons accessible={false} importantForAccessibility="no-hide-descendants" name={icon} size={20} color={inactive ? theme.colors['disabled-text'] : subtle ? destructive ? theme.colors.accent : theme.colors.foreground : theme.colors['button-text']} />}<Text style={{ fontFamily: theme.font.medium, fontSize: theme.size.section, color: inactive ? theme.colors['disabled-text'] : subtle ? destructive ? theme.colors.accent : theme.colors.foreground : theme.colors['button-text'], textAlign: 'center' }}>{title}</Text></Pressable>;
}

export function Field({ label, ...props }: TextInputProps & { label: string }) {
  const { theme } = useRuntime();
  return <View style={{ gap: theme.space[2] }}><Text style={{ fontFamily: theme.font.medium, fontSize: theme.size.content, color: theme.colors.muted }}>{label}</Text><TextInput accessibilityLabel={label} placeholderTextColor={theme.colors.muted} selectionColor={theme.colors.accent} {...props} style={[{ minHeight: 44, padding: theme.space[3], borderRadius: theme.radius.control, backgroundColor: theme.colors.field, fontFamily: theme.font.regular, fontSize: theme.size.section, color: theme.colors.foreground, textAlignVertical: props.multiline ? 'top' : 'center' }, props.style]} /></View>;
}

export function Row({ title, subtitle, onPress, trailing, disabled = false }: { title: string; subtitle?: string; onPress?: () => void; trailing?: React.ReactNode; disabled?: boolean }) {
  const { theme } = useRuntime();
  const label = <View style={{ flex: 1 }}><Body style={{ fontFamily: theme.font.medium, color: disabled ? theme.colors['disabled-text'] : theme.colors.foreground }}>{title}</Body>{subtitle && <Body muted style={{ fontSize: theme.size.content }}>{subtitle}</Body>}</View>;
  return <View style={{ minHeight: 52, paddingVertical: onPress ? 0 : theme.space[3], flexDirection: 'row', gap: theme.space[3], alignItems: 'center', borderBottomWidth: StyleSheet.hairlineWidth, borderBottomColor: theme.colors.border }}>
    {onPress ? <Pressable accessible accessibilityRole="button" accessibilityLabel={subtitle ? `${title}. ${subtitle}` : title} accessibilityState={{ disabled }} disabled={disabled} onPress={onPress} style={{ flex: 1, minHeight: 52, paddingVertical: theme.space[3], flexDirection: 'row', gap: theme.space[3], alignItems: 'center' }}>
      {label}
      <Ionicons accessible={false} importantForAccessibility="no-hide-descendants" name="chevron-forward" size={18} color={theme.colors.muted} />
    </Pressable> : label}
    {trailing}
  </View>;
}

export function Toggle({ title, value, onChange, description, disabled = false }: { title: string; value: boolean; onChange: (value: boolean) => void; description?: string; disabled?: boolean }) {
  const { theme } = useRuntime();
  return <Row title={title} subtitle={description} disabled={disabled} trailing={<Switch accessibilityLabel={title} value={value} disabled={disabled} onValueChange={onChange} trackColor={{ false: theme.colors.selection, true: theme.colors.accent }} />} />;
}

export function Choice<T extends string | number>({ label, value, options, onChange }: { label: string; value: T; options: readonly { value: T; label: string; disabled?: boolean }[]; onChange: (value: T) => void }) {
  const { theme } = useRuntime();
  return <View style={{ gap: theme.space[2] }}><Body muted style={{ fontSize: theme.size.content }}>{label}</Body><View accessibilityRole="radiogroup" style={{ gap: theme.space[1] }}>{options.map((option) => <Pressable key={option.value} accessible accessibilityRole="radio" accessibilityLabel={option.label} accessibilityState={{ checked: value === option.value, disabled: option.disabled }} disabled={option.disabled} onPress={() => onChange(option.value)} style={{ minHeight: 44, padding: theme.space[3], borderRadius: theme.radius.control, backgroundColor: value === option.value ? theme.colors.selection : 'transparent', flexDirection: 'row', alignItems: 'center', gap: theme.space[3] }}><Ionicons accessible={false} importantForAccessibility="no-hide-descendants" name={value === option.value ? 'radio-button-on' : 'radio-button-off'} size={20} color={option.disabled ? theme.colors['disabled-text'] : theme.colors.foreground} /><Body style={{ flex: 1, color: option.disabled ? theme.colors['disabled-text'] : theme.colors.foreground }}>{option.label}</Body></Pressable>)}</View></View>;
}

export function Notice({ children, error = false }: { children: React.ReactNode; error?: boolean }) { const { theme } = useRuntime(); return <Body accessibilityRole={error ? 'alert' : undefined} muted={!error} style={{ fontSize: theme.size.content, color: error ? theme.colors.accent : theme.colors.muted }}>{children}</Body>; }
export function Loading() { const { theme } = useRuntime(); return <ActivityIndicator style={{ margin: theme.space[6] }} color={theme.colors.foreground} />; }
export function Empty({ title, description }: { title: string; description?: string }) { const { theme } = useRuntime(); return <View style={{ paddingVertical: theme.space[8], gap: theme.space[2] }}><Body>{title}</Body>{description && <Body muted>{description}</Body>}</View>; }
export function Actions({ children }: { children: React.ReactNode }) { const { theme } = useRuntime(); return <View style={{ flexDirection: 'row', flexWrap: 'wrap', alignItems: 'center', gap: theme.space[2] }}>{children}</View>; }
export function confirm(title: string, message: string, action: string, cancel: string, onConfirm: () => void, destructive = false): void { Alert.alert(title, message, [{ text: cancel, style: 'cancel' }, { text: action, style: destructive ? 'destructive' : 'default', onPress: onConfirm }]); }
