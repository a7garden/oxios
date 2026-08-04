//! QR pairing offer (RFC-044 §6.2). `oxios://pair?code=<base64url json>`.
//! Symbols are intentionally forward-declared; `#[allow(dead_code)]` silences
//! the warning until Tasks 8 (RPC) and 10 (serve CLI) consume this API.
#![allow(dead_code)]
use anyhow::{Result, anyhow};
use base64::Engine;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PairingOffer {
    pub v: u32,
    pub endpoint: String,
    pub device_id: String,
    pub public_key_b64: String,
    pub endpoints: Vec<String>,
    pub scope: String,
}

impl PairingOffer {
    pub fn encode_url(&self) -> String {
        let json = serde_json::to_string(self).expect("serialize offer");
        let code = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json);
        format!("oxios://pair?code={code}")
    }
    pub fn decode_url(url: &str) -> Result<Self> {
        let code = url
            .strip_prefix("oxios://pair?code=")
            .or_else(|| url.strip_prefix("oxios://pair#code="))
            .ok_or_else(|| anyhow!("not an oxios://pair URL"))?;
        let json = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(code)?;
        Ok(serde_json::from_slice(&json)?)
    }
    /// Render the offer URL as an SVG QR code (terminal/printable).
    pub fn qr_svg(&self) -> Result<String> {
        let url = self.encode_url();
        let bits = qrcode::QrCode::new(url.as_bytes())?;
        Ok(bits
            .render::<qrcode::render::svg::Color>()
            .min_dimensions(240, 240)
            .build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_encode_decode() {
        let o = PairingOffer {
            v: 1,
            endpoint: "ws://100.64.1.20:6768".into(),
            device_id: "abc".into(),
            public_key_b64: "pk".into(),
            endpoints: vec![
                "ws://100.64.1.20:6768".into(),
                "ws://192.168.1.20:6768".into(),
            ],
            scope: "mobile".into(),
        };
        let url = o.encode_url();
        assert!(url.starts_with("oxios://pair?code="));
        let back = PairingOffer::decode_url(&url).unwrap();
        assert_eq!(back.device_id, "abc");
        assert_eq!(back.endpoints.len(), 2);
    }
    #[test]
    fn qr_svg_is_nonempty() {
        let o = PairingOffer {
            v: 1,
            endpoint: "ws://x:6768".into(),
            device_id: "d".into(),
            public_key_b64: "k".into(),
            endpoints: vec!["ws://x:6768".into()],
            scope: "mobile".into(),
        };
        let svg = o.qr_svg().unwrap();
        assert!(svg.contains("<svg"));
    }
}
