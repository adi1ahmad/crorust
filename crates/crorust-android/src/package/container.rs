use crate::manifest::manifest_dom::AndroidXmlManifestDescriptor;
use crate::package::authenticity::sign_apk_file;
use crate::package::boundary_align::{ANDROID_DEX_ALIGNMENT, ANDROID_SO_ALIGNMENT};
use crate::resources::compile_resources;
use crate::resources::ResourceSymbols;
use anyhow::{Context, Result};
use crorust_common::boundary_archive::{AlignedArchiveSink, EntryCompressionStrategy};
use crorust_common::identity_keypair::IdentityKeypair;
use std::io::Cursor;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeLibraryArtifact {
    pub abi: String,
    pub filename: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct AndroidPackageSpec<'a> {
    pub manifest: &'a AndroidXmlManifestDescriptor,
    pub bytecode_dex: &'a [u8],
    pub native_libraries: &'a [NativeLibraryArtifact],
    pub assets_directory: Option<&'a Path>,
    pub resource_catalog: Option<&'a ResourceSymbols>,
    pub signing_identity: &'a IdentityKeypair,
    pub output_path: &'a Path,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageOutputDescriptor {
    pub path: PathBuf,
    pub sha256: String,
    pub file_size_bytes: u64,
}

pub fn assemble_android_package(spec: &AndroidPackageSpec) -> Result<PackageOutputDescriptor> {
    let mut table = crate::resources::ResourceCatalogTable::default();
    let compiled_arsc = if let Some(catalog) = spec.resource_catalog {
        let pkg_name = spec
            .manifest
            .package
            .as_deref()
            .unwrap_or("com.example.app");
        let chunk = compile_resources(pkg_name, None, None, Some(catalog))
            .context("failed to compile resources.arsc catalog")?;
        table.import_chunk(&chunk);
        let mut arsc_bytes = Vec::new();
        chunk.write(&mut Cursor::new(&mut arsc_bytes))?;
        Some(arsc_bytes)
    } else {
        None
    };

    let xml = quick_xml::se::to_string(spec.manifest)?;
    let manifest_chunk =
        crate::manifest::compile_xml_with_symbols(&xml, &table, spec.resource_catalog)
            .context("failed to compile AndroidManifest.xml into binary AXML")?;
    let mut manifest_bytes = Vec::new();
    manifest_chunk.write(&mut manifest_bytes)?;

    let intermediate_path = spec.output_path.with_extension("unsealed.tmp");
    {
        let mut sink =
            AlignedArchiveSink::create_target(&intermediate_path).with_context(|| {
                format!(
                    "failed to initialize archive sink at {}",
                    intermediate_path.display()
                )
            })?;

        sink.write_entry(
            "AndroidManifest.xml",
            &manifest_bytes,
            EntryCompressionStrategy::CompressedDeflate,
        )?;

        if let Some(arsc_bytes) = &compiled_arsc {
            sink.write_entry(
                "resources.arsc",
                arsc_bytes,
                EntryCompressionStrategy::CompressedDeflate,
            )?;
        }

        if !spec.bytecode_dex.is_empty() {
            sink.write_entry(
                "classes.dex",
                spec.bytecode_dex,
                EntryCompressionStrategy::StoredBoundaryAligned(ANDROID_DEX_ALIGNMENT),
            )?;
        }

        for lib in spec.native_libraries {
            let entry_name = format!("lib/{}/{}", lib.abi, lib.filename);
            sink.write_entry(
                &entry_name,
                &lib.data,
                EntryCompressionStrategy::StoredBoundaryAligned(ANDROID_SO_ALIGNMENT),
            )?;
        }

        if let Some(assets_dir) = spec.assets_directory {
            if assets_dir.is_dir() {
                sink.ingest_directory_recursive("assets", assets_dir)?;
            }
        }

        sink.finalize()?;
    }

    let sealed_path = spec.output_path;
    if sealed_path.exists() {
        let _ = std::fs::remove_file(sealed_path);
    }
    crorust_common::atomic_replace_file(&intermediate_path, sealed_path)
        .context("failed to move unsealed archive to output destination")?;

    sign_apk_file(sealed_path, Some(spec.signing_identity))
        .context("failed to sign APK with APK Signature Scheme v2")?;

    let (sha256_hex, file_size_bytes) = crorust_common::hash_file_streaming(sealed_path)
        .with_context(|| {
            format!(
                "failed to compute streaming digest of sealed package at {}",
                sealed_path.display()
            )
        })?;

    Ok(PackageOutputDescriptor {
        path: sealed_path.to_path_buf(),
        sha256: sha256_hex,
        file_size_bytes,
    })
}
