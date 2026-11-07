<div align="center">
  <h1>crorust-runtime-stub</h1>
  <p><strong>Clean-Room Pure-Rust AppImage Type-2 ELF Runtime Launcher Stub</strong></p>
  <p>
    Part of the <a href="https://crorust.com"><strong>CroRust Cross-Platform Build Engine</strong></a>.
  </p>
</div>

---

## Overview

`crorust-runtime-stub` is an in-tree clean-room implementation of the AppImage Type-2 runtime launcher executable. It is prepended directly to SquashFS payloads by `crorust-linux` to form standalone executable AppImages.

Upon execution, the stub inspects its own binary, locates the SquashFS superblock at the designated offset, mounts or extracts the application payload, sets up environment variables (`APPDIR`, `APPIMAGE`), and executes `AppRun`.

---

## Architectural Documentation

For full cross-compilation guides, sysroot management, and multi-platform orchestration:
- [**CroRust Official Website**](https://crorust.com)
- [**CroRust GitHub Repository**](https://github.com/adi1ahmad/crorust)

---

## License

Licensed under the [GNU Affero General Public License v3.0 (AGPL-3.0-only)](https://github.com/adi1ahmad/crorust/blob/main/LICENSE).
