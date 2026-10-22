# Third-Party Attribution Notice: xbuild

This product includes software developed by the **xbuild** project and its contributors.

- **Upstream Repository**: [https://github.com/rust-mobile/xbuild](https://github.com/rust-mobile/xbuild)
- **Original Copyright**: Copyright (c) 2022–2025 David Craven, Marijn Suijten, and xbuild contributors
- **Original License**: Dual-licensed under the **Apache License, Version 2.0** OR the **MIT License** at the recipient's option.

---

## Component Origin and Adaptations

Portions of the following member crates and subsystems in `crorust` were originally adapted or derived from `xbuild`:

| `crorust` Workspace Crate | Corresponding `xbuild` Crate / Module | Description of Derived Functionality |
| :--- | :--- | :--- |
| [`crates/crorust`](../../crates/crorust) | `xbuild` | CLI architecture, cargo execution wrappers, platform toolchain runners, device manager. |
| [`crates/crorust-android`](../../crates/crorust-android) | `apk` | APK packaging primitives, Android Binary XML (AXML) compiler, and resource table generation. |
| [`crates/crorust-apple`](../../crates/crorust-apple) | `appbundle` | macOS and iOS `.app` bundle assembly, Info.plist generation, and code signing integration. |
| [`crates/crorust-windows`](../../crates/crorust-windows) | `msix` & `pri` | Windows MSIX packaging, block map computation, AppxSignature signing, and PRI table parsing. |
| [`crates/crorust-linux`](../../crates/crorust-linux) | `appimage` | Linux AppDir staging, desktop entry generation, and AppImage runtime wrapping. |
| [`crates/crorust-mvn`](../../crates/crorust-mvn) | `mvn` | Maven dependency resolution primitives using PubGrub and POM parsing. |
| [`crates/crorust-common`](../../crates/crorust-common) | `xcommon` | Common packaging utilities, cryptographic signing primitives, and LLVM/ELF inspection helpers. |

---

## Applicable Licenses

In compliance with the redistribution requirements of both upstream licenses:
- Full text of the Apache License, Version 2.0: [LICENSE-APACHE](LICENSE-APACHE)
- Full text of the MIT License: [LICENSE-MIT](LICENSE-MIT)
