use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;
use ipnet::IpNet;

fn parse_net(spec: &str) -> Option<IpNet> {
    let spec = spec.trim();
    spec.parse::<IpNet>()
        .ok()
        .or_else(|| spec.parse::<IpAddr>().ok().map(IpNet::from))
}

fn unmapped(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 => v4,
    }
}

pub fn parse_trusted_proxies(spec: &str) -> Vec<IpNet> {
    spec.split(',')
        .filter(|s| !s.trim().is_empty())
        .filter_map(parse_net)
        .collect()
}

/// The forwarded header is only believed when the request actually arrived from
/// one of our proxies. Otherwise a caller sets `X-Forwarded-For` themselves and
/// gets a fresh rate-limit bucket per request — which is the whole point of
/// keying a limit on the address.
pub fn client_ip(
    headers: &HeaderMap,
    peer: Option<SocketAddr>,
    trusted: &[IpNet],
) -> Option<String> {
    let peer_is_trusted = match peer {
        Some(addr) => trusted.iter().any(|net| net.contains(&unmapped(addr.ip()))),
        // No peer information (unit tests, or a server built without
        // ConnectInfo): fall back to the header rather than to nothing.
        None => true,
    };

    if peer_is_trusted {
        if let Some(forwarded) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            return Some(forwarded);
        }
        if let Some(real) = headers
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            return Some(real);
        }
    }

    peer.map(|addr| addr.ip().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(forwarded: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", forwarded.parse().unwrap());
        h
    }

    fn peer(addr: &str) -> Option<SocketAddr> {
        Some(SocketAddr::new(addr.parse().unwrap(), 1234))
    }

    #[test]
    fn parses_cidr_blocks_and_bare_addresses() {
        assert!(parse_net("10.0.0.0/8")
            .unwrap()
            .contains(&"10.9.9.9".parse::<IpAddr>().unwrap()));
        assert!(!parse_net("10.0.0.0/8")
            .unwrap()
            .contains(&"11.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(parse_net("127.0.0.1")
            .unwrap()
            .contains(&"127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(parse_net("fd00::/8")
            .unwrap()
            .contains(&"fd00::1".parse::<IpAddr>().unwrap()));
        assert!(parse_net("nonsense").is_none());
    }

    #[test]
    fn a_v4_mapped_v6_peer_matches_a_v4_proxy_range() {
        let trusted = parse_trusted_proxies("172.16.0.0/12");
        let ip = client_ip(&headers("203.0.113.7"), peer("::ffff:172.18.0.5"), &trusted);
        assert_eq!(ip.as_deref(), Some("203.0.113.7"));
    }

    #[test]
    fn believes_the_forwarded_header_from_our_own_proxy() {
        let trusted = parse_trusted_proxies("172.16.0.0/12");
        let ip = client_ip(&headers("203.0.113.7"), peer("172.18.0.5"), &trusted);
        assert_eq!(ip.as_deref(), Some("203.0.113.7"));
    }

    #[test]
    fn ignores_a_forwarded_header_from_an_untrusted_peer() {
        let trusted = parse_trusted_proxies("172.16.0.0/12");
        let ip = client_ip(&headers("203.0.113.7"), peer("8.8.8.8"), &trusted);
        assert_eq!(
            ip.as_deref(),
            Some("8.8.8.8"),
            "a spoofed header must not win a fresh bucket"
        );
    }

    #[test]
    fn falls_back_to_the_peer_when_no_header_is_present() {
        let trusted = parse_trusted_proxies("172.16.0.0/12");
        let ip = client_ip(&HeaderMap::new(), peer("172.18.0.5"), &trusted);
        assert_eq!(ip.as_deref(), Some("172.18.0.5"));
    }
}
