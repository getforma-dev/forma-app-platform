//! ADR-007 D7.14 trusted proxy: only the ALB reaches the task and appends the
//! peer address, so behind it the client is the rightmost `X-Forwarded-For`
//! hop. APP-074 restricts that reading to the load balancer's peers
//! (`GENUI_TRUSTED_PROXY_CIDRS`, the ALB's subnets): a header arriving from
//! any other peer is ignored and the socket peer is the client, so a forged
//! header from an untrusted hop never selects a limit key. With no trusted
//! peers configured (local runs) every header is ignored. The address class
//! keys the in-process limits: an IPv4 address, or an IPv6 /64.
use axum::http::HeaderMap;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};

/// The deployment's load-balancer subnets, comma-separated CIDRs.
pub const TRUSTED_PROXY_ENV: &str = "GENUI_TRUSTED_PROXY_CIDRS";
const MAX_NETWORKS: usize = 16;

/// The peers whose rightmost `X-Forwarded-For` hop is believed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrustedProxies(Vec<(IpAddr, u8)>);

/// IPv4-mapped IPv6 peers are judged as IPv4.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 => v4,
    }
}
fn bits(ip: IpAddr) -> u128 {
    match ip {
        IpAddr::V4(v4) => u128::from(u32::from(v4)),
        IpAddr::V6(v6) => u128::from(v6),
    }
}
fn width(ip: IpAddr) -> u8 {
    if ip.is_ipv4() {
        32
    } else {
        128
    }
}
/// The network mask of a `prefix` (1..=width) in an address of `width` bits.
fn mask(prefix: u8, width: u8) -> u128 {
    let ones = u128::MAX >> (128 - u32::from(width));
    if prefix == 0 {
        0
    } else {
        ones & !((1u128 << (width - prefix)) - 1)
    }
}

impl TrustedProxies {
    /// No trusted peer: every `X-Forwarded-For` header is ignored.
    pub fn none() -> Self {
        Self::default()
    }
    /// `a.b.c.d/n` or `x::/n` entries, comma-separated, canonical (no host
    /// bits), at most 16, never the whole address space. Empty is `none()`.
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        let refused = "trusted proxy networks refused";
        if text.trim().is_empty() {
            return Ok(Self::none());
        }
        let mut networks = Vec::new();
        for entry in text.split(',') {
            let (address, prefix) = entry.trim().split_once('/').ok_or(refused)?;
            let ip: IpAddr = address.parse().map_err(|_| refused)?;
            if matches!(ip, IpAddr::V6(v6) if v6.to_ipv4_mapped().is_some()) {
                return Err(refused);
            }
            if prefix.is_empty() || prefix.len() > 3 || !prefix.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(refused);
            }
            let prefix: u8 = prefix.parse().map_err(|_| refused)?;
            if prefix == 0 || prefix > width(ip) || bits(ip) & !mask(prefix, width(ip)) != 0 {
                return Err(refused);
            }
            networks.push((ip, prefix));
        }
        if networks.len() > MAX_NETWORKS {
            return Err(refused);
        }
        Ok(Self(networks))
    }
    /// The deployment setting; absent is `none()`.
    pub fn from_env() -> Result<Self, &'static str> {
        match std::env::var(TRUSTED_PROXY_ENV) {
            Ok(text) => Self::parse(&text),
            Err(std::env::VarError::NotPresent) => Ok(Self::none()),
            Err(_) => Err("trusted proxy networks refused"),
        }
    }
    pub fn trusts(&self, peer: IpAddr) -> bool {
        let peer = canonical(peer);
        self.0.iter().any(|(network, prefix)| {
            network.is_ipv4() == peer.is_ipv4()
                && bits(peer) & mask(*prefix, width(peer)) == bits(*network)
        })
    }
}

/// The client: the socket peer, or, when that peer is a trusted load
/// balancer, the rightmost hop of its last `X-Forwarded-For` line. An
/// unparseable rightmost hop is no address at all, never a fallback to an
/// earlier, client-controlled entry.
pub fn client_address(
    headers: &HeaderMap,
    peer: Option<SocketAddr>,
    trusted: &TrustedProxies,
) -> Option<IpAddr> {
    let peer = canonical(peer?.ip());
    if !trusted.trusts(peer) {
        return Some(peer);
    }
    let forwarded: Vec<_> = headers.get_all("x-forwarded-for").iter().collect();
    match forwarded.last() {
        Some(last) => last
            .to_str()
            .ok()?
            .rsplit(',')
            .next()?
            .trim()
            .parse()
            .ok()
            .map(canonical),
        None => Some(peer),
    }
}

/// The limit key of an address: `v4:a.b.c.d`, `v6:<first 64 bits>`, or
/// `unknown` (still limited, as one class).
pub fn address_class(
    headers: &HeaderMap,
    peer: Option<SocketAddr>,
    trusted: &TrustedProxies,
) -> String {
    match client_address(headers, peer, trusted) {
        Some(IpAddr::V4(v4)) => format!("v4:{v4}"),
        Some(IpAddr::V6(v6)) => {
            let s = v6.segments();
            let prefix = Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0);
            format!("v6:{prefix}/64")
        }
        None => "unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn forwarded(values: &[&'static str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append("x-forwarded-for", HeaderValue::from_static(value));
        }
        headers
    }

    #[test]
    fn rightmost_forwarded_hop_wins_only_behind_a_trusted_peer() {
        let alb = TrustedProxies::parse("10.72.0.0/24, 10.72.1.0/24").unwrap();
        let balancer: SocketAddr = "10.72.1.9:443".parse().unwrap();
        let direct: SocketAddr = "198.51.100.4:5000".parse().unwrap();
        let none = HeaderMap::new();
        assert_eq!(address_class(&none, Some(balancer), &alb), "v4:10.72.1.9");
        assert_eq!(address_class(&none, None, &alb), "unknown");
        let chain = forwarded(&["198.51.100.1, 203.0.113.7"]);
        assert_eq!(
            address_class(&chain, Some(balancer), &alb),
            "v4:203.0.113.7"
        );
        // Untrusted peer: the header is ignored whatever it says.
        assert_eq!(address_class(&chain, Some(direct), &alb), "v4:198.51.100.4");
        assert_eq!(
            address_class(&chain, Some(balancer), &TrustedProxies::none()),
            "v4:10.72.1.9",
            "no trusted peers configured: every header is ignored"
        );
        // The last header line is the balancer's; an earlier, forged line is not read.
        let lines = forwarded(&["192.0.2.66", "2001:db8:1:2:3::9"]);
        assert_eq!(
            address_class(&lines, Some(balancer), &alb),
            "v6:2001:db8:1:2::/64"
        );
        // An unparseable rightmost hop is no address, never the earlier entry.
        let garbage = forwarded(&["203.0.113.7, garbage"]);
        assert_eq!(address_class(&garbage, Some(balancer), &alb), "unknown");
        // IPv4-mapped peers and hops are IPv4.
        let mapped: SocketAddr = "[::ffff:10.72.0.5]:443".parse().unwrap();
        assert_eq!(address_class(&chain, Some(mapped), &alb), "v4:203.0.113.7");
        let mapped_hop = forwarded(&["::ffff:203.0.113.8"]);
        assert_eq!(
            address_class(&mapped_hop, Some(balancer), &alb),
            "v4:203.0.113.8"
        );
        assert_eq!(
            address_class(&none, Some("[2001:db8:9:8:7::1]:9".parse().unwrap()), &alb),
            "v6:2001:db8:9:8::/64"
        );
    }

    #[test]
    fn trusted_networks_parse_canonically_and_bounded() {
        let loopback = TrustedProxies::parse("127.0.0.1/32,::1/128").unwrap();
        assert!(loopback.trusts("127.0.0.1".parse().unwrap()));
        assert!(loopback.trusts("::1".parse().unwrap()));
        assert!(loopback.trusts("::ffff:127.0.0.1".parse().unwrap()));
        assert!(!loopback.trusts("127.0.0.2".parse().unwrap()));
        let v6 = TrustedProxies::parse("2001:db8::/32").unwrap();
        assert!(v6.trusts("2001:db8:ffff::1".parse().unwrap()));
        assert!(!v6.trusts("2001:db9::1".parse().unwrap()));
        let vpc = TrustedProxies::parse("10.72.0.0/16").unwrap();
        assert!(vpc.trusts("10.72.255.255".parse().unwrap()));
        assert!(!vpc.trusts("10.73.0.0".parse().unwrap()));
        assert_eq!(TrustedProxies::parse(" ").unwrap(), TrustedProxies::none());
        for bad in [
            "10.72.0.1/16",
            "0.0.0.0/0",
            "10.72.0.0",
            "10.72.0.0/33",
            "10.72.0.0/",
            "10.72.0.0/+8",
            "::ffff:10.0.0.0/104",
            "example.test/24",
            "10.72.0.0/24,",
        ] {
            assert!(TrustedProxies::parse(bad).is_err(), "{bad}");
        }
        let many = vec!["10.0.0.0/24"; MAX_NETWORKS + 1].join(",");
        assert!(TrustedProxies::parse(&many).is_err());
    }
}
