/**
 * Dark-monochrome UI primitives used by every screen. Tiny — just enough
 * to keep the screens readable without pulling in a UI kit. Inherit
 * colors from `./theme`.
 */

import { StyleSheet, View, Text, Pressable, ActivityIndicator } from 'react-native';
import type { ReactNode } from 'react';
import { colors, radius, space } from './theme';

export function Screen({ children }: { children: ReactNode }) {
  return <View style={styles.screen}>{children}</View>;
}

export function Header({ title, subtitle, right }: {
  title: string;
  subtitle?: string;
  right?: ReactNode;
}) {
  return (
    <View style={styles.header}>
      <View style={{ flex: 1 }}>
        <Text style={styles.h1}>{title}</Text>
        {subtitle ? <Text style={styles.subtitle}>{subtitle}</Text> : null}
      </View>
      {right}
    </View>
  );
}

export function Card({ children, onPress }: {
  children: ReactNode;
  onPress?: () => void;
}) {
  const inner = <View style={styles.card}>{children}</View>;
  if (!onPress) return inner;
  return (
    <Pressable
      onPress={onPress}
      style={({ pressed }) => [pressed && styles.cardPressed]}
      accessibilityRole="button"
    >
      {inner}
    </Pressable>
  );
}

export function Row({ label, value, mono }: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <View style={styles.row}>
      <Text style={styles.rowLabel}>{label}</Text>
      <Text
        style={[styles.rowValue, mono && styles.rowValueMono]}
        numberOfLines={1}
      >
        {value}
      </Text>
    </View>
  );
}

export function Button({
  label,
  onPress,
  disabled,
  loading,
  variant = 'primary'
}: {
  label: string;
  onPress: () => void;
  disabled?: boolean;
  loading?: boolean;
  variant?: 'primary' | 'secondary' | 'ghost';
}) {
  const isPrimary = variant === 'primary';
  const isGhost = variant === 'ghost';
  return (
    <Pressable
      onPress={onPress}
      disabled={disabled || loading}
      accessibilityRole="button"
      accessibilityState={{ disabled: !!disabled }}
      style={({ pressed }) => [
        styles.btn,
        isPrimary
          ? styles.btnPrimary
          : isGhost
            ? styles.btnGhost
            : styles.btnSecondary,
        (disabled || loading) && styles.btnDisabled,
        pressed && styles.btnPressed
      ]}
    >
      {loading ? (
        <ActivityIndicator color={isPrimary ? colors.accentText : colors.text} />
      ) : (
        <Text
          style={[
            styles.btnLabel,
            isPrimary
              ? styles.btnLabelPrimary
              : isGhost
                ? styles.btnLabelGhost
                : styles.btnLabelSecondary
          ]}
        >
          {label}
        </Text>
      )}
    </Pressable>
  );
}

export function Chip({
  label,
  active,
  onPress
}: {
  label: string;
  active?: boolean;
  onPress?: () => void;
}) {
  return (
    <Pressable
      onPress={onPress}
      style={[styles.chip, active && styles.chipActive]}
      accessibilityRole="button"
    >
      <Text style={[styles.chipLabel, active && styles.chipLabelActive]}>
        {label}
      </Text>
    </Pressable>
  );
}

export function Divider() {
  return <View style={styles.divider} />;
}

export function Badge({ label, tone = 'neutral' }: {
  label: string;
  tone?: 'neutral' | 'success' | 'warn' | 'danger';
}) {
  return (
    <View style={[styles.badge, styles[`badge_${tone}` as const]]}>
      <Text style={[styles.badgeLabel, styles[`badgeLabel_${tone}` as const]]}>
        {label}
      </Text>
    </View>
  );
}

export function EmptyState({
  title,
  body,
  action
}: {
  title: string;
  body: string;
  action?: ReactNode;
}) {
  return (
    <View style={styles.empty}>
      <Text style={styles.emptyTitle}>{title}</Text>
      <Text style={styles.emptyBody}>{body}</Text>
      {action ? <View style={styles.emptyAction}>{action}</View> : null}
    </View>
  );
}

const styles = StyleSheet.create({
  screen: {
    flex: 1,
    backgroundColor: colors.bg
  },
  header: {
    paddingHorizontal: space.lg,
    paddingTop: space.lg,
    paddingBottom: space.md,
    flexDirection: 'row',
    alignItems: 'flex-start',
    gap: space.md
  },
  h1: {
    color: colors.text,
    fontSize: 24,
    fontWeight: '700'
  },
  subtitle: {
    color: colors.textMuted,
    fontSize: 13,
    marginTop: 4
  },
  card: {
    backgroundColor: colors.surface,
    borderColor: colors.border,
    borderWidth: 1,
    borderRadius: radius.md,
    padding: space.lg,
    marginHorizontal: space.lg,
    marginBottom: space.sm
  },
  cardPressed: { opacity: 0.7 },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.sm,
    borderBottomColor: colors.border,
    borderBottomWidth: 1
  },
  rowLabel: {
    color: colors.textMuted,
    fontSize: 13
  },
  rowValue: {
    color: colors.text,
    fontSize: 14,
    maxWidth: '60%',
    textAlign: 'right'
  },
  rowValueMono: {
    fontFamily: 'Menlo'
  },
  btn: {
    paddingHorizontal: space.lg,
    paddingVertical: space.md,
    borderRadius: radius.md,
    alignItems: 'center',
    justifyContent: 'center',
    borderWidth: 1,
    borderColor: 'transparent'
  },
  btnPrimary: { backgroundColor: colors.accent, borderColor: colors.accent },
  btnSecondary: {
    backgroundColor: colors.surfaceElev,
    borderColor: colors.borderStrong
  },
  btnGhost: { backgroundColor: 'transparent', borderColor: 'transparent' },
  btnDisabled: { opacity: 0.4 },
  btnPressed: { opacity: 0.8 },
  btnLabel: { fontSize: 15, fontWeight: '600' },
  btnLabelPrimary: { color: colors.accentText },
  btnLabelSecondary: { color: colors.text },
  btnLabelGhost: { color: colors.textMuted },
  chip: {
    paddingHorizontal: space.md,
    paddingVertical: space.sm,
    borderRadius: radius.pill,
    backgroundColor: colors.surfaceElev,
    borderColor: colors.border,
    borderWidth: 1,
    marginRight: space.sm,
    marginBottom: space.sm
  },
  chipActive: {
    backgroundColor: colors.accent,
    borderColor: colors.accent
  },
  chipLabel: { color: colors.textMuted, fontSize: 13 },
  chipLabelActive: { color: colors.accentText },
  divider: {
    height: 1,
    backgroundColor: colors.border,
    marginHorizontal: space.lg,
    marginVertical: space.sm
  },
  badge: {
    paddingHorizontal: space.sm,
    paddingVertical: 4,
    borderRadius: radius.sm,
    borderWidth: 1
  },
  badge_neutral: {
    backgroundColor: colors.surfaceElev,
    borderColor: colors.borderStrong
  },
  badge_success: {
    backgroundColor: 'transparent',
    borderColor: colors.success
  },
  badge_warn: {
    backgroundColor: 'transparent',
    borderColor: colors.warn
  },
  badge_danger: {
    backgroundColor: 'transparent',
    borderColor: colors.danger
  },
  badgeLabel: { fontSize: 11, letterSpacing: 0.4, fontWeight: '600' },
  badgeLabel_neutral: { color: colors.textMuted },
  badgeLabel_success: { color: colors.success },
  badgeLabel_warn: { color: colors.warn },
  badgeLabel_danger: { color: colors.danger },
  empty: {
    flex: 1,
    paddingHorizontal: space.xl,
    alignItems: 'center',
    justifyContent: 'center'
  },
  emptyTitle: {
    color: colors.text,
    fontSize: 18,
    fontWeight: '600',
    marginBottom: space.sm,
    textAlign: 'center'
  },
  emptyBody: {
    color: colors.textMuted,
    fontSize: 14,
    textAlign: 'center',
    lineHeight: 20
  },
  emptyAction: { marginTop: space.xl, alignSelf: 'stretch' }
});
