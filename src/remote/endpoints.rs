//! Reachable direct-endpoint enumeration (RFC-044 §6.6).
#![allow(dead_code)]
use std::process::Command;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointKind {
    Lan,
    Tailscale,
}

/// Classify a host as Tailscale (CGNAT 100.64.0.0/10 or *.ts.net) or LAN.
pub fn classify(host: &str) -> EndpointKind {
    if host.ends_with(".ts.net") {
        return EndpointKind::Tailscale;
    }
    if let Some(parts) = parse_ipv4(host) {
        // CGNAT: 100.64.0.0/10  → octet0==100, octet1 in 64..=127
        if parts[0] == 100 && (64..=127).contains(&parts[1]) {
            return EndpointKind::Tailscale;
        }
    }
    EndpointKind::Lan
}
fn parse_ipv4(s: &str) -> Option<[u8; 4]> {
    let mut out = [0u8; 4];
    let mut it = s.split('.');
    for slot in out.iter_mut() {
        *slot = it.next()?.parse().ok()?;
    }
    it.next().is_none().then_some(out)
}

/// Enumerate direct WS URLs: Tailscale IPv4 (if `tailscale` CLI present) then LAN IPs.
pub fn enumerate_direct(port: u16) -> Vec<(String, EndpointKind)> {
    let mut out = Vec::new();
    if let Some(ts) = read_tailscale_ip() {
        out.push((format!("ws://{ts}:{port}"), EndpointKind::Tailscale));
    }
    if let Ok(ip) = local_ip_address::local_ip() {
        let host = ip.to_string();
        if classify(&host) == EndpointKind::Lan {
            out.push((format!("ws://{host}:{port}"), EndpointKind::Lan));
        }
    }
    out
}
fn read_tailscale_ip() -> Option<String> {
    let o = Command::new("tailscale").args(["ip", "-4"]).output().ok()?;
    if !o.status.success() {
        return None;
    }
    std::str::from_utf8(&o.stdout)
        .ok()?
        .lines()
        .next()
        .map(str::to_string)
}

/// Order endpoints Tailscale-first, deduped, as full ws:// URLs.
pub fn build_offer_endpoints(list: &[(String, EndpointKind)]) -> Vec<String> {
    let mut ts: Vec<_> = list
        .iter()
        .filter(|(_, k)| *k == EndpointKind::Tailscale)
        .map(|(u, _)| u.clone())
        .collect();
    let mut lan: Vec<_> = list
        .iter()
        .filter(|(_, k)| *k == EndpointKind::Lan)
        .map(|(u, _)| u.clone())
        .collect();
    ts.append(&mut lan);
    ts.sort();
    ts.dedup();
    ts
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classify_tailscale_cgnat() {
        assert!(matches!(classify("100.64.1.20"), EndpointKind::Tailscale));
        assert!(matches!(classify("100.127.255.1"), EndpointKind::Tailscale));
    }
    #[test]
    fn classify_tailscale_magicdns() {
        assert!(matches!(
            classify("my-mac.tailnet.ts.net"),
            EndpointKind::Tailscale
        ));
    }
    #[test]
    fn classify_lan_and_non_cgnat() {
        assert!(matches!(classify("192.168.1.20"), EndpointKind::Lan));
        // 100.20.x.x is NOT in the CGNAT 100.64.0.0/10 range.
        assert!(matches!(classify("100.20.1.5"), EndpointKind::Lan));
    }
    #[test]
    fn build_offer_tailscale_first() {
        let list = vec![
            ("ws://192.168.1.20:6768".into(), EndpointKind::Lan),
            ("ws://100.64.1.20:6768".into(), EndpointKind::Tailscale),
        ];
        let urls = build_offer_endpoints(&list);
        assert_eq!(urls[0], "ws://100.64.1.20:6768");
    }
}
