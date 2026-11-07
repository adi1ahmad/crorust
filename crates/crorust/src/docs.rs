//! # Comprehensive Architectural & Operational Guide Suite
//!
//! This module embeds the complete CroRust technical manual directly into Rustdoc,
//! providing a searchable offline guide on `docs.rs` and local documentation builds.
//!
//! Official Website & Resources: [crorust.com](https://crorust.com)

#![allow(clippy::needless_doctest_main)]

/// System Overview & Multi-Host Design Philosophy
pub mod overview {
    #![doc = include_str!("../docs/overview.md")]
}

/// Hermetic Toolchains, SDK Downloader & Sysroot Resolution
pub mod toolchains {
    #![doc = include_str!("../docs/toolchains-and-sysroots.md")]
}

/// Pure-Rust Packaging Internals (AXML, ARSC, OPC, AppImage)
pub mod packaging {
    #![doc = include_str!("../docs/packaging-internals.md")]
}

/// Target Device Discovery, Bridge Drivers & Live Debugging
pub mod devices {
    #![doc = include_str!("../docs/devices-and-debugging.md")]
}

/// manifest.toml Complete Configuration Specification
pub mod configuration {
    #![doc = include_str!("../docs/configuration-guide.md")]
}

/// Project Templates & Scaffolding Engine
pub mod templates {
    #![doc = include_str!("../docs/template-guide.md")]
}

/// Cache Scoping & Continuous Integration Pipelines
pub mod cache_ci {
    #![doc = include_str!("../docs/cache-and-ci.md")]
}
