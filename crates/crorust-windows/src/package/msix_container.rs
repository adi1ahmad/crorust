use super::block_map::MsixBlockMapBuilder;
use super::manifest::WindowsPackageManifestDocument;
use super::opc::OpcContentTypesBuilder;
use crate::mrm::MrmPriPackageBuilder;
use crate::security::{compute_authenticode_envelope, AuthenticodeDigests};
use anyhow::{Context, Result};
use crorust_common::identity_keypair::DEFAULT_DEBUG_PEM;
use crorust_common::{
    ArchiveEntryOptions, BoundaryZipArchive, CentralDirectoryLocator, IdentityKeypair,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

pub const DEBUG_PEM: &str = DEFAULT_DEBUG_PEM;
pub const EMBEDDED_DEBUG_CERTIFICATE_PEM: &str = DEBUG_PEM;

const OPC_CONTENT_TYPES_PART: &str = "[Content_Types].xml";
const PACKAGE_MANIFEST_PART: &str = "AppxManifest.xml";
const PACKAGE_BLOCK_MAP_PART: &str = "AppxBlockMap.xml";
const PACKAGE_SIGNATURE_PART: &str = "AppxSignature.p7x";

struct VisualAssetSpec {
    slot_name: &'static str,
    logical_width: u32,
    logical_height: u32,
    inset_padding: f32,
}

const WINDOWS_APP_ICONS: &[VisualAssetSpec] = &[
    VisualAssetSpec {
        slot_name: "SmallTile",
        logical_width: 71,
        logical_height: 71,
        inset_padding: 0.34,
    },
    VisualAssetSpec {
        slot_name: "Square150x150Logo",
        logical_width: 150,
        logical_height: 150,
        inset_padding: 0.34,
    },
    VisualAssetSpec {
        slot_name: "Wide310x150Logo",
        logical_width: 310,
        logical_height: 150,
        inset_padding: 0.34,
    },
    VisualAssetSpec {
        slot_name: "LargeTile",
        logical_width: 310,
        logical_height: 310,
        inset_padding: 0.0,
    },
    VisualAssetSpec {
        slot_name: "Square44x44Logo",
        logical_width: 44,
        logical_height: 44,
        inset_padding: 0.0,
    },
    VisualAssetSpec {
        slot_name: "SplashScreen",
        logical_width: 620,
        logical_height: 300,
        inset_padding: 0.34,
    },
    VisualAssetSpec {
        slot_name: "BadgeLogo",
        logical_width: 24,
        logical_height: 24,
        inset_padding: 0.0,
    },
    VisualAssetSpec {
        slot_name: "StoreLogo",
        logical_width: 50,
        logical_height: 50,
        inset_padding: 0.0,
    },
];

type ScaledIconEntry = (u32, String);
type IconVariantGroup = (String, String, Vec<ScaledIconEntry>);

pub struct WindowsPackageContainer {
    manifest: WindowsPackageManifestDocument,
    path: PathBuf,
    zip: Option<BoundaryZipArchive>,
    content_types: OpcContentTypesBuilder,
    block_map: MsixBlockMapBuilder,
    icon_variants: Vec<IconVariantGroup>,
    has_pri: bool,
    compress: bool,
    icon_cache_dir: Option<PathBuf>,
}

impl WindowsPackageContainer {
    pub fn new(
        path: PathBuf,
        manifest: WindowsPackageManifestDocument,
        compress: bool,
    ) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .with_context(|| format!("Failed to create MSIX file `{}`", path.display()))?;

        Ok(Self {
            manifest,
            zip: Some(BoundaryZipArchive::from_file(file, compress)),
            content_types: OpcContentTypesBuilder::default(),
            block_map: MsixBlockMapBuilder::new(),
            icon_variants: Vec::new(),
            has_pri: false,
            path,
            compress,
            icon_cache_dir: None,
        })
    }

    pub fn set_icon_cache_dir(&mut self, dir: impl Into<PathBuf>) {
        self.icon_cache_dir = Some(dir.into());
    }

    pub fn package_path(&self) -> &Path {
        &self.path
    }

    pub fn embed_application_icon(&mut self, path: &Path) -> Result<()> {
        let mut scaler = crorust_common::RasterResizer::open(path)
            .with_context(|| format!("Failed to read icon file at `{}`", path.display()))?;

        if let Some(cache_dir) = &self.icon_cache_dir {
            scaler.set_cache_dir(Some(cache_dir.clone()));
        }

        let mut jobs = Vec::with_capacity(WINDOWS_APP_ICONS.len() * 5);
        let mut constraints = Vec::with_capacity(WINDOWS_APP_ICONS.len() * 5);

        for preset in WINDOWS_APP_ICONS {
            for scale in [1.0f32, 1.25, 1.5, 2.0, 4.0] {
                let scale_int = (scale * 100.0) as u32;
                let target_w = (preset.logical_width as f32 * scale).round() as u32;
                let target_h = (preset.logical_height as f32 * scale).round() as u32;
                let (scaled_w, scaled_h) = if preset.inset_padding > 0.0 {
                    (
                        ((target_w as f32) * (1.0 - preset.inset_padding)).round() as u32,
                        ((target_h as f32) * (1.0 - preset.inset_padding)).round() as u32,
                    )
                } else {
                    (target_w, target_h)
                };

                let file_name = format!("{}.scale-{scale_int}.png", preset.slot_name);
                let relative_dest = Path::new("Assets").join(&file_name);

                let constraint = crorust_common::TransformConstraint {
                    target_width: target_w,
                    target_height: target_h,
                    scaled_size: scaled_w.min(scaled_h),
                    filter: image::imageops::FilterType::Lanczos3,
                };

                jobs.push((relative_dest, preset.slot_name, scale_int));
                constraints.push(constraint);
            }
        }

        let rendered = scaler.render_batch_parallel(&constraints)?;

        let mut groups: BTreeMap<&'static str, Vec<(u32, String)>> = BTreeMap::new();
        for ((relative_dest, group_name, scale_percent), (_, png_bytes)) in
            jobs.into_iter().zip(rendered)
        {
            self.add_data(&relative_dest, &png_bytes, ArchiveEntryOptions::Compressed)?;
            let win_rel_path = relative_dest.to_string_lossy().replace('/', "\\");
            groups
                .entry(group_name)
                .or_default()
                .push((scale_percent, win_rel_path));
        }

        for (group_name, mut candidates) in groups {
            candidates.sort_by_key(|(s, _)| *s);
            let pri_slot = format!("{group_name}.png");
            self.icon_variants
                .push(("Files/Assets".to_string(), pri_slot, candidates));
        }

        Ok(())
    }

    pub fn add_pri(&mut self, data: &[u8]) -> Result<()> {
        self.add_data(
            Path::new("resources.pri"),
            data,
            ArchiveEntryOptions::Compressed,
        )?;
        self.has_pri = true;
        Ok(())
    }

    fn generate_pri(&mut self) -> Result<()> {
        let mut builder = MrmPriPackageBuilder::new("App");
        for (scope, file_name, candidates) in &self.icon_variants {
            builder.add_asset_scale_variants(scope, file_name, candidates);
        }
        let pri_file = builder.finish()?;
        let mut pri_bytes = Vec::new();
        pri_file.write(&mut pri_bytes)?;
        self.add_pri(&pri_bytes)
    }

    pub fn embed_file(
        &mut self,
        source: &Path,
        dest: &Path,
        opts: ArchiveEntryOptions,
    ) -> Result<()> {
        if dest == Path::new("resources.pri") {
            self.has_pri = true;
        }
        self.content_types.register_path(dest);
        self.block_map.add_file(dest, source)?;
        self.zip
            .as_mut()
            .context("zip is closed")?
            .write_file_entry(source, dest, opts)
    }

    pub fn add_data(&mut self, dest: &Path, data: &[u8], opts: ArchiveEntryOptions) -> Result<()> {
        if dest == Path::new("resources.pri") {
            self.has_pri = true;
        }
        self.content_types.register_path(dest);
        self.block_map.add_data(dest, data);
        self.zip
            .as_mut()
            .context("zip is closed")?
            .create_file(dest, opts, data)
    }

    pub fn embed_directory(
        &mut self,
        source: &Path,
        dest: &Path,
        opts: ArchiveEntryOptions,
    ) -> Result<()> {
        self.append_directory_contents(dest, source, opts)
    }

    fn append_directory_contents(
        &mut self,
        dest_prefix: &Path,
        current_dir: &Path,
        opts: ArchiveEntryOptions,
    ) -> Result<()> {
        for entry in std::fs::read_dir(current_dir)
            .with_context(|| format!("Reading MSIX directory tree `{}`", current_dir.display()))?
        {
            let item = entry?;
            let sub_name = item.file_name();
            let source_child = item.path();
            let dest_child = dest_prefix.join(&sub_name);
            let ftype = item.file_type()?;
            if ftype.is_dir() {
                self.append_directory_contents(&dest_child, &source_child, opts)?;
            } else if ftype.is_file() {
                self.embed_file(&source_child, &dest_child, opts)?;
            }
        }
        Ok(())
    }

    pub fn add_assets_dir(&mut self, source_dir: &Path, dest_prefix: &Path) -> Result<()> {
        self.ingest_assets_tree(source_dir, source_dir, dest_prefix)
    }

    fn ingest_assets_tree(
        &mut self,
        root_dir: &Path,
        current_dir: &Path,
        dest_prefix: &Path,
    ) -> Result<()> {
        if !current_dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(current_dir)
            .with_context(|| format!("Scanning Windows asset tree `{}`", current_dir.display()))?
        {
            let item = entry?;
            let disk_path = item.path();
            let ftype = item.file_type()?;

            if ftype.is_dir() {
                self.ingest_assets_tree(root_dir, &disk_path, dest_prefix)?;
            } else if ftype.is_file() {
                let relative = disk_path.strip_prefix(root_dir)?;
                let dest_target = dest_prefix.join(relative);
                self.embed_file(&disk_path, &dest_target, ArchiveEntryOptions::Compressed)?;

                let scope_name = dest_target
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_string_lossy()
                    .to_string();
                let file_label = dest_target
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let win_path_str = dest_target.to_string_lossy().replace('/', "\\");

                self.icon_variants
                    .push((scope_name, file_label, vec![(100, win_path_str)]));
            }
        }
        Ok(())
    }

    pub fn finalize_package(mut self, signer: Option<IdentityKeypair>) -> Result<()> {
        if !self.has_pri && !self.icon_variants.is_empty() {
            self.generate_pri()?;
        }

        let signing_key = match signer {
            Some(s) => s,
            None => IdentityKeypair::new(DEBUG_PEM)
                .context("Failed initializing embedded debug signer")?,
        };

        let mut archive = self.zip.take().context("MSIX archive sink is closed")?;

        let manifest_bytes = quick_xml::se::to_string(&self.manifest)?;
        let manifest_path = Path::new(PACKAGE_MANIFEST_PART);
        self.content_types.register_path(manifest_path);
        self.block_map
            .add_data(manifest_path, manifest_bytes.as_bytes());
        archive.create_file(
            manifest_path,
            ArchiveEntryOptions::Compressed,
            manifest_bytes.as_bytes(),
        )?;

        let ct_bytes = quick_xml::se::to_string(&self.content_types.finish())?;
        let ct_digest = Sha256::digest(ct_bytes.as_bytes());
        let ct_path = Path::new(OPC_CONTENT_TYPES_PART);
        self.block_map.add_data(ct_path, ct_bytes.as_bytes());
        archive.create_file(
            ct_path,
            ArchiveEntryOptions::Compressed,
            ct_bytes.as_bytes(),
        )?;

        let bm_bytes = quick_xml::se::to_string(&self.block_map.finish())?;
        let bm_digest = Sha256::digest(bm_bytes.as_bytes());
        let bm_path = Path::new(PACKAGE_BLOCK_MAP_PART);
        archive.create_file(
            bm_path,
            ArchiveEntryOptions::Compressed,
            bm_bytes.as_bytes(),
        )?;

        let mut handle = archive.finish_into_inner()?;

        sign_container_handle(
            &mut handle,
            &signing_key,
            bm_digest.into(),
            ct_digest.into(),
            self.compress,
        )
    }

    pub fn repackage_binaries(
        msix_path: &Path,
        updated_binaries: &[(&Path, String)],
        signer: Option<IdentityKeypair>,
        compress: bool,
    ) -> Result<()> {
        let staging_path = msix_path.with_extension(format!("msix.repack.{}", std::process::id()));
        {
            let src_file = File::open(msix_path).with_context(|| {
                format!("Failed to open existing MSIX `{}`", msix_path.display())
            })?;
            let mut archive = ZipArchive::new(src_file)?;
            let mut new_zip = BoundaryZipArchive::new(&staging_path, compress)?;

            let updated_names: std::collections::HashSet<String> = updated_binaries
                .iter()
                .map(|(_, name)| name.clone())
                .collect();

            for i in 0..archive.len() {
                let name = {
                    let entry = archive.by_index_raw(i)?;
                    entry.name().to_string()
                };

                if updated_names.contains(&name)
                    || name == PACKAGE_BLOCK_MAP_PART
                    || name == OPC_CONTENT_TYPES_PART
                    || name == PACKAGE_SIGNATURE_PART
                {
                    continue;
                }

                let entry = archive.by_index_raw(i)?;
                new_zip.add_zip_file(entry)?;
            }

            for (src_path, dest_name) in updated_binaries {
                new_zip.add_file(
                    src_path,
                    Path::new(dest_name),
                    ArchiveEntryOptions::Compressed,
                )?;
            }

            new_zip.finish()?;
        }

        Self::sign_package_file(&staging_path, signer, compress)?;
        std::fs::rename(&staging_path, msix_path)?;
        Ok(())
    }

    pub fn sign_package_file(
        path: &Path,
        signer: Option<IdentityKeypair>,
        compress: bool,
    ) -> Result<()> {
        let signing_key = match signer {
            Some(s) => s,
            None => IdentityKeypair::new(DEBUG_PEM)
                .context("Failed initializing embedded debug signer")?,
        };

        let staging_path = path.with_extension(format!("msix.resign.{}", std::process::id()));
        {
            let src_file = File::open(path)?;
            let mut archive = ZipArchive::new(src_file)?;
            let mut new_zip = BoundaryZipArchive::new(&staging_path, compress)?;
            let mut content_types = OpcContentTypesBuilder::default();
            let mut block_map = MsixBlockMapBuilder::new();

            for i in 0..archive.len() {
                let (name, is_file) = {
                    let entry = archive.by_index_raw(i)?;
                    (entry.name().to_string(), entry.is_file())
                };

                if name == PACKAGE_BLOCK_MAP_PART
                    || name == OPC_CONTENT_TYPES_PART
                    || name == PACKAGE_SIGNATURE_PART
                {
                    continue;
                }

                if is_file {
                    let mut data = Vec::new();
                    {
                        let mut entry = archive.by_index(i)?;
                        entry.read_to_end(&mut data)?;
                    }
                    content_types.register_path(Path::new(&name));
                    block_map.add_data(Path::new(&name), &data);
                }

                let entry = archive.by_index_raw(i)?;
                new_zip.add_zip_file(entry)?;
            }

            let ct_xml = quick_xml::se::to_string(&content_types.finish())?;
            let ct_digest = Sha256::digest(ct_xml.as_bytes());
            block_map.add_data(Path::new(OPC_CONTENT_TYPES_PART), ct_xml.as_bytes());
            new_zip.create_file(
                Path::new(OPC_CONTENT_TYPES_PART),
                ArchiveEntryOptions::Compressed,
                ct_xml.as_bytes(),
            )?;

            let bm_xml = quick_xml::se::to_string(&block_map.finish())?;
            let bm_digest = Sha256::digest(bm_xml.as_bytes());
            new_zip.create_file(
                Path::new(PACKAGE_BLOCK_MAP_PART),
                ArchiveEntryOptions::Compressed,
                bm_xml.as_bytes(),
            )?;

            let mut handle = new_zip.finish_into_inner()?;
            sign_container_handle(
                &mut handle,
                &signing_key,
                bm_digest.into(),
                ct_digest.into(),
                compress,
            )?;
        }

        std::fs::rename(&staging_path, path)?;
        Ok(())
    }
}

fn sign_container_handle(
    file: &mut File,
    signer: &IdentityKeypair,
    block_map_hash: [u8; 32],
    content_types_hash: [u8; 32],
    compress: bool,
) -> Result<()> {
    let zip_meta = CentralDirectoryLocator::new(&mut *file)?;
    file.rewind()?;

    let mut hasher = Sha256::new();
    let mut payload_reader = (&mut *file).take(zip_meta.cd_start);
    std::io::copy(&mut payload_reader, &mut hasher)?;
    let payload_hash = hasher.finalize_reset();

    hasher.reset();
    std::io::copy(file, &mut hasher)?;
    let central_dir_hash = hasher.finalize();

    let digest_spec = AuthenticodeDigests {
        axpc: payload_hash.into(),
        axcd: central_dir_hash.into(),
        axct: content_types_hash,
        axbm: block_map_hash,
        axci: [0u8; 32],
    };

    let p7x_envelope = compute_authenticode_envelope(signer, &digest_spec)
        .context("failed to generate MSIX AppxSignature.p7x block")?;

    let mut re_zip = BoundaryZipArchive::append_file(file.try_clone()?, compress)?;
    re_zip.create_file(
        Path::new(PACKAGE_SIGNATURE_PART),
        ArchiveEntryOptions::Compressed,
        &p7x_envelope,
    )?;
    re_zip.finish()?;

    Ok(())
}
