use crorust::download::apple_catalog::{
    AppleCdnResolver, MINIMUM_MACOS_SDK_PKG_SIZE_BYTES, VERIFIED_FALLBACK_APPLE_SDK_SHA256,
    VERIFIED_FALLBACK_APPLE_SDK_URL,
};

#[test]
fn test_is_genuine_sdk_package_filters_stub_packages() {
    // Stubs that broke CI
    assert!(!AppleCdnResolver::is_genuine_sdk_package(
        "https://swcdn.apple.com/downloads/CLTools_macOS_DevSDK_Remove_macOS14.pkg",
        Some(5385)
    ));
    assert!(!AppleCdnResolver::is_genuine_sdk_package(
        "https://swcdn.apple.com/downloads/CLTools_macOS_SDK.pkg",
        Some(5366)
    ));
    assert!(!AppleCdnResolver::is_genuine_sdk_package(
        "https://swcdn.apple.com/downloads/CLTools_macOS_DevSDK_Remove_macOS13.pkg",
        Some(5385)
    ));

    // Packages below 20MB are rejected
    assert!(!AppleCdnResolver::is_genuine_sdk_package(
        "https://swcdn.apple.com/downloads/CLTools_macOSNMOS_SDK.pkg",
        Some(10_000_000)
    ));

    // Genuine packages are accepted
    assert!(AppleCdnResolver::is_genuine_sdk_package(
        "https://swcdn.apple.com/downloads/CLTools_macOSNMOS_SDK.pkg",
        Some(58_513_026)
    ));
    assert!(AppleCdnResolver::is_genuine_sdk_package(
        "https://swcdn.apple.com/downloads/CLTools_macOSLMOS_SDK.pkg",
        Some(59_460_733)
    ));
}

#[test]
fn test_score_candidate_package_prefers_nmos_over_lmos() {
    let score_nmos = AppleCdnResolver::score_candidate_package(
        "https://swcdn.apple.com/downloads/CLTools_macOSNMOS_SDK.pkg",
        58_513_026,
    );
    let score_lmos = AppleCdnResolver::score_candidate_package(
        "https://swcdn.apple.com/downloads/CLTools_macOSLMOS_SDK.pkg",
        59_460_733,
    );

    assert!(
        score_nmos > score_lmos,
        "NMOS (Newer/Next macOS train) must score higher than LMOS (Legacy/Lower macOS train)"
    );
}

#[test]
fn test_catalina_does_not_collide_with_macos15() {
    let score_catalina = AppleCdnResolver::score_candidate_package(
        "https://swcdn.apple.com/downloads/CLTools_macOS1015_SDK.pkg",
        65_000_000,
    );
    let score_sonoma = AppleCdnResolver::score_candidate_package(
        "https://swcdn.apple.com/downloads/CLTools_macOS140_SDK.pkg",
        58_000_000,
    );
    let score_sequoia = AppleCdnResolver::score_candidate_package(
        "https://swcdn.apple.com/downloads/CLTools_macOS150_SDK.pkg",
        59_000_000,
    );

    assert!(
        score_sequoia > score_sonoma,
        "macOS 15 must score higher than macOS 14"
    );
    assert!(
        score_sonoma > score_catalina,
        "macOS 14 must score higher than macOS 10.15 Catalina"
    );
}

#[test]
fn test_future_macos_version_scored_correctly() {
    let score_macos16 = AppleCdnResolver::score_candidate_package(
        "https://swcdn.apple.com/downloads/CLTools_macOS160_SDK.pkg",
        60_000_000,
    );
    let score_macos15 = AppleCdnResolver::score_candidate_package(
        "https://swcdn.apple.com/downloads/CLTools_macOS150_SDK.pkg",
        59_000_000,
    );

    assert!(
        score_macos16 > score_macos15,
        "Future macOS 16 must score higher than macOS 15"
    );
}

#[test]
fn test_lmos_scores_higher_than_catalina() {
    let score_lmos = AppleCdnResolver::score_candidate_package(
        "https://swcdn.apple.com/downloads/CLTools_macOSLMOS_SDK.pkg",
        59_460_733,
    );
    let score_catalina = AppleCdnResolver::score_candidate_package(
        "https://swcdn.apple.com/downloads/CLTools_macOS1015_SDK.pkg",
        65_000_000,
    );

    assert!(
        score_lmos > score_catalina,
        "Modern LMOS (macOS 14/15 train) must score higher than macOS 10.15 Catalina"
    );
}

#[test]
fn test_package_url_with_query_parameters_handled_correctly() {
    let url_with_query =
        "https://swcdn.apple.com/downloads/CLTools_macOSNMOS_SDK.pkg?download_token=abc123xyz";
    assert!(
        AppleCdnResolver::is_genuine_sdk_package(url_with_query, Some(58_513_026)),
        "URL containing query parameters must be identified as genuine SDK package"
    );

    let score = AppleCdnResolver::score_candidate_package(url_with_query, 58_513_026);
    assert!(
        score >= 3_000_000_000,
        "NMOS package with query parameters must still receive Tier 1 NMOS score"
    );
}

#[test]
fn test_fallback_url_constants_are_valid() {
    assert!(
        VERIFIED_FALLBACK_APPLE_SDK_URL.starts_with("https://swcdn.apple.com/"),
        "Fallback must use official Apple CDN domain"
    );
    assert_eq!(
        VERIFIED_FALLBACK_APPLE_SDK_SHA256.len(),
        64,
        "SHA256 hash must be 64 hexadecimal characters"
    );
    assert!(
        AppleCdnResolver::is_genuine_sdk_package(
            VERIFIED_FALLBACK_APPLE_SDK_URL,
            Some(MINIMUM_MACOS_SDK_PKG_SIZE_BYTES)
        ),
        "Fallback Apple SDK package URL must satisfy genuine package criteria"
    );
}
