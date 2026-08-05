/**
 * Expo Router root layout. Owns the navigation Stack, the dark theme, and
 * the global error boundary — every screen renders under this.
 */

import { Stack } from 'expo-router';
import { StatusBar } from 'expo-status-bar';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { StyleSheet, View, Text, Pressable } from 'react-native';
import { colors, font, space } from '@/ui/theme';
import type { ReactNode } from 'react';

export default function RootLayout() {
  return (
    <SafeAreaProvider>
      <StatusBar style="light" backgroundColor={colors.bg} translucent={false} />
      <Stack
        screenOptions={{
          headerShown: false,
          contentStyle: { backgroundColor: colors.bg },
          animation: 'slide_from_right',
          gestureEnabled: true
        }}
      >
        <Stack.Screen name="index" />
        <Stack.Screen name="pair-scan" options={{ animation: 'slide_from_bottom' }} />
        <Stack.Screen name="pair-confirm" />
        <Stack.Screen
          name="h/[hostId]"
          options={{ animation: 'slide_from_right' }}
        />
        <Stack.Screen
          name="session/[id]"
          options={{ animation: 'slide_from_right' }}
        />
      </Stack>
    </SafeAreaProvider>
  );
}

export function ErrorBoundary({ error, retry }: { error: Error; retry?: () => void }) {
  return (
    <SafeAreaProvider>
      <View style={styles.fallback}>
        <Text style={styles.fallbackTitle}>Something went wrong</Text>
        <Text style={styles.fallbackDetail}>{error.message}</Text>
        {retry ? (
          <Pressable onPress={retry} style={styles.fallbackBtn}>
            <Text style={styles.fallbackBtnLabel}>Try again</Text>
          </Pressable>
        ) : null}
      </View>
    </SafeAreaProvider>
  );
}

export function LoadingShell({ children }: { children?: ReactNode }) {
  return <View style={styles.shell}>{children}</View>;
}

const styles = StyleSheet.create({
  fallback: {
    flex: 1,
    backgroundColor: colors.bg,
    padding: space.xl,
    alignItems: 'center',
    justifyContent: 'center'
  },
  fallbackTitle: {
    color: colors.text,
    fontSize: 20,
    fontWeight: '600',
    marginBottom: space.md
  },
  fallbackDetail: {
    color: colors.danger,
    fontFamily: font.mono,
    fontSize: 13,
    textAlign: 'center',
    marginBottom: space.lg
  },
  fallbackBtn: {
    paddingHorizontal: space.lg,
    paddingVertical: space.md,
    backgroundColor: colors.surfaceElev,
    borderRadius: 10,
    borderWidth: 1,
    borderColor: colors.borderStrong
  },
  fallbackBtnLabel: { color: colors.text, fontWeight: '600' },
  shell: {
    flex: 1,
    backgroundColor: colors.bg
  }
});
