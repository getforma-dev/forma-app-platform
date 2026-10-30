//! ADR-007 D7.14 trusted proxy: only the ALB reaches the task and appends the
//! peer address, so the client is the rightmost `X-Forwarded-For` hop; with no
//! header (a direct local connection) it is the socket peer. The address class
//! keys the in-process limits: an IPv4 address, or an IPv6 /64.
use axum::http::HeaderMap;
use std::net::{IpAddr, SocketAddr};

pub fn client_address(headers: &HeaderMap, peer: Option<SocketAddr>) -> Option<IpAddr> {
    let forwarded: Vec<_> = headers.get_all("x-forwarded-for").iter().collect();
    if let Some(last) = forwarded.last() {
        // An unparseable rightmost hop is no address at all, never a fallback
        // to an earlier, client-controlled entry.
        return last.to_str().ok()?.rsplit(',').next()?.trim().parse().ok();
    }
    peer.map(|peer| peer.ip())
}

/// The limit key of an address: `v4:a.b.c.d`, `v6:<first 64 bits>`, or
/// `unknown` (still limited, as one class).
pub fn address_class(headers: &HeaderMap, peer: Option<SocketAddr>) -> String {
    match client_address(headers, peer) {
        Some(IpAddr::V4(v4)) => format!("v4:{v4}"),
        Some(IpAddr::V6(v6)) => match v6.to_ipv4_mapped() {
            Some(v4) => format!("v4:{v4}"),
            None => {
                let s = v6.segments();
                format!("v6:{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
        None => "unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn rightmost_forwarded_hop_wins_and_ipv6_is_a_slash_64() {
        let peer: SocketAddr = "10.0.0.9:443".parse().unwrap();
        let mut headers = HeaderMap::new();
        assert_eq!(address_class(&headers, Some(peer)), "v4:10.0.0.9");
        assert_eq!(address_class(&headers, None), "unknown");
        headers.append(
            "x-forwarded-for",
            HeaderValue::from_static("198.51.100.1, 203.0.113.7"),
        );
        assert_eq!(address_class(&headers, Some(peer)), "v4:203.0.113.7");
        headers.append(
            "x-forwarded-for",
            HeaderValue::from_static("2001:db8:1:2:3::9"),
        );
        assert_eq!(address_class(&headers, Some(peer)), "v6:2001:db8:1:2::/64");
        let mut forged = HeaderMap::new();
        forged.append(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.7, garbage"),
        );
        assert_eq!(address_class(&forged, Some(peer)), "unknown");
    }
}
