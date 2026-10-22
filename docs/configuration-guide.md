# Configuration Reference Manual (`manifest.toml`)

## 1. Overview & Location

`crorust` application configuration is declared in `manifest.toml` at the project or workspace root (or inside `[package.metadata.crorust]` in `Cargo.toml`).

The schema enforces strict validation (`#[serde(deny_unknown_fields)]`). Any typo, unrecognized key, or invalid data type causes an immediate parse failure with an actionable diagnostic pointing to the exact line in `manifest.toml`.

---

## 2. Global Package Identity

These values override or extend fields declared in `Cargo.toml`:

```toml
# The internal crate/binary name (defaults to Cargo.toml package name)
package = "my_app"

# Human-readable display name shown on device home screens and window titlebars
name = "My Application"

# Primary binary executable target
bin = "my_app"

# Semantic version string (defaults to Cargo.toml version)
version = "1.0.0"

# Monotonically increasing integer version code for app stores (Google Play, iOS App Store)
version_code = 1

# High-resolution application icon (PNG or SVG, recommended >= 512x512)
# Automatically converted to Android mipmaps, Apple ICNS, Windows ICO/PRI, and Linux .DirIcon
icon = "assets/icon.png"
```

---

## 3. Hermetic Toolchain Policies (`[toolchain]`)

Controls how external compilers, sysroots, and SDKs are provisioned and stored:

```toml
[toolchain]
# Cache storage scoping:
# - "project" (default): Stages assets in <project_root>/.cache/crorust/ for strict isolation.
# - "global": Shares assets in ~/.cache/crorust/ (or %LOCALAPPDATA%\crorust\cache\ on Windows).
cache_scope = "project"

# Explicit cache directory override (optional)
# cache_dir = "/path/to/shared/cache"

# Offline mode: strictly fail-fast if any compiler or sysroot is not already cached
offline = false

# Corporate mirror URL rewrite: redirects all upstream downloads to an internal mirror
# mirror_url = "https://artifactory.internal.corp/mirrors"

# Accommodate Nix/NixOS environments (automatically inferred by default)
# nix = false
```

---

## 4. Platform-Specific Tables

### Android Configuration (`[android]`)

```toml
[android]
# Minimum Android API level supported (default: 21, Android 5.0 Lollipop)
min_sdk = 21

# Target Android API level (default: 34, Android 14)
target_sdk = 34

# Compile Android SDK level (default: 35, Android 15)
compile_sdk = 35

# Build pipeline:
# - false (default): Pure-Rust standalone pipeline (AXML, ARSC, Sig v2). 10x faster, zero JVM needed.
# - true: Legacy Gradle wrapper fallback (uses hermetic Gradle 8.7 + Adoptium JDK 17).
use_gradle = false

# Enable Google GameActivity JNI bindings for low-latency graphics applications
game_activity = false

# Java bytecode target version (default: "17")
jvm_target = "17"

# Remote Maven repositories for resolving Android AAR/JAR dependencies
repositories = [
    { standard = "google" },
    { standard = "mavenCentral" },
    # { maven = "https://maven.pkg.github.com/my-org/repo", username_env = "GITHUB_ACTOR", password_env = "GITHUB_TOKEN" }
]

# Maven dependencies downloaded and unpacked via pure-Rust crorust-mvn
dependencies = [
    "androidx.core:core-ktx:1.10.1",
    "androidx.appcompat:appcompat:1.6.1",
]

# Static assets to embed into the APK assets/ directory
assets = [
    "assets",
    # Specific asset with custom alignment (e.g. 4096-byte alignment for fast mmap):
    # { path = "assets/models/scene.glb", dest = "models/scene.glb", alignment = 4096 }
]
```

### Apple iOS Configuration (`[ios]`)

```toml
[ios]
# Minimum iOS version constraint (default: "15.0")
minimum_os_version = "15.0"

# Unique reverse-DNS bundle identifier
bundle_identifier = "com.example.myapp"

# Target device type: false for physical iPhone/iPad hardware; true for Apple Silicon / Intel Simulators
simulator = false

# Path to Apple mobile provisioning profile (.mobileprovision)
# provisioning_profile = "certificates/AppStore.mobileprovision"

# Path to precompiled Apple Asset Catalog (Assets.car)
# assets_car = "assets/Assets.car"
```

### Apple macOS Configuration (`[macos]`)

```toml
[macos]
# Minimum macOS version constraint (default: "11.0" Big Sur)
minimum_system_version = "11.0"

# Unique reverse-DNS bundle identifier
bundle_identifier = "com.example.myapp"

# macOS Application Category (e.g. "public.app-category.developer-tools")
# category = "public.app-category.utilities"

# Path to XML entitlements file for sandbox capabilities
# entitlements = "entitlements.plist"
```

### Linux Desktop Configuration (`[linux]`)

```toml
[linux]
# C runtime library variant:
# - "gnu" (default): Dynamically links against host Glibc.
# - "musl": Compiles against static Musl libc (produces ultra-portable, self-contained binary).
libc = "gnu"

# SquashFS compression algorithm: "gzip" (fastest decompression) or "xz" (smallest file size)
compression = "gzip"

# SquashFS generator backend: "pure-rust" (backhand crate, default) or "system" (mksquashfs)
squashfs_backend = "auto"
```

### Windows Desktop Configuration (`[windows]`)

```toml
[windows]
# Target Windows SDK version (default: "10.0.22621.0")
sdk_version = "10.0.22621.0"

# Publisher Distinguished Name (must match the subject DN of your signing certificate)
publisher = "CN=My Organization, O=My Organization, C=US"

# Package Identity Name used in AppxManifest.xml
identity_name = "MyOrganization.MyApplication"

# Appx capabilities required by the application
capabilities = [
    "internetClient",
    "runFullTrust"
]
```

### WebAssembly Configuration (`[web]`)

```toml
[web]
# Local development HTTP server port (default: 8080)
port = 8080

# Bind address for development server (default: "127.0.0.1")
address = "127.0.0.1"

# Automatically open system default web browser on server start
open = false

# Output directory for compiled WASM, JS glue, and HTML assets
dist = ".cache/web-static/dist"
```
