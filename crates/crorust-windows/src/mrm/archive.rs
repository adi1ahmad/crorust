use anyhow::{ensure, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};

pub const MRM_MAGIC_HEADER: &[u8; 8] = b"mrm_pri2";
pub const MRM_MAGIC_FOOTER: &[u8; 8] = b"mrm_pri2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetVariantRecord {
    pub qualifier_scale: u32,
    pub target_relative_path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MrmPriPackage {
    pub package_identity: String,
    pub resources: BTreeMap<String, Vec<AssetVariantRecord>>,
}

impl MrmPriPackage {
    pub fn new(package_identity: impl Into<String>) -> Self {
        Self {
            package_identity: package_identity.into(),
            resources: BTreeMap::new(),
        }
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<()> {
        let mut body = Vec::new();

        let pkg_bytes = self.package_identity.as_bytes();
        body.write_u16::<LittleEndian>(pkg_bytes.len() as u16)?;
        body.write_all(pkg_bytes)?;
        pad_to_boundary(&mut body, 8)?;

        body.write_u32::<LittleEndian>(self.resources.len() as u32)?;

        for (name, variants) in &self.resources {
            let name_bytes = name.as_bytes();
            body.write_u16::<LittleEndian>(name_bytes.len() as u16)?;
            body.write_all(name_bytes)?;
            body.write_u16::<LittleEndian>(variants.len() as u16)?;

            for variant in variants {
                body.write_u32::<LittleEndian>(variant.qualifier_scale)?;
                let path_bytes = variant.target_relative_path.as_bytes();
                body.write_u16::<LittleEndian>(path_bytes.len() as u16)?;
                body.write_all(path_bytes)?;
            }
        }
        pad_to_boundary(&mut body, 8)?;

        let total_length = (16 + body.len() + 16) as u32;

        writer.write_all(MRM_MAGIC_HEADER)?;
        writer.write_u16::<LittleEndian>(2)?;
        writer.write_u16::<LittleEndian>(0)?;
        writer.write_u32::<LittleEndian>(total_length)?;

        writer.write_all(&body)?;

        writer.write_u32::<LittleEndian>(total_length)?;
        writer.write_u16::<LittleEndian>(2)?;
        writer.write_u16::<LittleEndian>(0)?;
        writer.write_all(MRM_MAGIC_FOOTER)?;

        Ok(())
    }

    pub fn read(reader: &mut (impl Read + Seek)) -> Result<Self> {
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        ensure!(
            &magic == MRM_MAGIC_HEADER || &magic == b"mrm_pri0",
            "Invalid MRM PRI magic header: {:?}",
            magic
        );

        let _major = reader.read_u16::<LittleEndian>()?;
        let _minor = reader.read_u16::<LittleEndian>()?;
        let total_length = reader.read_u32::<LittleEndian>()?;
        ensure!(
            total_length >= 32,
            "PRI total length too small: {}",
            total_length
        );

        let pkg_len = reader.read_u16::<LittleEndian>()? as usize;
        let mut pkg_buf = vec![0u8; pkg_len];
        reader.read_exact(&mut pkg_buf)?;
        let package_identity =
            String::from_utf8(pkg_buf).context("Invalid UTF-8 package identity")?;

        align_reader(reader, 8)?;

        let resource_count = reader.read_u32::<LittleEndian>()? as usize;
        let mut resources = BTreeMap::new();

        for _ in 0..resource_count {
            let name_len = reader.read_u16::<LittleEndian>()? as usize;
            let mut name_buf = vec![0u8; name_len];
            reader.read_exact(&mut name_buf)?;
            let name = String::from_utf8(name_buf).context("Invalid UTF-8 resource name")?;

            let variant_count = reader.read_u16::<LittleEndian>()? as usize;
            let mut variants = Vec::with_capacity(variant_count);

            for _ in 0..variant_count {
                let qualifier_scale = reader.read_u32::<LittleEndian>()?;
                let path_len = reader.read_u16::<LittleEndian>()? as usize;
                let mut path_buf = vec![0u8; path_len];
                reader.read_exact(&mut path_buf)?;
                let target_relative_path =
                    String::from_utf8(path_buf).context("Invalid UTF-8 target path")?;

                variants.push(AssetVariantRecord {
                    qualifier_scale,
                    target_relative_path,
                });
            }

            resources.insert(name, variants);
        }

        Ok(Self {
            package_identity,
            resources,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct MrmPriPackageBuilder {
    file: MrmPriPackage,
}

impl MrmPriPackageBuilder {
    pub fn new(package_identity: impl Into<String>) -> Self {
        Self {
            file: MrmPriPackage::new(package_identity),
        }
    }

    pub fn add_asset_scale_variants(
        &mut self,
        scope: &str,
        item_name: &str,
        variants: &[(u32, String)],
    ) {
        let key = if scope.is_empty() {
            format!("Files/{}", item_name)
        } else {
            format!("Files/{}/{}", scope.replace('\\', "/"), item_name)
        };

        let entry_variants = variants
            .iter()
            .map(|(scale, path)| AssetVariantRecord {
                qualifier_scale: *scale,
                target_relative_path: path.clone(),
            })
            .collect();

        self.file.resources.insert(key, entry_variants);
    }

    pub fn finish(self) -> Result<MrmPriPackage> {
        Ok(self.file)
    }
}

fn pad_to_boundary(buf: &mut Vec<u8>, align: usize) -> Result<()> {
    let remainder = buf.len() % align;
    if remainder != 0 {
        let padding = align - remainder;
        buf.resize(buf.len() + padding, 0);
    }
    Ok(())
}

fn align_reader(reader: &mut (impl Read + Seek), align: u64) -> Result<()> {
    let current = reader.stream_position()?;
    let remainder = current % align;
    if remainder != 0 {
        reader.seek(SeekFrom::Current((align - remainder) as i64))?;
    }
    Ok(())
}
