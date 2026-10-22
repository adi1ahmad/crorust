# Crate: `crorust-windows` (MSIX Packaging, Block Maps & Authenticode)

## 1. Overview & Architectural Role

`crorust-windows` implements Windows Modern application packaging and cryptographic signing in pure Rust, removing dependencies on Microsoft Visual Studio, `MakeAppx.exe`, and `SignTool.exe`.

```mermaid
flowchart TB
    %% Styling Classes (Dark Mode & Dual-Theme High Contrast)
    classDef input fill:#1f3354,stroke:#388bfd,stroke-width:1.5px,color:#e6edf3
    classDef assembly fill:#3d2e14,stroke:#d29922,stroke-width:1.5px,color:#e6edf3
    classDef crypto fill:#421d22,stroke:#f85149,stroke-width:1.5px,color:#e6edf3
    classDef out fill:#163824,stroke:#3fb950,stroke-width:1.5px,color:#e6edf3

    subgraph Inputs["Compiler & Configuration Inputs"]
        Exe["my_app.exe (PE/COFF 64-bit)"]
        Icons["PNG Assets & Visual Elements"]
        Man["manifest.toml [windows]"]
        Cert["Code Signing Certificate & RSA Key"]
    end

    subgraph Assembly["MSIX Container (ECMA-376 OPC)"]
        CT["[Content_Types].xml<br/><i>MIME type mappings</i>"]
        Manifest["AppxManifest.xml<br/><i>Package identity & capabilities</i>"]
        PRI["resources.pri<br/><i>Modern Resource Indexer</i>"]
        Block["AppxBlockMap.xml<br/><i>64KB SHA-256 block hashing</i>"]
        CT & Manifest & PRI & Exe --> Block
    end

    subgraph Security["Authenticode Signing"]
        Auth["compute_authenticode_envelope<br/><i>PKCS#7 / CMS Envelope</i>"]
        Block --> Auth
        Cert --> Auth
        Auth --> P7X["AppxSignature.p7x"]
    end

    P7X --> Out[Production .msix Package]

    %% Subgraph Styling
    style Inputs fill:none,stroke:#388bfd,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3
    style Assembly fill:none,stroke:#d29922,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3
    style Security fill:none,stroke:#f85149,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3

    %% Class Assignments
    class Exe,Icons,Man,Cert input
    class CT,Manifest,PRI,Block assembly
    class Auth,P7X crypto
    class Out out
```

---

## 2. Open Packaging Conventions (OPC) Anatomy

An MSIX container is an uncompressed or deflated ZIP archive structured according to ECMA-376 Open Packaging Conventions:

```
MyApp.msix/
├── [Content_Types].xml               # Declares MIME types for all extensions
├── AppxManifest.xml                  # Identity, capabilities, and visual elements
├── AppxBlockMap.xml                  # 64KB block cryptographic digests
├── resources.pri                     # Binary index for localized assets
├── AppxSignature.p7x                 # Uncompressed Authenticode PKCS#7 signature
└── my_app.exe                        # Windows PE/COFF executable
```

### The 64KB Block Map (`AppxBlockMap.xml`)
Windows AppX Deployment Service does not re-download entire multi-gigabyte packages during updates. Instead, it inspects `AppxBlockMap.xml`:
- Every file in the package is partitioned into contiguous 64KB (`65536` byte) blocks.
- Each block is hashed using SHA-256.
- During updates or streaming installs, the client requests only the modified 64KB blocks over HTTP range requests.

---

## 3. Authenticode PKCS#7 Signature Envelope

Windows validates MSIX packages at install time. If `AppxSignature.p7x` is missing or invalid, installation aborts with `TRUST_E_NOSIGNATURE` (`0x800B0100`).

`crorust-windows` builds the Authenticode envelope in pure Rust:
1. Computes the SHA-256 digest of `AppxBlockMap.xml`.
2. Computes the SHA-256 digest of `AppxManifest.xml`.
3. Computes the SHA-256 digest of `[Content_Types].xml`.
4. Wraps these digests inside an ASN.1 `SpcIndirectDataContent` structure.
5. Signs the structure using an RSA private key with PKCS#7 / CMS padding.
6. Writes the signature as an uncompressed (`CompressionMethod::Stored`) ZIP entry named `AppxSignature.p7x`.

---

## 4. Development Signing & Certificate Export

For local testing on Windows machines:
1. Export the built-in development certificate:
   ```sh
   crorust cert export-debug --platform windows -o crorust-debug.cer
   ```
2. Import the certificate into the Windows **Trusted People** certificate store (requires admin):
   ```powershell
   Import-Certificate -FilePath crorust-debug.cer -CertStoreLocation Cert:\LocalMachine\TrustedPeople
   ```
3. Packages built with `crorust build --platform windows` can now be double-clicked and installed without warnings.
