use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use shared_common::errors::{AppError, AppResult};

pub async fn validate_outbound_url(url: &str) -> AppResult<reqwest::Url> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| AppError::BadRequest(format!("invalid webhook url: {e}")))?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AppError::BadRequest(format!(
                "webhook url scheme must be http or https, got {other}"
            )));
        }
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::BadRequest("webhook url has no host".into()))?;

    if let Ok(ip) = host.parse::<IpAddr>() {
        if !is_public_ip(&ip) {
            return Err(AppError::BadRequest(
                "webhook url resolves to a disallowed (private/loopback/link-local) address".into(),
            ));
        }
        return Ok(parsed);
    }

    let port = parsed.port_or_known_default().unwrap_or(0);
    let authority = format!("{host}:{port}");

    let mut resolved = tokio::net::lookup_host(&authority)
        .await
        .map_err(|e| AppError::BadRequest(format!("webhook host resolution failed: {e}")))?
        .peekable();

    if resolved.peek().is_none() {
        return Err(AppError::BadRequest(
            "webhook host did not resolve to any address".into(),
        ));
    }

    for addr in resolved {
        if !is_public_ip(&addr.ip()) {
            return Err(AppError::BadRequest(
                "webhook host resolves to a disallowed (private/loopback/link-local) address"
                    .into(),
            ));
        }
    }

    Ok(parsed)
}

fn is_public_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_ipv4(v4),
        IpAddr::V6(v6) => is_public_ipv6(v6),
    }
}

fn is_public_ipv4(ip: &Ipv4Addr) -> bool {
    if ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
    {
        return false;
    }
    if ip.octets()[0] == 0 {
        return false;
    }
    let o = ip.octets();
    if o[0] == 100 && (o[1] & 0xc0) == 0x40 {
        return false;
    }
    true
}

fn is_public_ipv6(ip: &Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return false;
    }
    let segs = ip.segments();
    if (segs[0] & 0xfe00) == 0xfc00 {
        return false;
    }
    if (segs[0] & 0xffc0) == 0xfe80 {
        return false;
    }
    if let Some(v4) = ip.to_ipv4() {
        return is_public_ipv4(&v4);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pub_v4(s: &str) -> bool {
        is_public_ipv4(&s.parse().unwrap())
    }
    fn pub_v6(s: &str) -> bool {
        is_public_ipv6(&s.parse().unwrap())
    }

    #[test]
    fn rejects_private_and_special_v4() {
        assert!(!pub_v4("127.0.0.1"));
        assert!(!pub_v4("10.0.0.5"));
        assert!(!pub_v4("172.16.0.1"));
        assert!(!pub_v4("172.31.255.255"));
        assert!(!pub_v4("192.168.1.1"));
        assert!(!pub_v4("169.254.169.254"));
        assert!(!pub_v4("0.0.0.0"));
        assert!(!pub_v4("100.64.0.1"));
        assert!(!pub_v4("224.0.0.1"));
    }

    #[test]
    fn allows_public_v4() {
        assert!(pub_v4("8.8.8.8"));
        assert!(pub_v4("1.1.1.1"));
        assert!(pub_v4("93.184.216.34"));
        assert!(pub_v4("172.32.0.1"));
    }

    #[test]
    fn rejects_special_v6() {
        assert!(!pub_v6("::1"));
        assert!(!pub_v6("::"));
        assert!(!pub_v6("fc00::1"));
        assert!(!pub_v6("fd12::1"));
        assert!(!pub_v6("fe80::1"));
        assert!(!pub_v6("ff02::1"));
        assert!(!pub_v6("::ffff:169.254.169.254"));
    }

    #[test]
    fn allows_public_v6() {
        assert!(pub_v6("2606:4700:4700::1111"));
    }

    #[tokio::test]
    async fn rejects_non_http_scheme() {
        assert!(validate_outbound_url("file:///etc/passwd").await.is_err());
        assert!(validate_outbound_url("ftp://example.com/x").await.is_err());
    }

    #[tokio::test]
    async fn rejects_literal_internal_ip() {
        assert!(
            validate_outbound_url("http://169.254.169.254/latest/meta-data/")
                .await
                .is_err()
        );
        assert!(validate_outbound_url("http://127.0.0.1:8080/")
            .await
            .is_err());
        assert!(validate_outbound_url("http://[::1]/").await.is_err());
    }
}
