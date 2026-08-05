# oxios-companion

The native companion app for Oxios. Built with React Native + Expo
(Expo Router, file-based nav). Speaks Noise_XX_ChaChaPoly to the daemon
directly — no relay, no cloud hop.

## Stack

- **Runtime:** React Native 0.81, Expo ~54, Expo Router ~6
- **Crypto:** `@noble/ciphers` (ChaCha20-Poly1305) + `@noble/curves` (X25519)
  + `@noble/hashes` (SHA-256, HKDF) for a pure-JS Noise_XX implementation.
  Falls back to `tweetnacl` only where `@noble` does not cover the primitive.
- **Persistence:** `expo-secure-store` for the device token and host
  profile. Token is **split** from the profile so the public-key-only
  record can live in the same row without leaking secrets in exportable
  storage.
- **Security scope:** cleartext traffic allowed to `192.168.x` and
  `100.x` (`NSAllowsLocalNetworking`); no analytics, no telemetry.

## File layout (RFC-044 §7.1)

```
companion/
├── app/                       # Expo Router screens
│   ├── _layout.tsx            # root Stack, dark monochrome
│   ├── index.tsx              # paired hosts list
│   ├── pair-scan.tsx          # QR scanner (expo-camera)
│   ├── pair-confirm.tsx       # offer details + Noise handshake
│   ├── h/[hostId].tsx         # sessions + persona selector
│   └── session/[id].tsx       # chat send/subscribe + agent status
└── src/
    ├── types.ts               # PairingOffer, HostProfile, Rpc shapes
    ├── pairing/decode-offer.ts  # pure base64url+JSON offer decoder
    ├── services/api.ts        # the ONLY surface the screens import
    └── ui/                    # theme + monochrome primitives
```

The transport layer (port from `orca/mobile/src/transport/`, minus the
relay path) lives at `companion/src/transport/*` and
`companion/src/keychain/host-store.ts`. It exports:

- `getStableClient(hostId)` → `ClientHandle` (send / subscribe / getState / onStateChange / close)
- `hosts.list / save / saveDeviceToken / remove`
- `pairing.completeHandshake(offer) → { deviceToken, deviceId }`

The screens never import from `src/transport/*` directly — they go
through `src/services/api.ts`. That keeps the UI provably independent
of the wire/Nose plumbing.

## Wire (RFC-044 §6.3)

```
Frame: [type:1][size:4 BE][payload]      max 65536 bytes
Type:  Noise=0x01 | App=0x02 | Ping=0x03 | Pong=0x04 | Close=0x05
```

Noise_XX_25519_ChaChaPoly_SHA256 (initiator = companion, responder =
daemon). App frames carry JSON-RPC 2.0 envelopes.

## Direct endpoint race

Each pairing offer may carry multiple `direct_endpoint` entries (LAN +
Tailscale). The transport opens one WebSocket per direct URL in parallel;
the first Noise-authenticated wins, the losers get closed. Hysteresis:
3 successes / 30s observation, 60s min dwell, 60s failure cooldown.
Backoff is full-jitter 250 ms–30 s. There is no relay path — if both
direct paths fail, we surface the "Can't connect — check Tailscale" hint
when the unreachable endpoint looks Tailscale-shaped.

## Run

```sh
pnpm install
pnpm start          # Expo dev server
pnpm ios            # iOS simulator
pnpm android        # Android emulator
```

Camera permission is required for `pair-scan`. On iOS the bundle id is
`com.oxios.companion`; on Android the package is the same. The pairing
URL scheme is `oxios://pair?code=<base64url json>`.
