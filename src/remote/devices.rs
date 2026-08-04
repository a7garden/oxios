//! Paired-device registry (RFC-044 §6.1). Tokens hashed at rest.
//!
//! Public API consumed by RPC (Task 8) and surface wiring (Task 9).
//! Symbols are intentionally forward-declared; `#[allow(dead_code)]` silences
//! the warnings until those callers land.
#![allow(dead_code)]

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceEntry {
    pub device_id: String,
    pub token_hash: String,
    pub name: String,
    pub scope: String,
    pub paired_at: i64,
    pub last_seen: Option<i64>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RegistryFile {
    schema_version: u32,
    devices: Vec<DeviceEntry>,
}

pub struct DeviceRegistry {
    path: PathBuf,
    devices: Vec<DeviceEntry>,
}

impl DeviceRegistry {
    pub fn load_or_create(state_dir: &Path) -> Result<Self> {
        let path = state_dir.join("devices.json");
        let devices = match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<RegistryFile>(&bytes) {
                Ok(file) => file.devices,
                Err(e) => {
                    tracing::warn!("devices.json unreadable/corrupt, starting empty: {e}");
                    Vec::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!("devices.json not found (first run), starting empty");
                Vec::new()
            }
            Err(e) => {
                tracing::warn!("devices.json unreadable, starting empty: {e}");
                Vec::new()
            }
        };
        Ok(Self { path, devices })
    }

    fn save(&self) -> Result<()> {
        let file = RegistryFile {
            schema_version: 1,
            devices: self.devices.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&file)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;

            let tmp = self.path.with_extension("json.tmp");
            let result = (|| -> Result<()> {
                let mut output = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(&tmp)?;
                std::io::Write::write_all(&mut output, &bytes)?;
                output.sync_all()?;
                std::fs::rename(&tmp, &self.path)?;
                Ok(())
            })();
            if result.is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
            result
        }

        #[cfg(not(unix))]
        {
            let tmp = self.path.with_extension("json.tmp");
            let result = (|| -> Result<()> {
                let mut output = std::fs::File::create(&tmp)?;
                std::io::Write::write_all(&mut output, &bytes)?;
                output.sync_all()?;
                std::fs::rename(&tmp, &self.path)?;
                Ok(())
            })();
            if result.is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
            result
        }
    }

    /// Mint a new device token. Returns `(device_id, plaintext_token)`.
    ///
    /// The token is shown once; only its hash is persisted.
    pub fn pair(&mut self, name: &str, scope: &str) -> Result<(String, String)> {
        let mut token = [0_u8; 32];
        getrandom::getrandom(&mut token).expect("rng");
        let token_hex = hex::encode(token);
        let mut hasher = Sha256::new();
        hasher.update(token);
        let token_hash = hex::encode(hasher.finalize());
        let device_id = uuid::Uuid::new_v4().to_string();
        self.devices.push(DeviceEntry {
            device_id: device_id.clone(),
            token_hash,
            name: name.into(),
            scope: scope.into(),
            paired_at: chrono::Utc::now().timestamp(),
            last_seen: None,
        });
        self.save()?;
        Ok((device_id, token_hex))
    }

    pub fn verify(&self, device_id: &str, token: &str) -> bool {
        let mut token_bytes = [0_u8; 32];
        if hex::decode_to_slice(token, &mut token_bytes).is_err() {
            return false;
        }
        let mut hasher = Sha256::new();
        hasher.update(token_bytes);
        let token_hash = hex::encode(hasher.finalize());
        self.devices
            .iter()
            .any(|device| device.device_id == device_id && device.token_hash == token_hash)
    }

    pub fn list(&self) -> Vec<&DeviceEntry> {
        self.devices.iter().collect()
    }

    pub fn revoke(&mut self, device_id: &str) -> Result<bool> {
        let before = self.devices.len();
        self.devices.retain(|device| device.device_id != device_id);
        let changed = self.devices.len() != before;
        if changed {
            self.save()?;
        }
        Ok(changed)
    }

    pub fn touch(&mut self, device_id: &str) -> Result<()> {
        if let Some(device) = self
            .devices
            .iter_mut()
            .find(|device| device.device_id == device_id)
        {
            device.last_seen = Some(chrono::Utc::now().timestamp());
            self.save()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn pair_verify_revoke_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut reg = DeviceRegistry::load_or_create(dir.path()).unwrap();
        let (id, token) = reg.pair("pixel-9", "mobile").unwrap();
        assert!(reg.verify(&id, &token));
        assert!(!reg.verify(&id, "wrong-token"));
        assert_eq!(reg.list().len(), 1);
        assert!(reg.revoke(&id).unwrap());
        assert!(!reg.verify(&id, &token));
    }

    #[test]
    fn persisted_token_is_hashed_not_plaintext() {
        let dir = TempDir::new().unwrap();
        let mut reg = DeviceRegistry::load_or_create(dir.path()).unwrap();
        let (id, token) = reg.pair("iphone", "mobile").unwrap();
        let raw = std::fs::read_to_string(dir.path().join("devices.json")).unwrap();
        assert!(
            !raw.contains(&token),
            "plaintext token must not be persisted"
        );
        assert!(raw.contains(&id));
    }
}
