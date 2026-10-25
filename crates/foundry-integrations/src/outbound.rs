//! The outbound address policy (DDR-14, ADR-EIP-006 decision 6, OD-13).
//!
//! [`judge`] decides whether Foundry may connect to one address. Always
//! refused: loopback, "this network", reserved `240.0.0.0/4`, documentation
//! and benchmarking ranges, link-local, cloud metadata, multicast and
//! broadcast. Refused unless the operator lists it in
//! `FOUNDRY_OUTBOUND_ALLOW` ([`OutboundAllow`]): RFC 1918, CGNAT and
//! unique-local. An IPv6 address embedding an IPv4 address (IPv4-mapped,
//! IPv4-compatible, NAT64, 6to4) is judged by the IPv4 address it embeds.
//!
//! [`OutboundTarget::parse`] reads the host and port of an address the way a
//! browser does (decimal, hex, octal and short IPv4 forms are addresses, a
//! port-less `https` address is port 443), so a literal cannot slip past the
//! judge in a spelling it does not recognise.

use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// The setting naming the private ranges and hosts Foundry may reach.
pub const OUTBOUND_ALLOW_SETTING: &str = "FOUNDRY_OUTBOUND_ALLOW";

/// The judge's answer for one address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Refuse(Refusal),
}

/// Why an address is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Never reachable, whatever the allow-list says.
    Reserved,
    /// A private address the allow-list does not name.
    Private,
}

const ALWAYS_REFUSED_V4: [&str; 9] = [
    "0.0.0.0/8",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "192.0.2.0/24",
    "198.51.100.0/24",
    "203.0.113.0/24",
    "198.18.0.0/15",
    "224.0.0.0/4",
    // Reserved, and the broadcast address 255.255.255.255.
    "240.0.0.0/4",
];
const PRIVATE_V4: [&str; 4] = [
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "100.64.0.0/10",
];
/// `::1` and `::` embed `0.0.0.1` and `0.0.0.0`, so "this network" refuses them.
const ALWAYS_REFUSED_V6: [&str; 4] = [
    "fe80::/10",
    "ff00::/8",
    "2001:db8::/32",
    "fd00:ec2::254/128",
];
const PRIVATE_V6: [&str; 1] = ["fc00::/7"];

fn v4_in(ranges: &[&str], ip: Ipv4Addr) -> bool {
    ranges
        .iter()
        .filter_map(|range| range.parse::<Ipv4Net>().ok())
        .any(|net| net.contains(&ip))
}

fn v6_in(ranges: &[&str], ip: Ipv6Addr) -> bool {
    ranges
        .iter()
        .filter_map(|range| range.parse::<Ipv6Net>().ok())
        .any(|net| net.contains(&ip))
}

/// The IPv4 address an IPv6 address embeds, if it embeds one: IPv4-mapped
/// `::ffff:0:0/96`, IPv4-compatible `::/96`, NAT64 `64:ff9b::/96` and
/// `64:ff9b:1::/48` (the address in the last 32 bits) and 6to4 `2002::/16`.
fn embedded_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = ip.segments();
    let last = Ipv4Addr::new((s[6] >> 8) as u8, s[6] as u8, (s[7] >> 8) as u8, s[7] as u8);
    let mapped = s[..5] == [0; 5] && s[5] == 0xffff;
    let compatible = s[..6] == [0; 6];
    let nat64 = s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] || s[..3] == [0x64, 0xff9b, 1];
    if mapped || compatible || nat64 {
        return Some(last);
    }
    (s[0] == 0x2002)
        .then(|| Ipv4Addr::new((s[1] >> 8) as u8, s[1] as u8, (s[2] >> 8) as u8, s[2] as u8))
}

/// The address judged in place of `ip`: an embedded IPv4 address if any.
fn judged(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => embedded_v4(v6).map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    }
}

/// What kind of address `ip` is, before the allow-list.
fn class(ip: IpAddr) -> Option<Refusal> {
    let reserved = match ip {
        IpAddr::V4(v4) => v4_in(&ALWAYS_REFUSED_V4, v4),
        IpAddr::V6(v6) => v6_in(&ALWAYS_REFUSED_V6, v6),
    };
    let private = match ip {
        IpAddr::V4(v4) => v4_in(&PRIVATE_V4, v4),
        IpAddr::V6(v6) => v6_in(&PRIVATE_V6, v6),
    };
    if reserved {
        Some(Refusal::Reserved)
    } else if private {
        Some(Refusal::Private)
    } else {
        None
    }
}

/// Whether Foundry may connect to `ip` (ADR-EIP-006 decision 6).
pub fn judge(ip: IpAddr, allow: &OutboundAllow) -> Verdict {
    judge_named(None, ip, allow)
}

/// [`judge`] for an address the host name `host` resolved to: a private
/// address is also allowed when the allow-list names that host.
pub fn judge_host(host: &str, ip: IpAddr, allow: &OutboundAllow) -> Verdict {
    judge_named(Some(host), ip, allow)
}

fn judge_named(host: Option<&str>, ip: IpAddr, allow: &OutboundAllow) -> Verdict {
    let ip = judged(ip);
    match class(ip) {
        None => Verdict::Allow,
        Some(Refusal::Private) if allow.admits(host, ip) => Verdict::Allow,
        Some(refusal) => Verdict::Refuse(refusal),
    }
}

/// `FOUNDRY_OUTBOUND_ALLOW`: the private ranges and host names Foundry may
/// reach. Empty when the setting is unset. It never admits a reserved address.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutboundAllow {
    ranges: Vec<IpNet>,
    hosts: BTreeSet<String>,
}

/// A `FOUNDRY_OUTBOUND_ALLOW` entry that is neither a range nor a host name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedAllowEntry {
    /// Its position in the list, counting from 1 (never the entry itself).
    pub position: usize,
}

impl std::fmt::Display for MalformedAllowEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{OUTBOUND_ALLOW_SETTING} entry {} is neither a CIDR range nor a host name",
            self.position
        )
    }
}

impl std::error::Error for MalformedAllowEntry {}

impl OutboundAllow {
    /// Parse a comma-separated list of CIDR ranges and host names; a bare
    /// address is a one-address range. Blank entries are skipped.
    pub fn parse(raw: &str) -> Result<Self, MalformedAllowEntry> {
        let mut allow = Self::default();
        for (index, entry) in raw.split(',').map(str::trim).enumerate() {
            if entry.is_empty() {
                continue;
            }
            if let Ok(net) = entry.parse::<IpNet>() {
                allow.ranges.push(net);
            } else if let Ok(ip) = entry.parse::<IpAddr>() {
                allow.ranges.push(IpNet::from(ip));
            } else if let Some(host) = host_name(entry) {
                allow.hosts.insert(host);
            } else {
                return Err(MalformedAllowEntry {
                    position: index + 1,
                });
            }
        }
        Ok(allow)
    }

    fn admits(&self, host: Option<&str>, ip: IpAddr) -> bool {
        self.ranges.iter().any(|net| net.contains(&ip))
            || host
                .and_then(host_name)
                .is_some_and(|host| self.hosts.contains(&host))
    }
}

/// `entry` as a lower-case host name, if it is one: dot-separated labels of
/// letters, digits and hyphens, not all digits (that would be an address).
fn host_name(entry: &str) -> Option<String> {
    let name = entry.trim_end_matches('.').to_ascii_lowercase();
    let labels_ok = name.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    });
    let numeric = name.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    (labels_ok && !numeric).then_some(name)
}

/// The host of an outbound address: an IP address, or a name to resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetHost {
    Ip(IpAddr),
    Name(String),
}

/// Where an outbound address points: its host and port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundTarget {
    pub host: TargetHost,
    pub port: u16,
}

/// Why an outbound address cannot be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetError {
    /// Not `http` or `https`.
    Scheme,
    /// No host, or a host that is neither an address nor a name.
    Host,
    Port,
}

impl OutboundTarget {
    /// Read `url`'s host and port; a port-less address takes its scheme's
    /// port (443 for `https`, 80 for `http`) before it is judged.
    pub fn parse(url: &str) -> Result<Self, TargetError> {
        let (scheme, rest) = url.split_once("://").ok_or(TargetError::Scheme)?;
        let default_port = match scheme.to_ascii_lowercase().as_str() {
            "https" => 443,
            "http" => 80,
            _ => return Err(TargetError::Scheme),
        };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);
        let (host, port) = split_port(authority)?;
        let port = match port {
            None | Some("") => default_port,
            Some(digits) => digits.parse().map_err(|_| TargetError::Port)?,
        };
        Ok(Self {
            host: TargetHost::parse(host)?,
            port,
        })
    }

    /// The judge's verdict on an address target; `None` for a name, which is
    /// judged once resolved ([`judge_host`]).
    pub fn verdict(&self, allow: &OutboundAllow) -> Option<Verdict> {
        match &self.host {
            TargetHost::Ip(ip) => Some(judge(*ip, allow)),
            TargetHost::Name(_) => None,
        }
    }
}

fn split_port(authority: &str) -> Result<(&str, Option<&str>), TargetError> {
    if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, after) = bracketed.split_once(']').ok_or(TargetError::Host)?;
        return match after {
            "" => Ok((host, None)),
            _ => after
                .strip_prefix(':')
                .map(|port| (host, Some(port)))
                .ok_or(TargetError::Port),
        };
    }
    Ok(match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    })
}

impl TargetHost {
    /// Read a URL host: an IPv6 address, an IPv4 address in any form a
    /// browser accepts (`2130706433`, `0x7f.1`, `0177.0.0.1`, `127.1`), or a
    /// lower-case name.
    pub fn parse(host: &str) -> Result<Self, TargetError> {
        let host = host.trim_start_matches('[').trim_end_matches(']');
        if host.is_empty() {
            return Err(TargetError::Host);
        }
        if let Ok(v6) = host.parse::<Ipv6Addr>() {
            return Ok(Self::Ip(IpAddr::V6(v6)));
        }
        if let Some(v4) = ipv4_literal(host)? {
            return Ok(Self::Ip(IpAddr::V4(v4)));
        }
        host_name(host).map(Self::Name).ok_or(TargetError::Host)
    }
}

/// The WHATWG IPv4 parser: `Ok(None)` when the host's last label is not a
/// number (a name), an error when it is a number but not an address.
fn ipv4_literal(host: &str) -> Result<Option<Ipv4Addr>, TargetError> {
    let trimmed = host.strip_suffix('.').unwrap_or(host);
    let parts: Vec<&str> = trimmed.split('.').collect();
    let last = parts.last().copied().unwrap_or_default();
    if number(last).is_none() {
        return Ok(None);
    }
    let numbers: Option<Vec<u64>> = parts.iter().map(|part| number(part)).collect();
    let numbers = numbers.filter(|n| n.len() <= 4).ok_or(TargetError::Host)?;
    let (head, tail) = numbers.split_at(numbers.len() - 1);
    if head.iter().any(|&n| n > 255) {
        return Err(TargetError::Host);
    }
    let width = 8 * (5 - numbers.len() as u32);
    if tail[0] >= 1u64 << width {
        return Err(TargetError::Host);
    }
    let value = head
        .iter()
        .enumerate()
        .fold(tail[0], |acc, (i, &n)| acc | (n << (8 * (3 - i as u32))));
    Ok(u32::try_from(value).ok().map(Ipv4Addr::from))
}

/// One IPv4 part: decimal, `0x` hex or leading-`0` octal.
fn number(part: &str) -> Option<u64> {
    if part.is_empty() {
        return None;
    }
    let (digits, radix) = match part.strip_prefix("0x").or(part.strip_prefix("0X")) {
        Some(hex) => (hex, 16),
        None if part.len() > 1 && part.starts_with('0') => (&part[1..], 8),
        None => (part, 10),
    };
    if digits.is_empty() {
        return Some(0);
    }
    u64::from_str_radix(digits, radix).ok()
}
