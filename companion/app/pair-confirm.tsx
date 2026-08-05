/**
 * Pair-confirm screen. Displays the decoded offer (endpoint, device id,
 * fingerprint) and runs the Noise_XX handshake on tap. On success it
 * persists the device token via `hosts.saveDeviceToken` and routes into
 * the host detail screen.
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { ScrollView, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { useLocalSearchParams, useRouter, type Href } from 'expo-router';

import {
  hosts as hostsApi,
  isTransportOffline,
  pairing as pairingApi
} from '@/services/api';
import type { HostProfile, PairingOffer } from '@/types';
import { decodePairOfferUrl, hostIdForOffer } from '@/pairing/decode-offer';
import {
  Badge,
  Button,
  Card,
  Header,
  Row
} from '@/ui/primitives';
import { colors, font, space, text as t } from '@/ui/theme';

type Phase =
  | 'idle'
  | 'awaiting'
  | 'handshaking'
  | 'storing'
  | 'done'
  | 'error';

export default function PairConfirmScreen() {
  const router = useRouter();
  const params = useLocalSearchParams<{ offer?: string }>();

  // The scan screen passes a canonical `oxios://pair?code=…` URL; we
  // decode + validate it here so the confirm view always has a typed offer.
  const offer = useMemo<PairingOffer | null>(() => {
    if (typeof params.offer !== 'string') return null;
    try {
      return decodePairOfferUrl(params.offer);
    } catch {
      return null;
    }
  }, [params.offer]);

  const hostId = offer ? hostIdForOffer(offer) : null;

  const [phase, setPhase] = useState<Phase>(offer ? 'awaiting' : 'idle');
  const [error, setError] = useState<string | null>(null);
  const [fpHex, setFpHex] = useState<string | null>(null);

  useEffect(() => {
    if (!offer) return;
    // Best-effort fingerprint preview — first 16 hex chars of the
    // remote X25519 static key. Pure formatting; the public key is
    // opaque so we only show a prefix as the "fingerprint".
    try {
      const bytes = decodePublicKeyB64Url(offer.public_key_b64);
      let hex = '';
      const head = bytes.slice(0, 8);
      for (let i = 0; i < head.length; i += 1) {
        hex += head[i].toString(16).padStart(2, '0');
      }
      setFpHex(hex);
    } catch {
      setFpHex(null);
    }
  }, [offer]);

  const onConfirm = useCallback(async () => {
    if (!offer || !hostId) return;
    setPhase('handshaking');
    setError(null);
    try {
      const { deviceToken, deviceId } =
        await pairingApi.completeHandshake(offer);
      setPhase('storing');
      await hostsApi.saveDeviceToken(hostId, deviceToken);
      const profile: HostProfile = {
        id: hostId,
        name: `Host ${deviceId.slice(0, 8)}`,
        endpoint: offer.endpoint,
        deviceId,
        deviceToken,
        publicKeyB64: offer.public_key_b64,
        lastConnected: Date.now(),
        endpoints: parseEndpoints(offer)
      };
      await hostsApi.save(profile);
      setPhase('done');
      const next: Href = {
        pathname: '/h/[hostId]',
        params: { hostId }
      };
      router.replace(next);
    } catch (err) {
      setPhase('error');
      if (isTransportOffline(err)) {
        setError('Transport bridge offline — wire runtime adapters first.');
      } else {
        setError(err instanceof Error ? err.message : String(err));
      }
    }
  }, [offer, hostId, router]);

  if (!offer || !hostId) {
    return (
      <SafeAreaView style={styles.screen}>
        <Header title="Confirm pairing" />
        <View style={styles.fallback}>
          <Text style={t.bodyMuted}>
            This screen expects a scanned offer. Open the scanner from the
            hosts list to pair a new desktop.
          </Text>
          <View style={{ marginTop: space.lg }}>
            <Button
              label="Open scanner"
              onPress={() => router.replace('/pair-scan')}
            />
          </View>
        </View>
      </SafeAreaView>
    );
  }

  const showCancel = router.canGoBack();

  return (
    <SafeAreaView style={styles.screen} edges={['top']}>
      <Header
        title="Confirm pairing"
        subtitle="Verify the fingerprint before continuing"
      />
      <ScrollView contentContainerStyle={styles.scroll}>
        <Card>
          <Text style={[t.title, { marginBottom: space.sm }]}>
            Host details
          </Text>
          <Row label="Endpoint" value={offer.endpoint} mono />
          <Row label="Device ID" value={offer.device_id} mono />
          {fpHex ? (
            <Row label="Noise pubkey" value={`${fpHex}…`} mono />
          ) : null}
          <View style={styles.badgeRow}>
            <Badge label="Noise_XX" tone="neutral" />
            <Badge label="ChaChaPoly" tone="neutral" />
            <Badge label="SHA256" tone="neutral" />
          </View>
        </Card>

        <Card>
          <Text style={[t.bodyMuted, styles.helperText]}>
            Connecting establishes a Noise_XX_25519_ChaChaPoly_SHA256
            handshake. On success the device token is stored in SecureStore
            and you can skip this step next time.
          </Text>
        </Card>

        {error ? (
          <View style={styles.errorBanner}>
            <Text style={styles.errorText}>{error}</Text>
          </View>
        ) : null}

        <View style={styles.actions}>
          <Button
            label={
              phase === 'handshaking'
                ? 'Handshaking…'
                : phase === 'storing'
                  ? 'Saving…'
                  : 'Confirm and connect'
            }
            onPress={onConfirm}
            loading={phase === 'handshaking' || phase === 'storing'}
            disabled={phase === 'done'}
          />
          {showCancel ? (
            <Button
              label="Cancel"
              onPress={() => router.replace('/')}
              variant="ghost"
            />
          ) : null}
        </View>
      </ScrollView>
    </SafeAreaView>
  );
}

function parseEndpoints(
  offer: PairingOffer
): HostProfile['endpoints'] | undefined {
  const raw = offer.endpoints;
  if (!raw || raw.length === 0) return undefined;
  return raw.map((e) =>
    typeof e === 'string' ? { url: e, kind: 'lan' as const } : e
  );
}

const BASE64URL_RE = /^[A-Za-z0-9_-]+={0,2}$/;

/**
 * Decode a base64url-encoded X25519 public key into raw bytes.
 * Lives next to the pairing offer because the same scheme appears
 * at two different layers: the QR payload and any in-band rekey.
 */
function decodePublicKeyB64Url(input: string): Uint8Array {
  if (!BASE64URL_RE.test(input)) {
    throw new Error('invalid base64url public key');
  }
  const std = input.replace(/-/g, '+').replace(/_/g, '/');
  const padded = std + '='.repeat((4 - (std.length % 4)) % 4);
  const binary =
    typeof globalThis.atob === 'function'
      ? globalThis.atob(padded)
      : Buffer.from(padded, 'base64').toString('binary');
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    out[i] = binary.charCodeAt(i);
  }
  return out;
}

const styles = StyleSheet.create({
  screen: {
    flex: 1,
    backgroundColor: colors.bg
  },
  scroll: {
    paddingBottom: space.xxl
  },
  fallback: {
    paddingHorizontal: space.xl,
    paddingTop: space.xxl
  },
  helperText: {
    lineHeight: 20
  },
  badgeRow: {
    flexDirection: 'row',
    gap: space.sm,
    marginTop: space.md
  },
  actions: {
    marginHorizontal: space.lg,
    marginTop: space.md,
    gap: space.sm
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
