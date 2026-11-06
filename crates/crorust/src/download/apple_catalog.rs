//! Apple Public Software Update Catalog resolver.
//! Queries swscan.apple.com without authentication to locate official
//! Command Line Tools packages directly on Apple's CDN.

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

pub const APPLE_SUCATALOG_URLS: &[&str] = &[
    "https://swscan.apple.com/content/catalogs/others/index-15.merged-1.sucatalog",
    "https://swscan.apple.com/content/catalogs/others/index-10.16.merged-1.sucatalog",
    "https://swscan.apple.com/content/catalogs/others/index-10.15.merged-1.sucatalog",
];

pub const VERIFIED_FALLBACK_APPLE_SDK_URL: &str =
    "https://swcdn.apple.com/content/downloads/52/01/082-41241-A_0747ZN8FHV/dectd075r63pppkkzsb75qk61s0lfee22j/CLTools_macOSNMOS_SDK.pkg";

pub const VERIFIED_FALLBACK_APPLE_SDK_SHA256: &str =
    "ba3453d62b3d2babf67f3a4a44e8073d6555c85f114856f4390a1f53bd76e24a";

pub const MINIMUM_MACOS_SDK_PKG_SIZE_BYTES: u64 = 20_000_000; // 20 MB

#[derive(Debug, Deserialize)]
pub struct SucatalogRoot {
    #[serde(rename = "Products")]
    pub products: HashMap<String, SucatalogProduct>,
}

#[derive(Debug, Deserialize)]
pub struct SucatalogProduct {
    #[serde(rename = "Packages")]
    pub packages: Option<Vec<SucatalogPackage>>,
    #[serde(rename = "ServerMetadataURL")]
    pub server_metadata_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SucatalogPackage {
    #[serde(rename = "URL")]
    pub url: String,
    #[serde(rename = "Size")]
    pub size: Option<u64>,
    #[serde(rename = "Digest")]
    pub digest: Option<String>,
}

pub struct AppleCdnResolver;

impl AppleCdnResolver {
    /// Validates whether a candidate package URL represents a genuine, complete macOS SDK package
    /// and not a removal stub, clean-up script, or empty receipt metapackage.
    pub fn is_genuine_sdk_package(url: &str, size: Option<u64>) -> bool {
        // Enforce minimum size if known: real macOS SDK packages are 50MB - 65MB
        if let Some(sz) = size {
            if sz < MINIMUM_MACOS_SDK_PKG_SIZE_BYTES {
                return false;
            }
        }

        let raw_filename = match url.rsplit_once('/') {
            Some((_, f)) => f,
            None => url,
        };

        // Strip any URL query parameters if present
        let filename = raw_filename.split('?').next().unwrap_or(raw_filename);

        // Reject removal and cleanup scripts (e.g. CLTools_macOS_DevSDK_Remove_macOS14.pkg)
        if filename.contains("Remove") {
            return false;
        }

        // Reject the 5KB stub receipt package
        if filename == "CLTools_macOS_SDK.pkg" {
            return false;
        }

        // Must end with .pkg
        if !filename.ends_with(".pkg") {
            return false;
        }

        // Legitimate Apple CLT SDK packages contain SDK marker
        filename.ends_with("_SDK.pkg") || filename.contains("_SDK_")
    }

    /// Evaluates a candidate package priority score. Higher score wins.
    /// Prefers newer OS versions (NMOS > LMOS, macOS 16 > 15 > 14 > 10.15) and larger packages.
    pub fn score_candidate_package(url: &str, size: u64) -> u64 {
        let raw_filename = url.rsplit_once('/').map(|(_, f)| f).unwrap_or(url);
        let filename = raw_filename.split('?').next().unwrap_or(raw_filename);

        // Tier 1: Apple train codenames
        // NMOS = "Next/Newer macOS" (modern primary SDK in CLT train)
        // LMOS = "Legacy/Lower macOS" (backwards-compatible secondary SDK)
        // Both NMOS and LMOS take precedence over older standalone numeric packages.
        let base_score: u64 = if filename.contains("NMOS") {
            3_000_000_000
        } else if filename.contains("LMOS") {
            2_000_000_000
        } else {
            Self::extract_numeric_version_score(filename).unwrap_or(500_000_000)
        };

        // Add size as tie-breaker (larger package contains more complete framework stubs)
        base_score + size
    }

    /// Extracts structured major/minor version numbers from Apple CLT package filenames.
    /// Handles:
    /// - CLTools_macOS1015_SDK.pkg -> macOS 10.15 -> 1_015_000_000
    /// - CLTools_macOS140_SDK.pkg  -> macOS 14.0  -> 1_400_000_000
    /// - CLTools_macOS150_SDK.pkg  -> macOS 15.0  -> 1_500_000_000
    /// - CLTools_macOS160_SDK.pkg  -> macOS 16.0  -> 1_600_000_000
    pub fn extract_numeric_version_score(filename: &str) -> Option<u64> {
        let prefix = "CLTools_macOS";
        let rest = filename.strip_prefix(prefix)?;
        let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if num_str.is_empty() {
            return None;
        }

        if let Ok(num) = num_str.parse::<u64>() {
            // Distinguish macOS 10.x (e.g. 1015 for Catalina, 1014 for Mojave) from macOS 11+
            if (1000..1100).contains(&num) {
                // 1015 -> 1_015_000_000
                Some(num * 1_000_000)
            } else if (100..1000).contains(&num) {
                // 140 -> 14.0 -> 1_400_000_000, 150 -> 15.0 -> 1_500_000_000, 160 -> 16.0 -> 1_600_000_000
                Some(num * 10_000_000)
            } else if (11..100).contains(&num) {
                // 14 -> 1_400_000_000, 15 -> 1_500_000_000, 16 -> 1_600_000_000
                Some(num * 100_000_000)
            } else {
                Some(num * 1_000_000)
            }
        } else {
            None
        }
    }

    /// Resolves the latest macOS SDK package URL from Apple's public catalog.
    /// Exits early upon finding candidates in the latest catalog, sorting deterministically.
    pub fn resolve_clt_sdk_package_url() -> Result<String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        for catalog_url in APPLE_SUCATALOG_URLS {
            let mut candidates: Vec<(u64, String)> = Vec::new();

            if let Ok(resp) = client.get(*catalog_url).send() {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes() {
                        if let Ok(catalog) = plist::from_bytes::<SucatalogRoot>(&bytes) {
                            for (_prod_id, prod) in catalog.products {
                                if let Some(packages) = prod.packages {
                                    for pkg in packages {
                                        if Self::is_genuine_sdk_package(&pkg.url, pkg.size) {
                                            let sz = pkg
                                                .size
                                                .unwrap_or(MINIMUM_MACOS_SDK_PKG_SIZE_BYTES);
                                            let score = Self::score_candidate_package(&pkg.url, sz);
                                            candidates.push((score, pkg.url));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // If this catalog yielded candidates, sort deterministically (score descending, URL ascending)
            if !candidates.is_empty() {
                candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
                if let Some((_, best_url)) = candidates.into_iter().next() {
                    return Ok(best_url);
                }
            }
        }

        // Return verified official Apple CDN direct package link
        Ok(VERIFIED_FALLBACK_APPLE_SDK_URL.to_string())
    }
}
