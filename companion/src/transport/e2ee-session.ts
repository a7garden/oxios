import { chacha20poly1305 } from '@noble/ciphers/chacha'
import { x25519 } from '@noble/curves/ed25519'
import { sha256 } from '@noble/hashes/sha256'
import { hmac } from '@noble/hashes/hmac'
import { randomBytes } from '@noble/hashes/utils'

type Bytes = Uint8Array

const PROTOCOL = new TextEncoder().encode('Noise_XX_25519_ChaChaPoly_SHA256')
const EMPTY: Bytes = new Uint8Array()

const concat = (...xs: Bytes[]): Bytes => {
  const out = new Uint8Array(xs.reduce((n, x) => n + x.length, 0))
  let p = 0
  for (const x of xs) {
    out.set(x, p)
    p += x.length
  }
  return out
}
const same = (a: Bytes, b: Bytes): boolean =>
  a.length === b.length && a.reduce((v, x, i) => v | (x ^ b[i]!), 0) === 0
function hkdf(ck: Bytes, input: Bytes, count: number): Bytes[] {
  const temp = hmac(sha256, ck, input)
  const out: Bytes[] = []
  let prev: Bytes = EMPTY
  for (let i = 1; i <= count; i++) {
    prev = hmac(sha256, temp, concat(prev, new Uint8Array([i])))
    out.push(prev)
  }
  return out
}
function nonce(n: bigint): Bytes {
  const out = new Uint8Array(12)
  const view = new DataView(out.buffer)
  view.setUint32(4, Number(n & 0xffffffffn), true)
  view.setUint32(8, Number(n >> 32n), true)
  return out
}

function initializeSymmetricState() {
  const padded = PROTOCOL.length <= 32 ? concat(PROTOCOL, new Uint8Array(32 - PROTOCOL.length)) : sha256(PROTOCOL)
  return { ck: padded.slice(), h: padded.slice() }
}

/**
 * Noise_XX_25519_ChaChaPoly_SHA256 initiator compatible with `snow`. Implements
 * the 3-message XX handshake as initiator plus a 2-key Split() transport for
 * AEAD-encrypted App frames (`[type:1][size:4BE][payload]`, max 65536).
 */
export class NoiseXXInitiator {
  private ck: Bytes
  private h: Bytes
  private k: Bytes | null = null
  private n = 0n
  private e: Bytes | null = null
  private re: Bytes | null = null
  private rs: Bytes | null = null
  private sendKey: Bytes | null = null
  private receiveKey: Bytes | null = null
  private sendNonce = 0n
  private receiveNonce = 0n
  private stage: 0 | 1 | 2 | 3 = 0

  constructor(
    private readonly staticKey: Bytes = randomBytes(32),
    private readonly expectedRemoteStatic?: Bytes
  ) {
    const init = initializeSymmetricState()
    this.ck = init.ck
    this.h = init.h
  }

  get staticPublicKey(): Bytes {
    return x25519.getPublicKey(this.staticKey)
  }
  get remoteStaticPublicKey(): Bytes | null {
    return this.rs?.slice() ?? null
  }
  get complete(): boolean {
    return this.stage === 3
  }

  writeMessage1(): Bytes {
    if (this.stage !== 0) throw new Error('invalid Noise handshake stage')
    const ephemeral = randomBytes(32)
    this.e = ephemeral
    const ephemeralPublic = x25519.getPublicKey(ephemeral)
    this.mixHash(ephemeralPublic)
    // Noise_XX -> e, payload is empty; EncryptAndHash still advances h even
    // when the payload is empty (MixHash of nothing). Snow expects this.
    this.mixHash(EMPTY)
    this.stage = 1
    return ephemeralPublic
  }

  readMessage2(message: Bytes): void {
    if (this.stage !== 1 || !this.e || message.length !== 96) {
      throw new Error('invalid Noise XX message 2')
    }
    const ephemeral = this.e
    const re = message.slice(0, 32)
    this.re = re
    this.mixHash(re)
    this.mixKey(x25519.getSharedSecret(ephemeral, re))
    const rs = this.decryptAndHash(message.slice(32, 80))
    if (rs.length !== 32) throw new Error('invalid responder static key')
    if (this.expectedRemoteStatic && !same(rs, this.expectedRemoteStatic)) {
      throw new Error('responder static key mismatch')
    }
    this.rs = rs
    this.mixKey(x25519.getSharedSecret(ephemeral, rs))
    if (this.decryptAndHash(message.slice(80)).length) {
      throw new Error('unexpected Noise handshake payload')
    }
    this.stage = 2
  }

  writeMessage3(): Bytes {
    if (this.stage !== 2 || !this.re) throw new Error('invalid Noise handshake stage')
    const re = this.re
    const encryptedStatic = this.encryptAndHash(this.staticPublicKey)
    this.mixKey(x25519.getSharedSecret(this.staticKey, re))
    const encryptedPayload = this.encryptAndHash(EMPTY)
    const split = hkdf(this.ck, EMPTY, 2)
    this.sendKey = split[0]!
    this.receiveKey = split[1]!
    this.k = null
    this.stage = 3
    return concat(encryptedStatic, encryptedPayload)
  }

  encrypt(plaintext: Bytes): Bytes {
    if (!this.sendKey) throw new Error('Noise handshake incomplete')
    return chacha20poly1305(this.sendKey, nonce(this.sendNonce++)).encrypt(plaintext)
  }

  decrypt(ciphertext: Bytes): Bytes {
    if (!this.receiveKey) throw new Error('Noise handshake incomplete')
    return chacha20poly1305(this.receiveKey, nonce(this.receiveNonce++)).decrypt(ciphertext)
  }

  private mixHash(data: Bytes): void {
    this.h = sha256(concat(this.h, data))
  }
  private mixKey(input: Bytes): void {
    const parts = hkdf(this.ck, input, 2)
    this.ck = parts[0]!
    this.k = parts[1]!
    this.n = 0n
  }
  private encryptAndHash(plain: Bytes): Bytes {
    const sealed = this.k
      ? chacha20poly1305(this.k, nonce(this.n++), this.h).encrypt(plain)
      : plain
    this.mixHash(sealed)
    return sealed
  }
  private decryptAndHash(cipher: Bytes): Bytes {
    const opened = this.k
      ? chacha20poly1305(this.k, nonce(this.n++), this.h).decrypt(cipher)
      : cipher
    this.mixHash(cipher)
    return opened
  }
}
