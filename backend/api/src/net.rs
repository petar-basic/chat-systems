use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::http::HeaderMap;

/// An IPv4 CIDR block. Hand-rolled rather than pulling a crate in: the only
/// thing this needs to answer is "did this request arrive from our own reverse
/// proxy", and the deployment's proxies are always private IPv4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    network: u32,
    prefix: u8,
}

impl Cidr {
    pub fn parse(spec: &str) -> Option<Self> {
        let (addr, prefix) = match spec.split_once('/') {
            Some((addr, prefix)) => (addr, prefix.parse::<u8>().ok()?),
            None => (spec, 32),
        };
        if prefix > 32 {
            return None;
        }
        let addr: Ipv4Addr = addr.trim().parse().ok()?;
        let mask = mask_for(prefix);
        Some(Self {
            network: u32::from(addr) & mask,
            prefix,
        })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        let v4 = match ip {
            IpAddr::V4(v4) => v4,
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => v4,
                None => return false,
            },
        };
        u32::from(v4) & mask_for(self.prefix) == self.network
    }
}

fn mask_for(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    }
}

pub fn parse_trusted_proxies(spec: &str) -> Vec<Cidr> {
    spec.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(Cidr::parse)
        .collect()
}

/// The forwarded header is only believed when the request actually arrived from
/// one of our proxies. Otherwise a caller sets `X-Forwarded-For` themselves and
/// gets a fresh rate-limit bucket per request — which is the whole point of
/// keying a limit on the address.
pub fn client_ip(
    headers: &HeaderMap,
    peer: Option<SocketAddr>,
    trusted: &[Cidr],
) -> Option<String> {
    let peer_is_trusted = match peer {
        Some(addr) => trusted.iter().any(|c| c.contains(addr.ip())),
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
        assert!(Cidr::parse("10.0.0.0/8")
            .unwrap()
            .contains("10.9.9.9".parse().unwrap()));
        assert!(!Cidr::parse("10.0.0.0/8")
            .unwrap()
            .contains("11.0.0.1".parse().unwrap()));
        assert!(Cidr::parse("127.0.0.1")
            .unwrap()
            .contains("127.0.0.1".parse().unwrap()));
        assert!(Cidr::parse("nonsense").is_none());
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
