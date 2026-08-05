/**
 * QR scanner for `oxios://pair?code=...` URLs. Permission flow, live
 * preview, scan/throttle, navigate to pair-confirm with the offer.
 */

import { useCallback, useRef, useState } from 'react';
import {
  Linking,
  StyleSheet,
  Text,
  View,
  Pressable
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import {
  CameraView,
  useCameraPermissions
} from 'expo-camera';
import { useRouter } from 'expo-router';

import {
  decodePairOfferUrl,
  hostIdForOffer
} from '@/pairing/decode-offer';
import type { HostProfile, PairingOffer } from '@/types';
import { hosts as hostsApi } from '@/services/api';
import { Button, Header } from '@/ui/primitives';
import { colors, font, radius, space, text as t } from '@/ui/theme';

export default function PairScanScreen() {
  const router = useRouter();
  const [permission, requestPermission] = useCameraPermissions();
  const [scanned, setScanned] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const throttleRef = useRef(0);

  const handleScan = useCallback(
    async (event: { data?: string }) => {
      const data = event.data;
      if (!data || scanned) return;
      const now = Date.now();
      if (now - throttleRef.current < 800) return;
      throttleRef.current = now;
      setScanned(true);
      setError(null);
      try {
        // Validate the payload up-front so a bad QR never reaches
        // the confirm screen or the host-store adapter.
        const offer = decodePairOfferUrl(data);
        await persistHostProfile(offer);
        // Round-trip the full URL so confirm has the canonical
        // encoded payload available for the handshake.
        router.push({ pathname: '/pair-confirm', params: { offer: data } });
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        setTimeout(() => setScanned(false), 1500);
      }
    },
    [router, scanned]
  );

  if (!permission) {
    return (
      <SafeAreaView style={styles.screen}>
        <Header title="Scan QR" subtitle="Requesting camera permission…" />
      </SafeAreaView>
    );
  }

  if (!permission.granted) {
    return (
      <SafeAreaView style={styles.screen}>
        <Header
          title="Scan QR"
          subtitle="Camera permission is required to scan a pairing code."
        />
        <View style={styles.permissionWrap}>
          <Text style={styles.permissionBody}>
            The Oxios companion scans a QR code from your desktop app to
            establish a Noise_XX end-to-end encrypted session. Camera access
            is used only for the QR scan itself — no frames leave this
            device.
          </Text>
          {permission.canAskAgain ? (
            <Button
              label="Allow camera"
              onPress={() => {
                void requestPermission();
              }}
            />
          ) : (
            <Button
              label="Open settings"
              onPress={() => {
                void Linking.openSettings();
              }}
              variant="secondary"
            />
          )}
        </View>
      </SafeAreaView>
    );
  }

  return (
    <View style={styles.screen}>
      <CameraView
        style={StyleSheet.absoluteFill}
        facing="back"
        barcodeScannerSettings={{
          barcodeTypes: ['qr']
        }}
        onBarcodeScanned={handleScan}
      />

      <SafeAreaView style={styles.overlay} edges={['top', 'bottom']}>
        <View style={styles.topBar}>
          <Header
            title="Scan pairing QR"
            subtitle="Point the camera at the Oxios desktop code"
            right={
              <Pressable
                onPress={() => router.back()}
                accessibilityLabel="Cancel"
                style={styles.closeBtn}
              >
                <Text style={styles.closeBtnLabel}>Cancel</Text>
              </Pressable>
            }
          />
        </View>

        <View style={styles.reticleWrap} pointerEvents="none">
          <View style={styles.reticle} />
        </View>

        <View style={styles.bottomBar}>
          {error ? (
            <Text style={styles.errorLabel}>{error}</Text>
          ) : (
            <Text style={styles.helperLabel}>
              {scanned ? 'Decoded. Opening confirm…' : 'Waiting for QR'}
            </Text>
          )}
        </View>
      </SafeAreaView>
    </View>
  );
}

async function persistHostProfile(offer: PairingOffer) {
  // The transport layer owns the SecureStore shape; screens just hand
  // the validated offer to its adapter. Persist a "tentative" profile
  // with no device token so the row exists and the confirm screen
  // can update it after the handshake completes.
  const profile: HostProfile = {
    id: hostIdForOffer(offer),
    name: `Host ${offer.device_id.slice(0, 8)}`,
    endpoint: offer.endpoint,
    deviceId: offer.device_id,
    publicKeyB64: offer.public_key_b64,
    lastConnected: 0,
    endpoints: parseEndpoints(offer)
  };
  try {
    await hostsApi.save(profile);
  } catch {
    // Surface the failure to the confirm screen if needed; for now
    // we tolerate the transport being offline.
  }
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

const styles = StyleSheet.create({
  screen: {
    flex: 1,
    backgroundColor: colors.bg
  },
  overlay: {
    flex: 1,
    backgroundColor: colors.overlay,
    justifyContent: 'space-between'
  },
  topBar: {
    backgroundColor: 'rgba(0,0,0,0.6)'
  },
  bottomBar: {
    backgroundColor: 'rgba(0,0,0,0.6)',
    padding: space.lg,
    alignItems: 'center'
  },
  reticleWrap: {
    alignItems: 'center',
    justifyContent: 'center'
  },
  reticle: {
    width: 240,
    height: 240,
    borderRadius: radius.lg,
    borderColor: colors.text,
    borderWidth: 2,
    borderStyle: 'dashed'
  },
  helperLabel: {
    color: colors.textMuted,
    fontSize: 13,
    fontFamily: font.mono
  },
  errorLabel: {
    color: colors.danger,
    fontSize: 13,
    fontFamily: font.mono,
    textAlign: 'center'
  },
  closeBtn: {
    paddingHorizontal: space.md,
    paddingVertical: space.sm,
    borderColor: colors.borderStrong,
    borderWidth: 1,
    borderRadius: radius.md
  },
  closeBtnLabel: {
    color: colors.text,
    fontSize: 14,
    fontWeight: '600'
  },
  permissionWrap: {
    flex: 1,
    paddingHorizontal: space.xl,
    paddingTop: space.xxl,
    gap: space.lg
  },
  permissionBody: {
    ...t.bodyMuted,
    lineHeight: 20
  }
});
