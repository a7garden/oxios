/**
 * Paired hosts list. Reads host profiles from the SecureStore-backed
 * keychain (via `listHostProfiles`) and renders an empty state with
 * a "Scan QR" CTA when nothing is paired yet. Each card opens the
 * host's sessions list at `h/[hostId]`.
 */

import { useCallback, useEffect, useState } from 'react';
import { FlatList, RefreshControl, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { useFocusEffect, useRouter } from 'expo-router';

import {
  dropClient,
  hosts as hostsApi,
  isTransportOffline
} from '@/services/api';
import type { HostProfile } from '@/types';
import {
  Badge,
  Button,
  Card,
  EmptyState,
  Header,
  Row,
  Screen
} from '@/ui/primitives';
import { colors, font, space, text as t } from '@/ui/theme';

export default function HostsIndex() {
  const router = useRouter();
  const [items, setItems] = useState<HostProfile[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [transportOffline, setTransportOffline] = useState(false);

  const load = useCallback(async () => {
    try {
      const list = await hostsApi.list();
      setItems(list);
      setError(null);
      setTransportOffline(false);
    } catch (err) {
      if (isTransportOffline(err)) {
        setTransportOffline(true);
        setError('Transport bridge not wired yet.');
      } else {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      setLoaded(true);
    }
  }, []);

  useFocusEffect(
    useCallback(() => {
      load();
    }, [load])
  );

  useEffect(() => {
    load();
  }, [load]);

  const onRefresh = useCallback(async () => {
    setRefreshing(true);
    await load();
    setRefreshing(false);
  }, [load]);

  const goScan = () => router.push('/pair-scan');

  const onRemove = useCallback(
    async (id: string) => {
      await hostsApi.remove(id);
      await dropClient(id).catch(() => undefined);
      await load();
    },
    [load]
  );

  const headerRight = (
    <Button label="Scan QR" onPress={goScan} variant="secondary" />
  );

  return (
    <Screen>
      <SafeAreaView style={{ flex: 1 }} edges={['top']}>
        <Header
          title="Paired hosts"
          subtitle={
            items.length > 0
              ? `${items.length} connected`
              : 'Direct · LAN or Tailscale'
          }
          right={headerRight}
        />

        {error ? (
          <View style={styles.banner}>
            <Text style={styles.errorText}>
              {transportOffline
                ? 'Transport bridge offline — runtime WebSocket dial not wired.'
                : `Error: ${error}`}
            </Text>
          </View>
        ) : null}

        {!loaded ? (
          <View style={styles.loading}>
            <Text style={t.caption}>Loading…</Text>
          </View>
        ) : items.length === 0 ? (
          <EmptyState
            title="No hosts paired yet"
            body="Scan a QR code from your Oxios desktop to pair this phone. Pairing establishes a Noise_XX end-to-end encrypted channel — the device token lives in SecureStore and never leaves your hardware."
            action={<Button label="Scan QR to pair" onPress={goScan} />}
          />
        ) : (
          <FlatList
            data={items}
            keyExtractor={(item) => item.id}
            contentContainerStyle={styles.list}
            refreshControl={
              <RefreshControl
                refreshing={refreshing}
                onRefresh={onRefresh}
                tintColor={colors.textMuted}
              />
            }
            renderItem={({ item }) => (
              <Card onPress={() => router.push(`/h/${item.id}`)}>
                <View style={styles.cardHead}>
                  <Text style={t.title}>{item.name}</Text>
                  {item.deviceToken ? (
                    <Badge label="Direct" tone="success" />
                  ) : (
                    <Badge label="Unpaired" tone="warn" />
                  )}
                </View>
                <Row label="Endpoint" value={item.endpoint} mono />
                <Row
                  label="Device ID"
                  value={item.deviceId || '—'}
                  mono
                />
                <Row
                  label="Last seen"
                  value={formatRelative(item.lastConnected)}
                />
                <View style={styles.cardActions}>
                  <Button
                    label="Forget"
                    variant="ghost"
                    onPress={() => onRemove(item.id)}
                  />
                </View>
              </Card>
            )}
            ItemSeparatorComponent={() => <View style={{ height: space.sm }} />}
          />
        )}
      </SafeAreaView>
    </Screen>
  );
}

function formatRelative(ts: number): string {
  if (!Number.isFinite(ts) || ts === 0) return 'never';
  const delta = Date.now() - ts;
  if (delta < 30_000) return 'just now';
  if (delta < 60_000) return `${Math.floor(delta / 1000)}s ago`;
  if (delta < 3_600_000) return `${Math.floor(delta / 60_000)}m ago`;
  if (delta < 86_400_000) return `${Math.floor(delta / 3_600_000)}h ago`;
  return `${Math.floor(delta / 86_400_000)}d ago`;
}

const styles = StyleSheet.create({
  list: { paddingTop: space.md, paddingBottom: space.xxl },
  cardHead: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: space.sm
  },
  cardActions: {
    marginTop: space.sm,
    alignItems: 'flex-start'
  },
  loading: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center'
  },
  banner: {
    marginHorizontal: space.lg,
    marginBottom: space.sm,
    padding: space.md,
    backgroundColor: colors.surface,
    borderColor: colors.danger,
    borderWidth: 1,
    borderRadius: 8
  },
  errorText: {
    color: colors.danger,
    fontSize: 12,
    fontFamily: font.mono
  }
});
