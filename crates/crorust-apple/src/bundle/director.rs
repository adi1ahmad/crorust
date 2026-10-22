use crate::codesign::prepare_signing_key_and_cert;
use crate::plist::DarwinPropertyList;
use anyhow::{Context, Result};
use apple_codesign::{BundleSigner, CodeSignatureFlags, SettingsScope, SigningSettings};
use crorust_common::{IdentityKeypair, RasterResizer, TransformConstraint};
use icns::{IconFamily, Image};
use plist::Value;
use rasn_cms::{ContentInfo, SignedData};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

pub const MACOS_ICON_SIZES: [u32; 6] = [16, 32, 64, 128, 256, 512];
pub const IOS_ICON_SIZES: [u32; 7] = [58, 76, 80, 120, 152, 167, 1024];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DarwinPlatform {
    MacOs,
    Ios,
}

pub struct AppBundleDirector {
    appdir: PathBuf,
    plist: DarwinPropertyList,
    platform: DarwinPlatform,
    entitlements: Option<Value>,
    is_development: bool,
    icon_cache_dir: Option<PathBuf>,
}

impl AppBundleDirector {
    pub fn new(output_dir: &Path, plist: DarwinPropertyList) -> Result<Self> {
        let name = plist
            .bundle_name
            .as_deref()
            .context("PropertyList missing CFBundleName")?;

        let bundle_folder = if name.ends_with(".app") {
            name.to_string()
        } else {
            format!("{name}.app")
        };

        let appdir = output_dir.join(bundle_folder);
        let _ = std::fs::remove_dir_all(&appdir);
        std::fs::create_dir_all(&appdir).with_context(|| {
            format!("Failed to create bundle directory at {}", appdir.display())
        })?;

        let platform = if plist.is_ios() {
            DarwinPlatform::Ios
        } else {
            DarwinPlatform::MacOs
        };

        Ok(Self {
            appdir,
            plist,
            platform,
            entitlements: None,
            is_development: false,
            icon_cache_dir: None,
        })
    }

    pub fn open_or_new(output_dir: &Path, plist: DarwinPropertyList) -> Result<Self> {
        let name = plist
            .bundle_name
            .as_deref()
            .context("PropertyList missing CFBundleName")?;

        let bundle_folder = if name.ends_with(".app") {
            name.to_string()
        } else {
            format!("{name}.app")
        };

        let appdir = output_dir.join(bundle_folder);
        std::fs::create_dir_all(&appdir).with_context(|| {
            format!("Failed to create bundle directory at {}", appdir.display())
        })?;

        let platform = if plist.is_ios() {
            DarwinPlatform::Ios
        } else {
            DarwinPlatform::MacOs
        };

        Ok(Self {
            appdir,
            plist,
            platform,
            entitlements: None,
            is_development: false,
            icon_cache_dir: None,
        })
    }

    pub fn bundle_appdir(&self) -> &Path {
        &self.appdir
    }

    pub fn property_list(&self) -> &DarwinPropertyList {
        &self.plist
    }

    pub fn property_list_mut(&mut self) -> &mut DarwinPropertyList {
        &mut self.plist
    }

    pub fn target_darwin_platform(&self) -> DarwinPlatform {
        self.platform
    }

    pub fn set_icon_cache_dir(&mut self, dir: impl Into<PathBuf>) {
        self.icon_cache_dir = Some(dir.into());
    }

    pub fn contents_dir(&self) -> PathBuf {
        match self.platform {
            DarwinPlatform::MacOs => self.appdir.join("Contents"),
            DarwinPlatform::Ios => self.appdir.clone(),
        }
    }

    pub fn bundle_executable_dir(&self) -> PathBuf {
        match self.platform {
            DarwinPlatform::MacOs => self.contents_dir().join("MacOS"),
            DarwinPlatform::Ios => self.appdir.clone(),
        }
    }

    pub fn resources_dir(&self) -> PathBuf {
        match self.platform {
            DarwinPlatform::MacOs => self.contents_dir().join("Resources"),
            DarwinPlatform::Ios => self.appdir.clone(),
        }
    }

    pub fn frameworks_dir(&self) -> PathBuf {
        match self.platform {
            DarwinPlatform::MacOs => self.contents_dir().join("Frameworks"),
            DarwinPlatform::Ios => self.appdir.join("Frameworks"),
        }
    }

    pub fn register_existing_icons(&mut self) {
        match self.platform {
            DarwinPlatform::Ios => {
                for size in IOS_ICON_SIZES {
                    self.plist
                        .icon_files
                        .push(format!("icon_{size}x{size}.png"));
                }
            }
            DarwinPlatform::MacOs => {
                self.plist.icon_file = Some("AppIcon".to_string());
            }
        }
    }

    pub fn rasterize_and_attach_icon(&mut self, source_icon: &Path) -> Result<()> {
        let mut scaler = RasterResizer::open(source_icon)
            .with_context(|| format!("Failed to read icon from {}", source_icon.display()))?;

        if let Some(cache_dir) = &self.icon_cache_dir {
            scaler.set_cache_dir(Some(cache_dir.clone()));
        }

        match self.platform {
            DarwinPlatform::Ios => {
                let constraints: Vec<TransformConstraint> = IOS_ICON_SIZES
                    .iter()
                    .map(|&s| TransformConstraint::new(s))
                    .collect();
                let rendered = scaler.render_batch_parallel(&constraints)?;

                for (&size, (_, png_bytes)) in IOS_ICON_SIZES.iter().zip(rendered) {
                    let file_name = format!("icon_{size}x{size}.png");
                    let out_path = self.appdir.join(&file_name);
                    std::fs::write(&out_path, png_bytes)?;
                    self.plist.icon_files.push(file_name);
                }
            }
            DarwinPlatform::MacOs => {
                let constraints: Vec<TransformConstraint> = MACOS_ICON_SIZES
                    .iter()
                    .map(|&s| TransformConstraint::new(s))
                    .collect();
                let rendered = scaler.render_batch_parallel(&constraints)?;

                let mut family = IconFamily::new();
                for (&size, (_, png_bytes)) in MACOS_ICON_SIZES.iter().zip(rendered) {
                    let image = Image::read_png(&*png_bytes)
                        .with_context(|| format!("Failed to parse icon at size {size}"))?;
                    family.add_icon(&image)?;
                }

                let icns_path = self.resources_dir().join("AppIcon.icns");
                if let Some(parent) = icns_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                family.write(BufWriter::new(File::create(&icns_path)?))?;
                self.plist.icon_file = Some("AppIcon".to_string());
            }
        }

        Ok(())
    }

    pub fn stage_bundle_executable(&mut self, source_binary: &Path) -> Result<()> {
        let file_name = source_binary
            .file_name()
            .and_then(|n| n.to_str())
            .with_context(|| format!("Invalid binary path: {}", source_binary.display()))?;

        let exe_dir = self.bundle_executable_dir();
        std::fs::create_dir_all(&exe_dir)?;
        let destination = exe_dir.join(file_name);
        std::fs::copy(source_binary, &destination)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o755))?;
        }

        if self.plist.executable_name.is_none() {
            self.plist.executable_name = Some(file_name.to_string());
        }

        Ok(())
    }

    pub fn add_resource_file(&self, source: &Path, relative_dest: &Path) -> Result<()> {
        let dest = self.resources_dir().join(relative_dest);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(source, dest)?;
        Ok(())
    }

    pub fn add_resource_dir(&self, source: &Path, relative_dest: &Path) -> Result<()> {
        let target = self.resources_dir().join(relative_dest);
        std::fs::create_dir_all(&target)?;
        crorust_common::copy_dir_all(source, &target)?;
        Ok(())
    }

    pub fn add_framework_or_dylib(&self, path: &Path) -> Result<()> {
        let file_name = path
            .file_name()
            .with_context(|| format!("Invalid library path: {}", path.display()))?;

        let fw_dir = self.frameworks_dir();
        std::fs::create_dir_all(&fw_dir)?;
        let dest = fw_dir.join(file_name);

        if path.is_dir() {
            crorust_common::copy_dir_all(path, &dest)?;
        } else {
            std::fs::copy(path, dest)?;
        }
        Ok(())
    }

    pub fn write_embedded_provisioning_profile(&mut self, profile_der_bytes: &[u8]) -> Result<()> {
        let cms_info = rasn::der::decode::<ContentInfo>(profile_der_bytes)
            .map_err(|err| anyhow::anyhow!("ASN.1 DER decode error: {err}"))?;
        let signed_data = rasn::der::decode::<SignedData>(cms_info.content.as_bytes())
            .map_err(|err| anyhow::anyhow!("CMS SignedData decode error: {err}"))?;

        let raw_xml = signed_data
            .encap_content_info
            .content
            .as_ref()
            .context("Provisioning profile is missing encapsulated XML content")?;

        let parsed_profile: plist::Value = plist::from_reader_xml(raw_xml.as_ref())
            .context("Failed to parse embedded mobileprovision XML plist")?;

        let dict = parsed_profile
            .as_dictionary()
            .context("Provisioning profile root is not a dictionary")?;

        let entitlements = dict
            .get("Entitlements")
            .context("Missing Entitlements key in provisioning profile")?
            .clone();

        self.is_development = dict.get("ProvisionedDevices").is_some();
        self.entitlements = Some(entitlements);

        let out_path = self.appdir.join("embedded.mobileprovision");
        std::fs::write(out_path, profile_der_bytes)?;
        Ok(())
    }

    pub fn set_entitlements(&mut self, entitlements: Value) {
        self.entitlements = Some(entitlements);
    }

    pub fn seal_and_sign(&self, signer: Option<IdentityKeypair>) -> Result<()> {
        let contents = self.contents_dir();
        std::fs::create_dir_all(&contents)?;

        let plist_path = match self.platform {
            DarwinPlatform::MacOs => contents.join("Info.plist"),
            DarwinPlatform::Ios => self.appdir.join("Info.plist"),
        };
        self.plist.write_to_file(&plist_path)?;

        if matches!(self.platform, DarwinPlatform::MacOs) {
            let pkg_info = contents.join("PkgInfo");
            std::fs::write(pkg_info, b"APPL????")?;
        }

        if let Some(signer) = signer {
            let bundle_id = self
                .plist
                .bundle_identifier
                .as_deref()
                .context("Cannot sign Darwin bundle without CFBundleIdentifier")?;

            tracing::info!(bundle = %self.appdir.display(), bundle_id = %bundle_id, "Signing Darwin AppBundle");

            let mut settings = SigningSettings::default();
            let (key, cert) = prepare_signing_key_and_cert(&signer)?;
            settings.set_signing_key(&key, cert);
            let _ = settings.set_team_id_from_signing_certificate();

            if self.is_development {
                settings.set_time_stamp_url("http://timestamp.apple.com/ts01")?;
            }

            if let Some(entitlements) = &self.entitlements {
                let mut buf = Vec::new();
                entitlements.to_writer_xml(&mut buf)?;
                let ent_str = std::str::from_utf8(&buf)?;
                settings.set_entitlements_xml(SettingsScope::Main, ent_str)?;
            }

            if matches!(self.platform, DarwinPlatform::MacOs) {
                settings.set_code_signature_flags(SettingsScope::Main, CodeSignatureFlags::RUNTIME);
            }

            let bundle_signer = BundleSigner::new_from_path(&self.appdir)?;
            bundle_signer.write_signed_bundle(&self.appdir, &settings)?;
        }

        Ok(())
    }
}
