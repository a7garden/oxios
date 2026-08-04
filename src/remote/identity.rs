//! Persistent Noise static keypair + device-id fingerprint (RFC-044 §6.1).
//!
//! Public API consumed by pairing (Task 4) and surface wiring (Task 9).
//! Symbols are intentionally forward-declared; `#[allow(dead_code)]` silences
//! the warnings until those callers land.
#![allow(dead_code)]

use anyhow::{Context, Result};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::path::Path;

const IDENTITY_FILE: &str = "remote-identity.json";

/// The daemon's persistent Noise_XX static identity.
pub struct DeviceIdentity {
    /// 32-byte X25519 static keypair (Noise `25519` DH).
    pub keypair: snow::Keypair,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct IdentityFile {
    public: Vec<u8>,
    /// snow::Keypair exposes this as `private`; we persist as `secret` for
    /// human-readable debug dumps (no behavioural difference).
    secret: Vec<u8>,
}

impl DeviceIdentity {
    /// Load the keypair from `<state_dir>/remote-identity.json`, or generate
    /// and persist a new one (mode 0600) if absent.
    pub fn load_or_create(state_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(state_dir).ok();
        let path = state_dir.join(IDENTITY_FILE);
        if let Ok(bytes) = std::fs::read(&path) {
            let f: IdentityFile = serde_json::from_slice(&bytes).context("bad identity file")?;
            return Ok(Self {
                keypair: snow::Keypair {
                    public: f.public,
                    private: f.secret,
                },
            });
        }
        let builder = snow::Builder::new(
            "Noise_XX_25519_ChaChaPoly_SHA256"
                .parse()
                .expect("valid cipher suite"),
        );
        let keypair = builder.generate_keypair().expect("generate keypair");
        let f = IdentityFile {
            public: keypair.public.clone(),
            secret: keypair.private.clone(),
        };
        let json = serde_json::to_vec(&f)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;

            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?;
            std::io::Write::write_all(&mut file, &json)?;
            file.sync_all()?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&path, &json)?;
        }
        Ok(Self { keypair })
    }

    /// 16-byte hex fingerprint of the static public key.
    pub fn device_id(&self) -> String {
        let mut h = Sha256::new();
        h.update(&self.keypair.public);
        hex::encode(&h.finalize()[..16])
    }

    /// base64 (url-safe, no pad) of the static public key — goes in the QR offer.
    pub fn public_key_b64(&self) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&self.keypair.public)
    }

    /// Raw static key bytes for building a Noise responder.
    pub fn snow_static(&self) -> &[u8] {
        &self.keypair.private
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    #[test]
    fn load_or_create_is_stable_across_reload() {
        let dir = TempDir::new().unwrap();
        let a = DeviceIdentity::load_or_create(dir.path()).unwrap();
        let b = DeviceIdentity::load_or_create(dir.path()).unwrap();
        assert_eq!(
            a.public_key_b64(),
            b.public_key_b64(),
            "keypair must persist"
        );
        assert_eq!(a.device_id(), b.device_id());
    }
    #[test]
    fn device_id_is_32_hex_chars() {
        let dir = TempDir::new().unwrap();
        let id = DeviceIdentity::load_or_create(dir.path()).unwrap();
        assert_eq!(id.device_id().len(), 32);
        assert!(id.device_id().chars().all(|c| c.is_ascii_hexdigit()));
    }
    #[test]
    fn persisted_file_is_mode_0600() {
        let dir = TempDir::new().unwrap();
        let _ = DeviceIdentity::load_or_create(dir.path()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(dir.path().join("remote-identity.json")).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
    }
}
