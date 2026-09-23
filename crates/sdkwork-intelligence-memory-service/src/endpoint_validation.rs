//! Outbound URL validation for webhook and provider integrations.
//!
//! Prevents SSRF against private networks, loopback, and cloud metadata endpoints.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use crate::platform;

/// Validates URL syntax and literal hosts without performing DNS resolution.
pub fn validate_outbound_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|error| format!("invalid URL: {error}"))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("URL credentials are not allowed".to_string());
    }

    match parsed.scheme() {
        "https" => {}
        "http" if !platform::is_production_like_environment() => {}
        "http" => return Err("HTTP scheme is not allowed in production; use HTTPS".to_string()),
        other => return Err(format!("unsupported URL scheme: {other}")),
    }

    let host = parsed
        .host()
        .ok_or_else(|| "URL must have a host".to_string())?;
    if let Some(ip) = host_ip(&host) {
        validate_public_ip(ip)?;
    }
    let host_name = host
        .to_string()
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();
    if host_name == "localhost" || host_name.ends_with(".localhost") {
        return Err("localhost hostnames are not allowed".to_string());
    }
    if matches!(
        host_name.as_str(),
        "metadata.google.internal" | "metadata.aws.internal"
    ) {
        return Err("cloud metadata endpoints are not allowed".to_string());
    }
    Ok(())
}

/// Resolves every address, rejects mixed public/private answers, and pins the
/// validated addresses into the returned no-redirect client.
pub async fn build_pinned_http_client(
    url: &str,
    timeout: Duration,
    pool_max_idle_per_host: usize,
) -> Result<(url::Url, reqwest::Client), String> {
    validate_outbound_url(url)?;
    let parsed = url::Url::parse(url).map_err(|error| format!("invalid URL: {error}"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "URL must have a host".to_string())?
        .trim_matches(['[', ']'])
        .to_string();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| "URL scheme does not define a port".to_string())?;
    let addresses = resolve_public_addresses(&host, port).await?;
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .pool_max_idle_per_host(pool_max_idle_per_host)
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(&host, &addresses)
        .build()
        .map_err(|error| format!("create pinned HTTP client failed: {error}"))?;
    Ok((parsed, client))
}

async fn resolve_public_addresses(host: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    let mut addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|error| format!("DNS resolution failed: {error}"))?
        .collect::<Vec<_>>();
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty() {
        return Err("DNS resolution returned no addresses".to_string());
    }
    for address in &addresses {
        validate_public_ip(address.ip())?;
    }
    Ok(addresses)
}

fn host_ip(host: &url::Host<&str>) -> Option<IpAddr> {
    match host {
        url::Host::Ipv4(ip) => Some(IpAddr::V4(*ip)),
        url::Host::Ipv6(ip) => Some(IpAddr::V6(*ip)),
        url::Host::Domain(_) => None,
    }
}

fn validate_public_ip(ip: IpAddr) -> Result<(), String> {
    let is_public = match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    };
    is_public
        .then_some(())
        .ok_or_else(|| "outbound endpoint resolved to a non-public IP address".to_string())
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || a == 0
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 240)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = ip.segments();
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_localhost_hostname() {
        assert!(validate_outbound_url("https://localhost/hook").is_err());
    }

    #[test]
    fn rejects_private_ip_literal() {
        assert!(validate_outbound_url("https://10.0.0.1/hook").is_err());
        assert!(validate_outbound_url("https://[fc00::1]/hook").is_err());
        assert!(validate_outbound_url("https://[fe80::1]/hook").is_err());
        assert!(validate_outbound_url("https://[::ffff:127.0.0.1]/hook").is_err());
        assert!(validate_outbound_url("https://192.0.2.10/hook").is_err());
    }

    #[test]
    fn rejects_embedded_credentials() {
        assert!(validate_outbound_url("https://user:secret@example.com/hook").is_err());
    }

    #[test]
    fn accepts_public_https_endpoint() {
        assert!(validate_outbound_url("https://hooks.example.com/memory").is_ok());
    }
}
