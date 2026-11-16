//! Numbers come from docs/CONSTANTS.md through build.rs; nothing numeric is typed here.
include!(concat!(env!("OUT_DIR"), "/constants.rs"));

/// The provider database of the Mozilla Autoconfig format (step a).
pub const ISPDB_BASE: &str = "https://autoconfig.thunderbird.net/v1.1/";
/// Named in the User-Agent so a server operator can find who is asking.
pub const PRODUCT_URL: &str = "https://whoosh.app";
/// Microsoft's shared Autodiscover front for Microsoft 365 tenants (step d, second request).
pub const MS_AUTODISCOVER_V2: &str = "https://autodiscover-s.outlook.com/autodiscover/autodiscover.json";
/// Exchange Online's documented IMAP and SMTP hosts, used when Autodiscover marks the domain a Microsoft 365 tenant.
pub const M365_IMAP_HOST: &str = "outlook.office365.com";
pub const M365_SMTP_HOST: &str = "smtp.office365.com";
