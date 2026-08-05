/**
 * Pure decoder for `oxios://pair?code=<base64url-json>` URLs.
 * Lives in its own file because the QR scan screen needs to validate
 * a scanned string before the transport module is online — no noise,
 * no socket, just base64url + JSON.parse under tight validation.
 */

import type { DirectEndpoint, PairingOffer } from '@/types';

const BASE64URL_RE = /^[A-Za-z0-9_-]+={0,2}$/;

function base64UrlDecode(input: string): Uint8Array {
  if (!BASE64URL_RE.test(input)) {
    throw new Error('pair-offer: invalid base64url payload');
  }
  // Convert URL-safe -> standard base64, restore padding.
  const std = input.replace(/-/g, '+').replace(/_/g, '/');
  const padded = std + '='.repeat((4 - (std.length % 4)) % 4);

  // React Native exposes a global atob that accepts base64 strings.
  // node-buffer would be ~70 KB, this stays free.
  const binary =
    typeof globalThis.atob === 'function'
      ? globalThis.atob(padded)
      : Buffer.from(padded, 'base64').toString('binary');

  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

function isEndpointShape(value: unknown): value is DirectEndpoint {
  if (!value || typeof value !== 'object') return false;
  if (!('url' in value)) return false;
  if (!('kind' in value)) return false;
  const url = (value as { url: unknown }).url;
  const kind = (value as { kind: unknown }).kind;
  return typeof url === 'string' && (kind === 'lan' || kind === 'tailscale');
}

export function decodePairOfferUrl(url: string): PairingOffer {
  const prefix = 'oxios://pair?code=';
  if (!url.startsWith(prefix)) {
    throw new Error(`pair-offer: expected ${prefix}…`);
  }
  const code = url.slice(prefix.length).split('&')[0];
  if (!code) {
    throw new Error('pair-offer: missing code param');
  }
  const json = new TextDecoder('utf-8').decode(base64UrlDecode(code));
  const parsed: unknown = JSON.parse(json);

  if (!parsed || typeof parsed !== 'object') {
    throw new Error('pair-offer: payload is not an object');
  }
  const obj = parsed as Record<string, unknown>;
  const version = obj.v;
  const endpoint = obj.endpoint;
  const deviceId = obj.device_id;
  const publicKey = obj.public_key_b64;

  if (typeof version !== 'number' || version < 1) {
    throw new Error(`pair-offer: unsupported version ${String(version)}`);
  }
  if (typeof endpoint !== 'string' || endpoint.length === 0) {
    throw new Error('pair-offer: missing endpoint');
  }
  if (typeof deviceId !== 'string' || deviceId.length === 0) {
    throw new Error('pair-offer: missing device_id');
  }
  if (typeof publicKey !== 'string' || publicKey.length === 0) {
    throw new Error('pair-offer: missing public_key_b64');
  }

  const offer: PairingOffer = {
    v: version,
    endpoint,
    device_id: deviceId,
    public_key_b64: publicKey
  };

  if (Array.isArray(obj.endpoints)) {
    const out: PairingOffer['endpoints'] = [];
    for (const entry of obj.endpoints) {
      if (typeof entry === 'string') {
        out.push(entry);
      } else if (isEndpointShape(entry)) {
        out.push(entry);
      }
    }
    if (out.length > 0) offer.endpoints = out;
  }

  if (typeof obj.scope === 'string' || Array.isArray(obj.scope)) {
    offer.scope = obj.scope as string | string[];
  }

  return offer;
}

/** Generate a host id from the daemon's published device_id. Stable across re-scans. */
export function hostIdForOffer(offer: PairingOffer): string {
  return `host-${offer.device_id.slice(0, 12)}`;
}
