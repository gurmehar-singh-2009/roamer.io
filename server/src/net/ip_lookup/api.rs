/* src/api.rs */

use crate::net::ip_lookup::{
    lookup::{lookup_with_apipcc, lookup_with_ipapiis, lookup_with_ipinfo, lookup_with_ipsb},
    LookupResult,
};

#[derive(Debug, Copy, Clone)]
/// IP lookup services that can be queried.
pub enum LookupProvider {
    // IpApi,
    /// ipinfo.io
    IpInfo,
    /// ip.sb
    IpSb,
    // IpApiIo,
    /// apip.cc
    ApipCc,
    /// ipapi.is
    IpapiIs,
    // Geolocated,
    // IpLocationApi,
}

/// Looks up an IP with one provider. Returns `None` if the request fails.
pub fn lookup(ip: &str, provider: LookupProvider) -> Option<LookupResult> {
    match provider {
        // LookupProvider::IpApi => lookup_with_ipapi(ip),
        LookupProvider::IpInfo => lookup_with_ipinfo(ip),
        LookupProvider::IpSb => lookup_with_ipsb(ip),
        // LookupProvider::IpApiIo => lookup_with_ipapiio(ip),
        LookupProvider::ApipCc => lookup_with_apipcc(ip),
        LookupProvider::IpapiIs => lookup_with_ipapiis(ip),
        // LookupProvider::Geolocated => lookup_with_geolocated(ip),
        // LookupProvider::IpLocationApi => lookup_with_iplocationapi(ip),
    }
}

impl LookupProvider {
    /// Every provider.
    pub const fn all() -> &'static [Self] {
        &[
            // LookupProvider::IpApi,
            Self::IpInfo,
            Self::IpSb,
            // LookupProvider::IpApiIo,
            Self::ApipCc,
            Self::IpapiIs,
            // LookupProvider::Geolocated,
            // LookupProvider::IpLocationApi,
        ]
    }
}
