use super::CaptureError;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const DEFAULT_HTTP_PORT: u16 = 80;
const DEFAULT_HTTPS_PORT: u16 = 443;
const CGNAT_FIRST_OCTET: u8 = 100;
const CGNAT_MIN_SECOND_OCTET: u8 = 64;
const CGNAT_MAX_SECOND_OCTET: u8 = 127;

/// Network access policy enforced by web capture backends (`WebBackend`, `CdpBackend`).
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkPolicy {
    /// Any URL is permitted (suitable for trusted local dev targets).
    #[default]
    Unrestricted,
    /// Only `http://` and `https://` targets resolving exclusively to public IP addresses are
    /// permitted (blocks loopback, RFC1918 private networks, link-local / cloud metadata, and
    /// userinfo credentials on initial navigation, redirects, and subresources).
    PublicOnly,
}

impl NetworkPolicy {
    /// Returns a policy that permits only public HTTP(S) destinations.
    #[must_use]
    pub const fn public_only() -> Self {
        Self::PublicOnly
    }

    /// Whether `ip` is a globally routable public IP address.
    #[must_use]
    pub fn is_public_ip(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => is_public_v4(v4),
            IpAddr::V6(v6) => is_public_v6(v6),
        }
    }

    /// Validates URL syntax and literal IP addresses under `self`.
    ///
    /// Returns `Ok(Some((host, port)))` when DNS resolution is still needed for a hostname,
    /// or `Ok(None)` when no resolution is required.
    ///
    /// # Errors
    ///
    /// Returns an [`super::ErrorClass::Target`] error if the URL violates `self`.
    pub fn check_url_syntax(self, raw: &str) -> Result<Option<(String, u16)>, CaptureError> {
        if self == Self::Unrestricted {
            return Ok(None);
        }
        let url = raw.trim();
        if url.is_empty() || url.chars().any(char::is_control) || url.contains(char::is_whitespace)
        {
            return Err(CaptureError::target(
                "capture URL must be a valid HTTP(S) URL",
            ));
        }
        let (rest, default_port) = if let Some(rest) = url.strip_prefix("http://") {
            (rest, DEFAULT_HTTP_PORT)
        } else if let Some(rest) = url.strip_prefix("https://") {
            (rest, DEFAULT_HTTPS_PORT)
        } else {
            return Err(CaptureError::target(
                "capture URL must use http:// or https://",
            ));
        };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let (host, port) = split_authority(authority, default_port)?;
        let lower = host.trim_end_matches('.').to_ascii_lowercase();
        if lower == "localhost" || lower.ends_with(".localhost") {
            return Err(CaptureError::target(
                "capture target host `localhost` is blocked by NetworkPolicy::PublicOnly",
            ));
        }
        if let Ok(ip) = host.parse::<IpAddr>() {
            if !Self::is_public_ip(ip) {
                return Err(CaptureError::target(format!(
                    "capture target IP `{ip}` is blocked by NetworkPolicy::PublicOnly"
                )));
            }
            return Ok(None);
        }
        Ok(Some((host, port)))
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    let is_cgnat = octets[0] == CGNAT_FIRST_OCTET
        && (CGNAT_MIN_SECOND_OCTET..=CGNAT_MAX_SECOND_OCTET).contains(&octets[1]);
    !(ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || is_cgnat)
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || ip.to_ipv4_mapped().is_some_and(|v4| !is_public_v4(v4)))
}

fn parse_port(port: &str) -> Result<u16, CaptureError> {
    let parsed = port
        .parse::<u16>()
        .map_err(|_| CaptureError::target("invalid port in capture URL"))?;
    if parsed == 0 {
        return Err(CaptureError::target("port 0 is not valid in capture URL"));
    }
    Ok(parsed)
}

fn split_authority(authority: &str, default_port: u16) -> Result<(String, u16), CaptureError> {
    if authority.is_empty() {
        return Err(CaptureError::target("capture URL must include a host"));
    }
    if authority.contains('@') {
        return Err(CaptureError::target(
            "capture URL must not contain userinfo",
        ));
    }
    if authority.contains('\\') {
        return Err(CaptureError::target("capture URL host is invalid"));
    }
    if let Some(rest) = authority.strip_prefix('[') {
        let Some(end) = rest.find(']') else {
            return Err(CaptureError::target("invalid IPv6 host in capture URL"));
        };
        let host = &rest[..end];
        if host.parse::<Ipv6Addr>().is_err() {
            return Err(CaptureError::target("invalid IPv6 address in capture URL"));
        }
        let after = &rest[end + 1..];
        let port = if after.is_empty() {
            default_port
        } else if let Some(port_str) = after.strip_prefix(':') {
            parse_port(port_str)?
        } else {
            return Err(CaptureError::target("invalid port in capture URL"));
        };
        return Ok((host.to_owned(), port));
    }
    if authority.contains('[') || authority.contains(']') {
        return Err(CaptureError::target("invalid host brackets in capture URL"));
    }
    if let Some((host, port_str)) = authority.split_once(':') {
        if host.is_empty() || port_str.contains(':') {
            return Err(CaptureError::target("invalid host or port in capture URL"));
        }
        return Ok((host.to_owned(), parse_port(port_str)?));
    }
    Ok((authority.to_owned(), default_port))
}
