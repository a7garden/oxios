//! Noise_XX session (server responder) + WS frame format (RFC-044 §6.3-6.4).
//!
//! E2EE core for the remote companion surface. The `Responder` drives the
//! 3-message XX handshake against an independent initiator (the companion
//! learns the server's static key on the wire — no out-of-band trust needed
//! beyond the QR pairing offer). On `into_transport()` both sides reach
//! transport mode and the symmetric AEAD (`encrypt`/`decrypt`) takes over.
//!
//! Public symbols are forward-declared for Task 7 (WS transport) and Task 9
//! (surface wiring); the `dead_code` allowance is intentional.
#![allow(dead_code)]

use anyhow::{Context, Result, anyhow};
use snow::HandshakeState;

/// Maximum payload size for a single frame. Frames larger than this are
/// rejected at both encode and decode time.
pub const FRAME_MAX: usize = 65536;

/// Frame header = 1 byte type + 4 byte big-endian payload length.
const FRAME_HEADER_LEN: usize = 5;
/// AEAD overhead appended by `snow` (ChaCha20-Poly1305).
const NOISE_TAG_LEN: usize = 16;

/// Frame type byte sent on the wire as the first byte of each frame.
///
/// `Noise` carries handshake bytes from the XX handshake; once transport
/// mode is reached the wire switches to AEAD-sealed `App` frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    Noise = 0x01,
    App = 0x02,
    Ping = 0x03,
    Pong = 0x04,
    Close = 0x05,
}

impl FrameType {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::Noise),
            0x02 => Some(Self::App),
            0x03 => Some(Self::Ping),
            0x04 => Some(Self::Pong),
            0x05 => Some(Self::Close),
            _ => None,
        }
    }
}

/// Encode a frame as `[type:1][size:4 BE][payload]`.
///
/// Returns `None` if the payload exceeds [`FRAME_MAX`] — oversized frames
/// are a transport-level DoS vector and MUST NOT be sent.
pub fn encode_frame(ty: FrameType, payload: &[u8]) -> Option<Vec<u8>> {
    if payload.len() > FRAME_MAX {
        return None;
    }
    let len = u32::try_from(payload.len()).ok()?;
    let mut out = Vec::with_capacity(FRAME_HEADER_LEN + payload.len());
    out.push(ty as u8);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(payload);
    Some(out)
}
/// Decode a frame buffer. Returns `(type, payload_slice)` borrowed from
/// `buf`, or `None` if the header is malformed, the type byte is unknown,
/// the declared size is invalid, or the total would exceed [`FRAME_MAX`].
pub fn decode_frame(buf: &[u8]) -> Option<(FrameType, &[u8])> {
    if buf.len() < FRAME_HEADER_LEN {
        return None;
    }
    let ty = FrameType::from_byte(buf[0])?;
    let size = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
    if size > FRAME_MAX {
        return None;
    }
    let end = FRAME_HEADER_LEN.checked_add(size)?;
    // Guard against wrapping and against callers passing slices that
    // claim more bytes than actually follow the header.
    if end > buf.len() {
        return None;
    }
    Some((ty, &buf[FRAME_HEADER_LEN..end]))
}
/// Server-side Noise_XX responder. Built from the daemon's static secret
/// ([`crate::remote::identity::DeviceIdentity::snow_static`]). Wraps a
/// [`snow::HandshakeState`] until [`Responder::into_transport`] consumes
/// it and yields an AEAD [`Transport`].
pub struct Responder {
    hs: HandshakeState,
    done: bool,
}

impl Responder {
    /// Build a responder from the server's 32-byte Noise static secret.
    pub fn new(server_static: &[u8]) -> Result<Self> {
        let params: snow::params::NoiseParams = "Noise_XX_25519_ChaChaPoly_SHA256"
            .parse()
            .context("parse Noise_XX params")?;
        let hs = snow::Builder::new(params)
            .local_private_key(server_static)
            .context("set server static key")?
            .build_responder()
            .context("build responder")?;
        Ok(Self { hs, done: false })
    }

    /// True once both sides have completed the 3-message XX handshake.
    pub fn is_handshake_finished(&self) -> bool {
        self.done
    }

    /// Process the next incoming handshake message from the initiator and
    /// produce the next outgoing handshake message (if any).
    ///
    /// Returns:
    /// - `Ok(Some(msg))` when the responder wrote a handshake payload
    ///   (msg2 in the XX flow),
    /// - `Ok(None)` once the handshake is complete (after msg3) — caller
    ///   should then call [`Responder::into_transport`].
    ///
    /// Calling `handshake_msg` after the handshake is complete is a
    /// programming error and returns `Err`.
    pub fn handshake_msg(&mut self, theirs: &[u8]) -> Result<Option<Vec<u8>>> {
        if self.done {
            return Err(anyhow!("handshake already finished"));
        }
        // Validate incoming size before handing to snow — XX payload caps
        // are bounded by the pattern but a 1 MiB buffer should not reach
        // the cipher state.
        if theirs.len() > FRAME_MAX {
            return Err(anyhow!("handshake message exceeds FRAME_MAX"));
        }
        // Read the initiator's payload (discarded — XX carries no app
        // data in this surface).
        let mut payload = [0u8; FRAME_MAX];
        let _n = self
            .hs
            .read_message(theirs, &mut payload)
            .map_err(|e| anyhow!("read_message: {e}"))?;

        if self.hs.is_handshake_finished() {
            self.done = true;
            return Ok(None);
        }

        // Write our outgoing handshake message (msg2 in the XX flow).
        let mut out = vec![0u8; FRAME_MAX];
        let written = self
            .hs
            .write_message(&[], &mut out)
            .map_err(|e| anyhow!("write_message: {e}"))?;
        out.truncate(written);

        if self.hs.is_handshake_finished() {
            self.done = true;
        }
        Ok(Some(out))
    }

    /// Consume the responder and finalize the handshake, yielding an AEAD
    /// [`Transport`].
    pub fn into_transport(self) -> Result<Transport> {
        if !self.done {
            return Err(anyhow!("handshake not finished"));
        }
        let ts = self
            .hs
            .into_transport_mode()
            .map_err(|e| anyhow!("into_transport_mode: {e}"))?;
        Ok(Transport { ts })
    }
}

/// Post-handshake AEAD transport. Each `encrypt`/`decrypt` call advances
/// the nonce counter; snow enforces the 2^64 nonce bound.
pub struct Transport {
    ts: snow::TransportState,
}

impl Transport {
    /// Seal a plaintext frame. The returned ciphertext includes the 16-byte
    /// AEAD tag; the caller is responsible for framing it on the wire
    /// ([`encode_frame`] with [`FrameType::App`] / `Ping` / `Pong` / `Close`).
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        if plaintext.len() > FRAME_MAX {
            return Err(anyhow!("plaintext exceeds FRAME_MAX"));
        }
        let mut buf = vec![0u8; plaintext.len() + NOISE_TAG_LEN];
        let n = self
            .ts
            .write_message(plaintext, &mut buf)
            .map_err(|e| anyhow!("encrypt: {e}"))?;
        buf.truncate(n);
        Ok(buf)
    }

    /// Open a sealed ciphertext frame produced by the peer.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() > FRAME_MAX + NOISE_TAG_LEN {
            return Err(anyhow!("ciphertext exceeds FRAME_MAX + tag"));
        }
        // Output buffer matches the max plaintext a legal frame can carry.
        let mut out = vec![0u8; FRAME_MAX + NOISE_TAG_LEN];
        let n = self
            .ts
            .read_message(ciphertext, &mut out)
            .map_err(|e| anyhow!("decrypt: {e}"))?;
        out.truncate(n);
        Ok(out)
    }

    /// True if this transport is the initiator side (false for a responder).
    /// Useful for tests and diagnostics; the wire protocol does not require it.
    pub fn is_initiator(&self) -> bool {
        self.ts.is_initiator()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Standard Noise pattern name used everywhere in this surface.
    const NOISE_XX: &str = "Noise_XX_25519_ChaChaPoly_SHA256";

    #[test]
    fn frame_roundtrip() {
        let encoded = encode_frame(FrameType::App, b"hello").expect("encode ok");
        assert_eq!(encoded.len(), FRAME_HEADER_LEN + 5);
        assert_eq!(encoded[0], FrameType::App as u8);
        let (ty, payload) = decode_frame(&encoded).expect("decode ok");
        assert_eq!(ty, FrameType::App);
        assert_eq!(payload, b"hello");
    }

    #[test]
    fn frame_max_rejects_oversized() {
        let oversized = vec![0xAA_u8; FRAME_MAX + 1];
        // encode_frame MUST refuse payloads beyond FRAME_MAX — silent
        // truncation would be a wire-format bug.
        assert!(
            encode_frame(FrameType::App, &oversized).is_none(),
            "encode_frame must reject oversized payloads"
        );
        // decode_frame MUST refuse headers that claim more than FRAME_MAX.
        let header = {
            let mut h = [0u8; FRAME_HEADER_LEN];
            h[0] = FrameType::App as u8;
            let len = (FRAME_MAX as u32 + 1).to_be_bytes();
            h[1..].copy_from_slice(&len);
            h.to_vec()
        };
        assert!(
            decode_frame(&header).is_none(),
            "decode_frame must reject oversized headers"
        );
        // decode_frame MUST also refuse truncated frames.
        let truncated = encode_frame(FrameType::App, b"x").expect("encode ok");
        assert!(decode_frame(&truncated[..FRAME_HEADER_LEN]).is_none());
    }

    #[test]
    fn noise_xx_handshake_and_transport() {
        // Server static keypair — the responder is built from `private`.
        let server_kp = snow::Builder::new(NOISE_XX.parse().unwrap())
            .generate_keypair()
            .expect("generate server keypair");
        // Independent initiator with NO prior knowledge of the server key
        // (XX authenticates the server on the wire). The peer side here
        // is a raw snow transport — proving our responder interoperates
        // with any compliant Noise_XX peer. XX needs BOTH sides to hold
        // a static key — the initiator's goes in msg3.
        let client_kp = snow::Builder::new(NOISE_XX.parse().unwrap())
            .generate_keypair()
            .expect("generate client keypair");
        let mut initiator = snow::Builder::new(NOISE_XX.parse().unwrap())
            .local_private_key(&client_kp.private)
            .expect("set client static")
            .build_initiator()
            .expect("build initiator");
        let mut responder = Responder::new(&server_kp.private).expect("build responder");

        // msg1: -> e, es  (initiator writes)
        let mut msg1 = vec![0u8; 1024];
        let n = initiator
            .write_message(&[], &mut msg1)
            .expect("init write msg1");
        msg1.truncate(n);
        // Responder processes msg1 and writes msg2.
        let msg2 = responder
            .handshake_msg(&msg1)
            .expect("responder handshake_msg msg1")
            .expect("responder must produce msg2");
        // Initiator reads msg2: <- e, ee, s, es
        let mut buf = [0u8; 1024];
        initiator
            .read_message(&msg2, &mut buf)
            .expect("init read msg2");
        // msg3: -> s, se  (initiator writes)
        let mut msg3 = vec![0u8; 1024];
        let n = initiator
            .write_message(&[], &mut msg3)
            .expect("init write msg3");
        msg3.truncate(n);
        // Responder processes msg3 — produces NO outgoing message; XX is done.
        assert!(
            responder
                .handshake_msg(&msg3)
                .expect("responder handshake_msg msg3")
                .is_none(),
            "XX has 3 messages — the responder writes none after msg3"
        );
        assert!(responder.is_handshake_finished());

        let mut server_t = responder.into_transport().expect("responder -> transport");
        let mut client_t = initiator.into_transport_mode().expect("init -> transport");
        assert!(!server_t.is_initiator());
        assert!(client_t.is_initiator());

        // Round-trip a plaintext across the AEAD transport. The peer
        // uses raw snow write_message/read_message; our server_t uses
        // the `Transport` wrapper. Both directions prove the session
        // keys match.
        let plaintext = b"e2ee ping";
        let mut sealed = vec![0u8; plaintext.len() + NOISE_TAG_LEN];
        let n = client_t
            .write_message(plaintext, &mut sealed)
            .expect("client write_message");
        sealed.truncate(n);
        let opened = server_t.decrypt(&sealed).expect("server decrypt");
        assert_eq!(opened, plaintext);

        let reply = b"e2ee pong";
        let sealed_reply = server_t.encrypt(reply).expect("server encrypt");
        let mut opened_reply = vec![0u8; sealed_reply.len()];
        let n = client_t
            .read_message(&sealed_reply, &mut opened_reply)
            .expect("client read_message");
        opened_reply.truncate(n);
        assert_eq!(opened_reply, reply);
    }
}
