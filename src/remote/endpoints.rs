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
///
/// Order is **preserved**: the partition is built Tailscale-then-LAN and then
/// deduplicated by an order-preserving pass (a `HashSet<String>` seen-set). A
/// naive `sort() + dedup()` would re-shuffle by string comparison and break the
/// Tailscale-first contract when a Tailscale URL sorts after a LAN URL
/// (e.g. `ws://my-mac.ts.net:6768` vs `ws://192.168.1.20:6768`, or the
/// CGNAT-vs-LAN string case `ws://10...` vs `ws://100...`).
pub fn build_offer_endpoints(list: &[(String, EndpointKind)]) -> Vec<String> {
    let mut ts: Vec<&str> = list
        .iter()
        .filter(|(_, k)| *k == EndpointKind::Tailscale)
        .map(|(u, _)| u.as_str())
        .collect();
    let lan: Vec<&str> = list
        .iter()
        .filter(|(_, k)| *k == EndpointKind::Lan)
        .map(|(u, _)| u.as_str())
        .collect();
    ts.extend(lan);
    let mut seen = std::collections::HashSet::with_capacity(ts.len());
    ts.into_iter()
        .filter(|u| seen.insert(*u))
        .map(str::to_string)
        .collect()
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
    #[test]
    fn build_offer_tailscale_first_when_ts_sorts_after_lan() {
        // Regression: `ts.net` host sorts AFTER a `192.168.x.x` LAN host under
        // string comparison, so a naive `sort() + dedup()` would re-shuffle
        // and put the LAN URL first, breaking the Tailscale-first contract.
        let list = vec![
            ("ws://192.168.1.20:6768".into(), EndpointKind::Lan),
            ("ws://my-mac.ts.net:6768".into(), EndpointKind::Tailscale),
        ];
        let urls = build_offer_endpoints(&list);
        assert_eq!(urls[0], "ws://my-mac.ts.net:6768");
        assert_eq!(urls[1], "ws://192.168.1.20:6768");
    }
    #[test]
    fn build_offer_tailscale_first_when_ts_string_less_than_lan() {
        // Regression: the CGNAT `100.64.1.20` URL string sorts AFTER the LAN
        // `10.0.0.5` URL (`'1' == '1', '0' == '0', '0' < '0'` then `100...` vs
        // `10...` — `'0' < '.'`), so a naive `sort() + dedup()` would put
        // the LAN URL first.
        let list = vec![
            ("ws://10.0.0.5:6768".into(), EndpointKind::Lan),
            ("ws://100.64.1.20:6768".into(), EndpointKind::Tailscale),
        ];
        let urls = build_offer_endpoints(&list);
        assert_eq!(urls[0], "ws://100.64.1.20:6768");
        assert_eq!(urls[1], "ws://10.0.0.5:6768");
    }
}
