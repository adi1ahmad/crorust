use crate::manifest::manifest_dom::AndroidXmlManifestDescriptor;
use crate::package::authenticity::{sign_apk_file, verify_apk_signature};
use crate::protocol::chunk_stream::ResourceValueType;
use crate::protocol::chunks::AndroidChunk;
use crate::resources::symbol_catalog::ResourceSymbols;
use crate::resources::table_compiler::{
    compile_mipmap_resources, compile_resources, ResourceCatalogTable,
};
use crate::utils::AndroidAbi;
use anyhow::{bail, Context, Result};
use crorust_common::{
    ArchiveEntryOptions, BoundaryZipArchive, Certificate, IdentityKeypair, RasterResizer,
    TransformConstraint,
};
use std::io::Cursor;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppEntryPoint {
    pub package: String,
    pub activity: String,
}

impl std::fmt::Display for AppEntryPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.package, self.activity)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AxmlAttribute<'a> {
    pub name: &'a str,
    pub namespace: Option<&'a str>,
    pub value: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AxmlElement<'a> {
    pub name: &'a str,
    pub attributes: Vec<AxmlAttribute<'a>>,
    pub children: Vec<AxmlElement<'a>>,
}

impl<'a> AxmlElement<'a> {
    pub fn find_attribute(&self, name: &str) -> Option<&'a str> {
        self.attributes
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value)
    }

    pub fn namespaced_attr(&self, ns: &str, name: &str) -> Option<&'a str> {
        self.attributes
            .iter()
            .find(|a| a.name == name && a.namespace == Some(ns))
            .map(|a| a.value)
    }

    pub fn children_by_tag<'b>(
        &'b self,
        tag: &'b str,
    ) -> impl Iterator<Item = &'b AxmlElement<'a>> {
        self.children.iter().filter(move |c| c.name == tag)
    }
}

pub struct AndroidPackageContainer {
    manifest: AndroidXmlManifestDescriptor,
    path: PathBuf,
    zip: BoundaryZipArchive,
    icon_cache_dir: Option<PathBuf>,
}

impl AndroidPackageContainer {
    pub fn new(
        path: PathBuf,
        manifest: AndroidXmlManifestDescriptor,
        compress: bool,
    ) -> Result<Self> {
        let zip = BoundaryZipArchive::new(&path, compress)?;
        Ok(Self {
            manifest,
            path,
            zip,
            icon_cache_dir: None,
        })
    }

    pub fn set_icon_cache_dir(&mut self, dir: impl Into<PathBuf>) {
        self.icon_cache_dir = Some(dir.into());
    }

    pub fn bundle_resources(&mut self, icon: Option<&Path>, android: &Path) -> Result<()> {
        self.bundle_resources_with_symbols(icon, android, None, None)
    }

    pub fn bundle_resources_with_symbols(
        &mut self,
        icon: Option<&Path>,
        android: &Path,
        theme: Option<&str>,
        symbols: Option<&ResourceSymbols>,
    ) -> Result<()> {
        let mut table = ResourceCatalogTable::default();
        table.import_apk(android)?;

        let package = self
            .manifest
            .package
            .as_deref()
            .context("missing manifest.package")?;

        let icon_name = if icon.is_some() { Some("icon") } else { None };
        let res_chunk = compile_resources(package, icon_name, theme, symbols)?;

        let mut buf = Vec::new();
        res_chunk.write(&mut buf)?;
        self.zip.create_file(
            Path::new("resources.arsc"),
            ArchiveEntryOptions::Aligned(4),
            &buf,
        )?;

        table.import_chunk(&res_chunk);

        if let Some(path) = icon {
            let mut scaler = RasterResizer::open(path)?;
            if let Some(cache_dir) = &self.icon_cache_dir {
                scaler.set_cache_dir(Some(cache_dir.clone()));
            }
            scaler.optimize();

            let mipmap = compile_mipmap_resources(package, "icon")?;
            let variant_specs: Vec<(String, TransformConstraint)> = mipmap
                .variants()
                .map(|(name, size)| (name, TransformConstraint::new(size)))
                .collect();

            let constraints: Vec<TransformConstraint> = variant_specs.iter().map(|v| v.1).collect();
            let rendered_images = scaler.render_batch_parallel(&constraints)?;

            for ((name, _), (_, png_bytes)) in variant_specs.into_iter().zip(rendered_images) {
                self.zip.create_file(
                    Path::new(&name),
                    ArchiveEntryOptions::Aligned(4),
                    &png_bytes,
                )?;
            }
            self.manifest.application.icon = Some("@mipmap/icon".into());
        }

        if let Some(theme_name) = theme {
            self.manifest.application.theme = Some(format!("@style/{theme_name}"));
        }

        let xml = quick_xml::se::to_string(&self.manifest)?;
        let manifest_chunk = crate::manifest::compile_xml_with_symbols(&xml, &table, symbols)?;
        buf.clear();
        manifest_chunk.write(&mut buf)?;

        self.zip.create_file(
            Path::new("AndroidManifest.xml"),
            ArchiveEntryOptions::Compressed,
            &buf,
        )?;

        Ok(())
    }

    pub fn add_raw_file(
        &mut self,
        dest: &Path,
        options: ArchiveEntryOptions,
        data: &[u8],
    ) -> Result<()> {
        self.zip.create_file(dest, options, data)?;
        Ok(())
    }

    pub fn add_file_at(
        &mut self,
        src: &Path,
        dest: &Path,
        options: ArchiveEntryOptions,
    ) -> Result<()> {
        self.zip.add_file(src, dest, options)?;
        Ok(())
    }

    pub fn bundle_dex(&mut self, dex_path: &Path) -> Result<()> {
        let name = dex_path.file_name().context("invalid dex file path")?;
        self.zip
            .add_file(dex_path, Path::new(name), ArchiveEntryOptions::Aligned(4))?;
        Ok(())
    }

    pub fn add_asset_with_dest(
        &mut self,
        path: &Path,
        dest: &Path,
        options: ArchiveEntryOptions,
    ) -> Result<()> {
        let target_in_apk = Path::new("assets").join(dest);
        if path.is_file() {
            self.zip.add_file(path, &target_in_apk, options)?;
        } else if path.is_dir() {
            add_directory_recursive(&mut self.zip, path, path, &target_in_apk, options)?;
        }
        Ok(())
    }

    pub fn bundle_shared_library(&mut self, target: AndroidAbi, lib: &Path) -> Result<()> {
        let filename = lib.file_name().context("invalid lib path")?;
        let dest = Path::new("lib").join(target.as_str()).join(filename);
        self.zip
            .add_file(lib, &dest, ArchiveEntryOptions::Aligned(16384))?;
        Ok(())
    }

    pub fn finalize_apk(self, signer: Option<IdentityKeypair>) -> Result<()> {
        self.zip.finalize_archive()?;
        sign_apk_file(&self.path, signer.as_ref())?;
        Ok(())
    }

    pub fn repackage_libs(
        apk_path: &Path,
        libs: &[(AndroidAbi, PathBuf)],
        signer: Option<IdentityKeypair>,
        alignment: u32,
        compress: bool,
    ) -> Result<()> {
        let staging_path = apk_path.with_extension(format!("apk.repack.{}", std::process::id()));
        {
            let src_file = std::fs::File::open(apk_path)
                .with_context(|| format!("Failed to open existing APK `{}`", apk_path.display()))?;
            let mut archive = zip::ZipArchive::new(src_file)?;
            let mut new_zip = BoundaryZipArchive::new(&staging_path, compress)?;

            for i in 0..archive.len() {
                let (name, is_lib, is_stored) = {
                    let entry = archive.by_index_raw(i)?;
                    (
                        entry.name().to_string(),
                        entry.name().starts_with("lib/"),
                        entry.compression() == zip::CompressionMethod::Stored,
                    )
                };
                if is_lib {
                    continue;
                }
                if is_stored {
                    let mut entry = archive.by_index(i)?;
                    new_zip.start_file(Path::new(&name), ArchiveEntryOptions::Aligned(4))?;
                    std::io::copy(&mut entry, &mut new_zip)?;
                } else {
                    let entry = archive.by_index_raw(i)?;
                    new_zip.add_zip_file(entry)?;
                }
            }

            for (target, lib_path) in libs {
                let name = lib_path
                    .file_name()
                    .context("invalid native library path")?;
                new_zip.add_file(
                    lib_path,
                    &Path::new("lib").join(target.as_str()).join(name),
                    ArchiveEntryOptions::Aligned(alignment as u16),
                )?;
            }

            new_zip.finalize_archive()?;
        }

        sign_apk_file(&staging_path, signer.as_ref())?;
        if apk_path.exists() {
            let _ = std::fs::remove_file(apk_path);
        }
        std::fs::rename(&staging_path, apk_path).or_else(|_| {
            std::fs::copy(&staging_path, apk_path)?;
            let _ = std::fs::remove_file(&staging_path);
            Ok::<(), std::io::Error>(())
        })?;

        Ok(())
    }

    pub fn sign_apk_target(path: &Path, signer: Option<IdentityKeypair>) -> Result<()> {
        sign_apk_file(path, signer.as_ref())?;
        Ok(())
    }

    pub fn verify_apk_target(path: &Path) -> Result<Vec<Certificate>> {
        verify_apk_signature(path)
    }

    pub fn query_entry_point(path: &Path) -> Result<AppEntryPoint> {
        let manifest = crorust_common::unpack_single_entry_secure(path, "AndroidManifest.xml")?;
        let chunk = AndroidChunk::parse(&mut Cursor::new(manifest))?;
        let chunks = if let AndroidChunk::Xml(chunks) = chunk {
            chunks
        } else {
            bail!("invalid manifest: root chunk is not XML");
        };
        find_entry_point(&chunks)
    }
}

pub fn build_axml_tree<'a>(
    chunks: &[AndroidChunk],
    strings: &'a [String],
) -> Result<Vec<AxmlElement<'a>>> {
    let mut roots: Vec<AxmlElement<'a>> = Vec::new();
    let mut stack: Vec<AxmlElement<'a>> = Vec::new();

    for chunk in chunks {
        match chunk {
            AndroidChunk::XmlStartElement(_header, el, attrs) => {
                let name = resolve_string(el.element_name_index, strings).unwrap_or("");
                let mut parsed_attrs = Vec::with_capacity(attrs.len());
                for attr in attrs {
                    let attr_name =
                        resolve_string(attr.attribute_name_index, strings).unwrap_or("");
                    let attr_ns = resolve_string(attr.namespace_uri_index, strings);
                    let attr_val = if let Some(s) = resolve_string(attr.raw_string_index, strings) {
                        s
                    } else if attr.typed_value_entry.data_type_tag
                        == ResourceValueType::String as u8
                    {
                        resolve_string(attr.typed_value_entry.data_payload as i32, strings)
                            .unwrap_or("")
                    } else {
                        ""
                    };
                    parsed_attrs.push(AxmlAttribute {
                        name: attr_name,
                        namespace: attr_ns,
                        value: attr_val,
                    });
                }
                stack.push(AxmlElement {
                    name,
                    attributes: parsed_attrs,
                    children: Vec::new(),
                });
            }
            AndroidChunk::XmlEndElement(_header, _el) => {
                if let Some(closed_node) = stack.pop() {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(closed_node);
                    } else {
                        roots.push(closed_node);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(roots)
}

fn resolve_string(idx: i32, strings: &[String]) -> Option<&str> {
    if idx >= 0 && (idx as usize) < strings.len() {
        Some(strings[idx as usize].as_str())
    } else {
        None
    }
}

pub fn find_entry_point(chunks: &[AndroidChunk]) -> Result<AppEntryPoint> {
    let strings = if let Some(AndroidChunk::StringPool(strings, _)) = chunks.first() {
        strings
    } else {
        bail!("invalid manifest: missing string pool");
    };

    let roots = build_axml_tree(chunks, strings)?;
    let manifest_el = roots
        .iter()
        .find(|el| el.name == "manifest")
        .context("invalid manifest: missing <manifest> root element")?;

    let package = manifest_el
        .find_attribute("package")
        .context("invalid manifest: missing 'package' attribute")?
        .to_string();

    let app_el = manifest_el
        .children_by_tag("application")
        .next()
        .context("invalid manifest: missing <application> element")?;

    for act in app_el.children_by_tag("activity") {
        for filter in act.children_by_tag("intent-filter") {
            let has_main = filter
                .children_by_tag("action")
                .any(|a| a.find_attribute("name") == Some("android.intent.action.MAIN"));
            let has_launcher = filter
                .children_by_tag("category")
                .any(|c| c.find_attribute("name") == Some("android.intent.category.LAUNCHER"));

            if has_main && has_launcher {
                let act_name = act
                    .find_attribute("name")
                    .context("missing activity name")?;
                let full_act = if act_name.starts_with('.') {
                    format!("{package}{act_name}")
                } else if !act_name.contains('.') {
                    format!("{package}.{act_name}")
                } else {
                    act_name.to_string()
                };
                return Ok(AppEntryPoint {
                    package,
                    activity: full_act,
                });
            }
        }
    }

    bail!("No launchable activity found in manifest")
}

fn add_directory_recursive(
    zip: &mut BoundaryZipArchive,
    root: &Path,
    current: &Path,
    target_base: &Path,
    options: ArchiveEntryOptions,
) -> Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            add_directory_recursive(zip, root, &path, target_base, options)?;
        } else if path.is_file() {
            let rel = path.strip_prefix(root)?;
            let full_dest = target_base.join(rel);
            zip.add_file(&path, &full_dest, options)?;
        }
    }
    Ok(())
}
