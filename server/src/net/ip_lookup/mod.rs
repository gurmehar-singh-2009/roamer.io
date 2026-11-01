/// Choosing a lookup provider.
pub mod api;
/// Finding the server's public IP.
pub mod ip;
/// Lookup results and the per-provider requests.
pub mod lookup;

// pub use ip::get_public_ip_addr;
pub use api::{lookup, LookupProvider};
pub use lookup::LookupResult;

/// Normalizes an IP string so the same client always maps to the same key.
/// IPv4-mapped IPv6 addresses (`::ffff:1.2.3.4`) become plain IPv4.
pub fn normalize_ip(ip: &str) -> String {
    ip.parse::<std::net::IpAddr>().map_or_else(
        |_| ip.to_string(),
        |addr| match addr {
            std::net::IpAddr::V6(v6) => v6
                .to_ipv4_mapped()
                .map_or_else(|| addr.to_string(), |v4| v4.to_string()),
            std::net::IpAddr::V4(v4) => v4.to_string(),
        },
    )
}
