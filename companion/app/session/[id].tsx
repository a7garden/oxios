/**
 * Chat surface for a single session. Lists messages, sends new ones
 * via `chatSend`, subscribes to the daemon's streaming events via
 * `chatSubscribe`, and surfaces agent lifecycle events from
 * `agentStatus` in the header.
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import {
  FlatList,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { useLocalSearchParams, useRouter } from 'expo-router';

import {
  getClient,
  isTransportOffline,
  type ClientHandle
} from '@/services/api';
import type {
  AgentStatus,
  ChatMessage,
  ConnectionState,
  SessionSummary,
  SubscribeEvent
} from '@/types';
import { Badge, Header } from '@/ui/primitives';
import { colors, font, radius, space, text as t } from '@/ui/theme';

export default function ChatScreen() {
  const router = useRouter();
  const params = useLocalSearchParams<{ id: string; hostId: string }>();
  const sessionId = params.id;
  const hostId = params.hostId;

  const [client, setClient] = useState<ClientHandle | null>(null);
  const [state, setState] = useState<ConnectionState>('connecting');
  const [session, setSession] = useState<SessionSummary | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [agent, setAgent] = useState<AgentStatus>({ state: 'idle' });
  const [input, setInput] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const listRef = useRef<FlatList<ChatMessage> | null>(null);

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
        if (mounted) {
          if (isTransportOffline(err)) {
            setError('Transport bridge offline — wire runtime adapters first.');
          } else {
            setError(err instanceof Error ? err.message : String(err));
          }
        }
      }
    })();
    return () => {
      mounted = false;
      if (offState) offState();
    };
  }, [hostId]);

  // Subscribe to streaming events once connected.
  useEffect(() => {
    if (!client || !sessionId || state !== 'connected') return;
    const off = client.chatSubscribe(sessionId, (event: unknown) => {
      const parsed = parseEvent(event, sessionId);
      if (parsed.kind === 'message') {
        setMessages((prev) => upsertById(prev, parsed.message));
      } else if (parsed.kind === 'agent.status') {
        setAgent({ state: parsed.state, detail: parsed.detail });
      }
      // 'pong' / 'unknown' are advisory; nothing to render.
    });
    return off;
  }, [client, sessionId, state]);

  // Surface agent lifecycle events separately so they appear even when
  // the session-id-keyed chat stream is silent (idle states).
  useEffect(() => {
    if (!client || state !== 'connected') return;
    return client.agentStatus((event: unknown) => {
      const parsed = parseEvent(event, sessionId);
      if (parsed.kind === 'agent.status') {
        setAgent({ state: parsed.state, detail: parsed.detail });
      }
    });
  }, [client, sessionId, state]);

  // Load session metadata when connected.
  useEffect(() => {
    if (!client || !sessionId || state !== 'connected') return;
    let cancelled = false;
    void (async () => {
      try {
        const sessions = await client.sessionList();
        if (cancelled) return;
        setSession(sessions.sessions.find((s) => s.id === sessionId) ?? null);
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, sessionId, state]);

  const onSend = useCallback(async () => {
    if (!client || !sessionId) return;
    const content = input.trim();
    if (content.length === 0) return;

    // Optimistic: render the user message immediately so the composer
    // looks responsive even if the daemon is slow to acknowledge.
    const optimistic: ChatMessage = {
      id: `local-${Date.now()}`,
      role: 'user',
      content,
      created_at: new Date().toISOString()
    };
    setMessages((prev) => upsertById(prev, optimistic));
    setInput('');
    setSending(true);
    setError(null);
    try {
      await client.chatSend({ content, session_id: sessionId });
    } catch (err) {
      setMessages((prev) => prev.filter((m) => m.id !== optimistic.id));
      setInput(content);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSending(false);
    }
  }, [client, input, sessionId]);

  const isStreaming = agent.state === 'thinking' || agent.state === 'acting';
  const connBadge =
    state === 'connected'
      ? { label: client?.getPath() ?? 'Direct', tone: 'success' as const }
      : state === 'connecting'
        ? { label: 'Connecting', tone: 'warn' as const }
        : state === 'handshaking'
          ? { label: 'Authenticating', tone: 'warn' as const }
          : state === 'reconnecting'
            ? { label: 'Reconnecting', tone: 'warn' as const }
            : state === 'auth-failed'
              ? { label: 'Auth failed', tone: 'danger' as const }
              : { label: 'Offline', tone: 'danger' as const };

  return (
    <SafeAreaView style={styles.screen} edges={['top']}>
      <Header
        title={session?.title ?? 'Session'}
        subtitle={
          isStreaming
            ? (agent.detail ?? `${agent.state}…`)
            : state === 'connected'
              ? 'Ready'
              : 'Waiting for connection'
        }
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

      <View style={styles.statusRow}>
        <Badge label={connBadge.label} tone={connBadge.tone} />
        {isStreaming ? (
          <Badge label={agent.state} tone="neutral" />
        ) : (
          <Badge label="idle" tone="neutral" />
        )}
      </View>

      {error ? (
        <View style={styles.errorBanner}>
          <Text style={styles.errorText}>{error}</Text>
        </View>
      ) : null}

      <KeyboardAvoidingView
        behavior={Platform.OS === 'ios' ? 'padding' : undefined}
        style={styles.flex}
        keyboardVerticalOffset={0}
      >
        <FlatList
          ref={(r) => {
            listRef.current = r;
          }}
          data={messages}
          keyExtractor={(m) => m.id}
          contentContainerStyle={styles.list}
          renderItem={({ item }) => (
            <MessageBubble role={item.role} content={item.content} />
          )}
          ListEmptyComponent={
            <View style={styles.empty}>
              <Text style={t.bodyMuted}>
                Send the first message to start this session. Your conversation
                is end-to-end encrypted end to end.
              </Text>
            </View>
          }
          onContentSizeChange={() =>
            listRef.current?.scrollToEnd({ animated: false })
          }
        />

        <View style={styles.composer}>
          <TextInput
            value={input}
            onChangeText={setInput}
            placeholder="Message"
            placeholderTextColor={colors.textDim}
            editable={state === 'connected' && !sending}
            multiline
            style={styles.composerInput}
            onSubmitEditing={onSend}
            blurOnSubmit={false}
            returnKeyType="send"
          />
          <Pressable
            disabled={
              state !== 'connected' || sending || input.trim().length === 0
            }
            onPress={onSend}
            style={({ pressed }) => [
              styles.sendBtn,
              (state !== 'connected' || sending || input.trim().length === 0) &&
                styles.sendBtnDisabled,
              pressed && styles.sendBtnPressed
            ]}
            accessibilityRole="button"
            accessibilityLabel="Send message"
          >
            <Text style={styles.sendBtnLabel}>Send</Text>
          </Pressable>
        </View>
      </KeyboardAvoidingView>
    </SafeAreaView>
  );
}

function MessageBubble({
  role,
  content
}: {
  role: ChatMessage['role'];
  content: string;
}) {
  const isUser = role === 'user';
  return (
    <View
      style={[
        styles.bubbleRow,
        { justifyContent: isUser ? 'flex-end' : 'flex-start' }
      ]}
    >
      <View
        style={[
          styles.bubble,
          isUser ? styles.bubbleUser : styles.bubbleAssistant
        ]}
      >
        <Text
          style={[
            styles.bubbleText,
            isUser ? styles.bubbleTextUser : styles.bubbleTextAssistant
          ]}
        >
          {content}
        </Text>
      </View>
    </View>
  );
}

function upsertById(prev: ChatMessage[], next: ChatMessage): ChatMessage[] {
  const idx = prev.findIndex((m) => m.id === next.id);
  if (idx === -1) return [...prev, next];
  const copy = prev.slice();
  copy[idx] = next;
  return copy;
}

/**
 * Validate and normalise a raw streaming event from the transport.
 * The daemon emits JSON-RPC notifications; the shape is intentionally
 * permissive so a partial schema does not crash the chat surface.
 */
function parseEvent(raw: unknown, expectedSessionId: string): SubscribeEvent {
  if (!raw || typeof raw !== 'object') return { kind: 'unknown' };
  const obj = raw as Record<string, unknown>;

  if (typeof obj.kind === 'string') {
    if (obj.kind === 'message') {
      const sid = typeof obj.session_id === 'string' ? obj.session_id : '';
      const msg = obj.message;
      if (typeof msg === 'object' && msg && !Array.isArray(msg)) {
        const m = msg as Record<string, unknown>;
        const content = typeof m.content === 'string' ? m.content : '';
        const id =
          typeof m.id === 'string' && m.id
            ? m.id
            : `evt-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
        if (sid === expectedSessionId) {
          return {
            kind: 'message',
            session_id: sid,
            message: {
              id,
              role:
                m.role === 'assistant' || m.role === 'system'
                  ? m.role
                  : 'user',
              content,
              created_at:
                typeof m.created_at === 'string'
                  ? m.created_at
                  : new Date().toISOString()
            }
          };
        }
      }
    }
    if (obj.kind === 'agent.status') {
      const sid = typeof obj.session_id === 'string' ? obj.session_id : expectedSessionId;
      const state =
        obj.state === 'thinking' ||
        obj.state === 'acting' ||
        obj.state === 'waiting' ||
        obj.state === 'error'
          ? obj.state
          : 'idle';
      const detail = typeof obj.detail === 'string' ? obj.detail : undefined;
      return { kind: 'agent.status', session_id: sid, state, detail };
    }
    if (obj.kind === 'pong') {
      const ts = typeof obj.ts === 'number' ? obj.ts : Date.now();
      return { kind: 'pong', ts };
    }
  }

  // JSON-RPC fallback: daemon-pushed notifications may also arrive as
  // `{ method, params }` envelopes.
  if (typeof obj.method === 'string' && obj.method === 'chat.message') {
    const params = obj.params as Record<string, unknown> | undefined;
    if (params && typeof params === 'object') {
      return parseEvent(
        { kind: 'message', session_id: params.session_id, message: params.message },
        expectedSessionId
      );
    }
  }
  if (typeof obj.method === 'string' && obj.method === 'agent.status') {
    const params = obj.params as Record<string, unknown> | undefined;
    if (params && typeof params === 'object') {
      return parseEvent(
        { kind: 'agent.status', session_id: params.session_id, state: params.state, detail: params.detail },
        expectedSessionId
      );
    }
  }

  return { kind: 'unknown' };
}

const styles = StyleSheet.create({
  flex: { flex: 1 },
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
  statusRow: {
    flexDirection: 'row',
    paddingHorizontal: space.lg,
    paddingBottom: space.md,
    gap: space.sm
  },
  list: {
    paddingHorizontal: space.lg,
    paddingBottom: space.lg
  },
  empty: {
    paddingVertical: space.xxl,
    alignItems: 'center'
  },
  bubbleRow: {
    flexDirection: 'row',
    marginVertical: 4
  },
  bubble: {
    maxWidth: '85%',
    borderRadius: radius.lg,
    paddingHorizontal: space.md,
    paddingVertical: space.sm,
    borderWidth: 1
  },
  bubbleUser: {
    backgroundColor: colors.accent,
    borderColor: colors.accent,
    borderBottomRightRadius: radius.sm
  },
  bubbleAssistant: {
    backgroundColor: colors.surfaceElev,
    borderColor: colors.border,
    borderBottomLeftRadius: radius.sm
  },
  bubbleText: {
    fontSize: 15,
    lineHeight: 20
  },
  bubbleTextUser: { color: colors.accentText },
  bubbleTextAssistant: { color: colors.text },
  composer: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    padding: space.md,
    borderTopColor: colors.border,
    borderTopWidth: 1,
    backgroundColor: colors.surface,
    gap: space.sm
  },
  composerInput: {
    flex: 1,
    minHeight: 40,
    maxHeight: 140,
    paddingHorizontal: space.md,
    paddingVertical: space.sm,
    color: colors.text,
    backgroundColor: colors.surfaceElev,
    borderRadius: radius.md,
    borderColor: colors.border,
    borderWidth: 1,
    fontSize: 15
  },
  sendBtn: {
    paddingHorizontal: space.lg,
    paddingVertical: space.sm,
    borderRadius: radius.md,
    backgroundColor: colors.accent,
    minWidth: 72,
    alignItems: 'center',
    justifyContent: 'center'
  },
  sendBtnDisabled: { opacity: 0.4 },
  sendBtnPressed: { opacity: 0.8 },
  sendBtnLabel: {
    color: colors.accentText,
    fontWeight: '600',
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
  }
});
