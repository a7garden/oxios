/**
 * Host detail. Calls `session.list` + `persona.list` RPCs (via the
 * stable client), renders the persona chips at the top and the session
 * cards below. "New session" hits `session.create` and navigates into
 * the chat surface.
 */

import { useCallback, useEffect, useState } from 'react';
import { FlatList, Pressable, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { useLocalSearchParams, useRouter, type Href } from 'expo-router';

import {
  getClient,
  isTransportOffline,
  type ClientHandle
} from '@/services/api';
import type {
  ConnectionState,
  Persona,
  SessionSummary
} from '@/types';
import {
  Badge,
  Button,
  Card,
  Chip,
  EmptyState,
  Header
} from '@/ui/primitives';
import { colors, font, space, text as t } from '@/ui/theme';

export default function HostDetailScreen() {
  const router = useRouter();
  const params = useLocalSearchParams<{ hostId: string }>();
  const hostId = params.hostId;

  const [client, setClient] = useState<ClientHandle | null>(null);
  const [state, setState] = useState<ConnectionState>('connecting');
  const [connError, setConnError] = useState<string | null>(null);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [activePersona, setActivePersona] = useState<Persona | null>(null);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchAll = useCallback(async (c: ClientHandle) => {
    const sessRes = await c.sessionList();
    setSessions(sessRes.sessions ?? []);

    try {
      const personaRes = await c.personaList();
      const list = personaRes.personas ?? [];
      setPersonas(list);
      const active = list.find((p) => p.is_active) ?? list[0] ?? null;
      setActivePersona(active);
    } catch {
      // Personas are optional; don't block the screen.
    }
  }, []);

  useEffect(() => {
    if (!hostId) return;
    let offState: (() => void) | null = null;
    let mounted = true;

    void (async () => {
      try {
        const c = await getClient(hostId)
        if (!mounted) return;
        setClient(c);
        offState = c.onStateChange((s) => {
          if (mounted) setState(s);
        });
      } catch (err) {
        if (!mounted) return;
        setConnError(err instanceof Error ? err.message : String(err));
      }
    })();

    return () => {
      mounted = false;
      if (offState) offState();
    };
  }, [hostId]);

  useEffect(() => {
    if (!client) return;
    if (state !== 'connected') return;
    let cancelled = false;
    void (async () => {
      try {
        await fetchAll(client);
        if (!cancelled) setError(null);
      } catch (err) {
        if (!cancelled) {
          if (isTransportOffline(err)) {
            setConnError('Transport bridge offline — wire runtime adapters first.');
          } else {
            setError(err instanceof Error ? err.message : String(err));
          }
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, state, fetchAll]);

  const onSelectPersona = useCallback(
    async (persona: Persona) => {
      if (!client || persona.id === activePersona?.id) return;
      try {
        await client.personaActivate(persona.id);
        setActivePersona(persona);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [client, activePersona]
  );

  const onCreateSession = useCallback(async () => {
    if (!client) return;
    setBusy(true);
    setError(null);
    try {
      const params: { persona_id?: string } = {};
      if (activePersona) params.persona_id = activePersona.id;
      const created = await client.sessionCreate(params);
      const next: Href = {
        pathname: '/session/[id]',
        params: { id: created.id, hostId }
      };
      router.push(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }, [client, activePersona, hostId, router]);

  const pathLabel = client?.getPath() ?? 'Direct · LAN';
  const subtitle =
    state === 'connected'
      ? `Connected · ${pathLabel}`
      : state === 'connecting'
        ? 'Connecting…'
        : state === 'handshaking'
          ? 'Authenticating…'
          : state === 'reconnecting'
            ? 'Reconnecting…'
            : state === 'auth-failed'
              ? 'Auth failed — re-pair required'
              : connError
                ? connError
                : 'Not connected';

  return (
    <SafeAreaView style={styles.screen} edges={['top']}>
      <Header
        title="Host"
        subtitle={subtitle}
        right={
          <Pressable
            onPress={() => router.back()}
            accessibilityLabel="Back"
            style={styles.backBtn}
          >
            <Text style={styles.backBtnLabel}>Back</Text>
          </Pressable>
        }
      />

      <View style={styles.pathBar}>
        <Badge
          label={pathLabel}
          tone={state === 'connected' ? 'success' : 'warn'}
        />
        {connError ? <Badge label="Bridge offline" tone="danger" /> : null}
      </View>

      {personas.length > 0 ? (
        <View style={styles.personas}>
          <Text style={styles.sectionLabel}>Persona</Text>
          <FlatList
            horizontal
            data={personas}
            keyExtractor={(p) => p.id}
            contentContainerStyle={styles.personaList}
            showsHorizontalScrollIndicator={false}
            renderItem={({ item }) => (
              <Chip
                label={item.name}
                active={item.id === activePersona?.id}
                onPress={() => onSelectPersona(item)}
              />
            )}
          />
        </View>
      ) : null}

      {error ? (
        <View style={styles.errorBanner}>
          <Text style={styles.errorText}>{error}</Text>
        </View>
      ) : null}

      <FlatList
        data={sessions}
        keyExtractor={(item) => item.id}
        contentContainerStyle={styles.sessionList}
        ListHeaderComponent={
          <Text style={[styles.sectionLabel, styles.sectionLabelGap]}>
            Sessions
          </Text>
        }
        renderItem={({ item }) => (
          <Card
            onPress={() => {
              const next: Href = {
                pathname: '/session/[id]',
                params: { id: item.id, hostId }
              };
              router.push(next);
            }}
          >
            <View style={styles.sessionHead}>
              <Text style={t.title} numberOfLines={1}>
                {item.title || 'Untitled session'}
              </Text>
              <Text style={styles.sessionCount}>{item.message_count} msgs</Text>
            </View>
            <Text style={t.caption}>
              Updated {formatRelative(Date.parse(item.updated_at))}
            </Text>
          </Card>
        )}
        ListEmptyComponent={
          state !== 'connected' ? (
            <View style={styles.connectHint}>
              <Text style={t.bodyMuted}>
                Waiting for a working connection. The transport runs an
                endpoint race over LAN + Tailscale and uses 3-success/30s
                hysteresis before promoting a path.
              </Text>
            </View>
          ) : (
            <EmptyState
              title="No sessions yet"
              body="Start a new session — your first message here will select the active persona and open the chat surface."
              action={
                <Button
                  label="New session"
                  onPress={onCreateSession}
                  loading={busy}
                />
              }
            />
          )
        }
        ItemSeparatorComponent={() => <View style={{ height: space.sm }} />}
      />

      <View style={styles.footer}>
        <Button label="New session" onPress={onCreateSession} loading={busy} />
      </View>
    </SafeAreaView>
  );
}

function formatRelative(ms: number): string {
  if (!Number.isFinite(ms)) return '—';
  const delta = Date.now() - ms;
  if (delta < 30_000) return 'just now';
  if (delta < 60_000) return `${Math.floor(delta / 1000)}s ago`;
  if (delta < 3_600_000) return `${Math.floor(delta / 60_000)}m ago`;
  if (delta < 86_400_000) return `${Math.floor(delta / 3_600_000)}h ago`;
  return `${Math.floor(delta / 86_400_000)}d ago`;
}

const styles = StyleSheet.create({
  screen: {
    flex: 1,
    backgroundColor: colors.bg
  },
  backBtn: {
    paddingHorizontal: space.md,
    paddingVertical: space.sm,
    borderColor: colors.borderStrong,
    borderWidth: 1,
    borderRadius: 8
  },
  backBtnLabel: { color: colors.text, fontWeight: '600' },
  pathBar: {
    flexDirection: 'row',
    paddingHorizontal: space.lg,
    paddingBottom: space.md,
    gap: space.sm
  },
  personas: {
    paddingHorizontal: space.lg
  },
  personaList: {
    paddingRight: space.lg
  },
  sectionLabel: {
    ...t.micro,
    textTransform: 'uppercase',
    color: colors.textDim
  },
  sectionLabelGap: {
    paddingHorizontal: space.lg,
    marginTop: space.md,
    marginBottom: space.sm
  },
  sessionList: {
    paddingBottom: 80
  },
  sessionHead: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: 4
  },
  sessionCount: {
    color: colors.textDim,
    fontSize: 12,
    fontFamily: font.mono
  },
  errorBanner: {
    marginHorizontal: space.lg,
    marginBottom: space.sm,
    padding: space.md,
    borderColor: colors.danger,
    borderWidth: 1,
    borderRadius: 8,
    backgroundColor: colors.surface
  },
  errorText: {
    color: colors.danger,
    fontSize: 12,
    fontFamily: font.mono
  },
  connectHint: {
    paddingHorizontal: space.xl,
    paddingTop: space.xl
  },
  footer: {
    position: 'absolute',
    left: space.lg,
    right: space.lg,
    bottom: space.lg
  }
});
