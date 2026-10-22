use super::{PackagingContext, PackagingOutput, PlatformPackager};
use crate::core::env::SessionExecutionContext;
use crate::core::target::{TargetOpt, TargetPlatform};
use crate::packaging_cache::{IncrementalDecision, PackagingStamp};
use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;

#[derive(Default)]
pub struct WebPackager;

impl WebPackager {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformPackager for WebPackager {
    fn name(&self) -> &'static str {
        "Web (Trunk / wasm)"
    }

    fn target_platform(&self) -> TargetPlatform {
        TargetPlatform::Web
    }

    fn check_incremental(
        &self,
        _env: &SessionExecutionContext,
        _ctx: &PackagingContext,
    ) -> Result<(IncrementalDecision, PackagingStamp)> {
        Ok((IncrementalDecision::FullRebuild, PackagingStamp::default()))
    }

    fn package(
        &self,
        env: &SessionExecutionContext,
        _ctx: &PackagingContext,
        _decision: IncrementalDecision,
        _stamp: PackagingStamp,
    ) -> Result<PackagingOutput> {
        let web_dist = env.manifest_config().web.dist.as_ref();
        let out_dir = match web_dist {
            Some(custom_dist) => {
                if custom_dist.is_relative() {
                    env.project_root_dir().join(custom_dist)
                } else {
                    custom_dist.clone()
                }
            }
            None => env
                .project_root_dir()
                .join(".cache")
                .join("web-static")
                .join("dist"),
        };
        std::fs::create_dir_all(&out_dir)?;

        // Pre-create .cache directory before starting Trunk so Trunk's internal canonicalize
        // never errors on a missing ignore directory across Linux, macOS, and Windows.
        let cache_dir = env.project_root_dir().join(".cache");
        let _ = std::fs::create_dir_all(&cache_dir);

        let has_trunk_toml = env.project_root_dir().join("Trunk.toml").exists();
        let has_index_html = env.project_root_dir().join("index.html").exists();

        if has_trunk_toml || has_index_html {
            let trunk_bin = which::which("trunk").map_err(|_| {
                anyhow::anyhow!(
                    "Packaging web applications requires `trunk` to be installed on PATH.\n\
                     Install via:\n\
                       cargo install trunk\n\
                     Or via your host package manager:\n\
                       Linux:   cargo install trunk\n\
                       macOS:   brew install trunk\n\
                       Windows: winget install trunk.trunk\n\
                     Or ensure Trunk.toml / index.html exists in the project root."
                )
            })?;

            let mut trunk = std::process::Command::new(trunk_bin);
            trunk.current_dir(env.project_root_dir());
            trunk.env_remove("RUSTUP_TOOLCHAIN");
            trunk.arg("build");
            trunk.arg("--dist").arg(&out_dir);

            if env.build_target().optimization_profile() == TargetOpt::Release {
                trunk.arg("--release");
            }

            let status = trunk.status().context("Failed to execute trunk build")?;
            anyhow::ensure!(
                status.success(),
                "trunk build failed with exit code {:?}",
                status.code()
            );
        } else if has_index_html {
            std::fs::copy(
                env.project_root_dir().join("index.html"),
                out_dir.join("index.html"),
            )?;
        }

        let sw_src = env.project_root_dir().join("sw.js");
        if sw_src.exists() && !out_dir.join("sw.js").exists() {
            let _ = std::fs::copy(&sw_src, out_dir.join("sw.js"));
        }
        let manifest_src = env
            .project_root_dir()
            .join("assets")
            .join("manifest.webmanifest");
        let manifest_dst = out_dir.join("assets").join("manifest.webmanifest");
        if manifest_src.exists() && !manifest_dst.exists() {
            let _ = std::fs::create_dir_all(out_dir.join("assets"));
            let _ = std::fs::copy(&manifest_src, manifest_dst);
        }

        if env.build_target().optimization_profile() == TargetOpt::Release {
            precompress_distribution_directory(&out_dir)?;
        }

        Ok(PackagingOutput::single(out_dir))
    }

    fn verify(&self, artifact: &Path) -> Result<()> {
        anyhow::ensure!(
            artifact.exists(),
            "Web distribution directory was not created"
        );
        Ok(())
    }
}

pub fn precompress_file(source: &Path) -> Result<()> {
    let extension = source.extension().and_then(|s| s.to_str()).unwrap_or("");
    if !matches!(
        extension,
        "wasm" | "js" | "html" | "css" | "svg" | "json" | "webmanifest"
    ) {
        return Ok(());
    }

    let gz_target = source.with_file_name(format!(
        "{}.gz",
        source.file_name().unwrap().to_string_lossy()
    ));
    let br_target = source.with_file_name(format!(
        "{}.br",
        source.file_name().unwrap().to_string_lossy()
    ));

    // 1. Gzip compression (Maximum compression level 9)
    {
        let input = std::fs::File::open(source)
            .with_context(|| format!("Failed to open {:?} for gzip", source))?;
        let mut reader = std::io::BufReader::new(input);
        let output = std::fs::File::create(&gz_target)
            .with_context(|| format!("Failed to create {:?}", gz_target))?;
        let writer = std::io::BufWriter::new(output);
        let mut encoder = flate2::write::GzEncoder::new(writer, flate2::Compression::best());
        std::io::copy(&mut reader, &mut encoder)
            .with_context(|| format!("Failed to gzip compress {:?}", source))?;
        encoder.finish()?;
    }

    // 2. Brotli compression (Quality 11, LGWin 22)
    {
        let input = std::fs::File::open(source)
            .with_context(|| format!("Failed to open {:?} for brotli", source))?;
        let mut reader = std::io::BufReader::new(input);
        let output = std::fs::File::create(&br_target)
            .with_context(|| format!("Failed to create {:?}", br_target))?;
        let mut writer = std::io::BufWriter::new(output);
        brotli::BrotliCompress(
            &mut reader,
            &mut writer,
            &brotli::enc::BrotliEncoderParams {
                quality: 9,
                lgwin: 22,
                ..Default::default()
            },
        )
        .with_context(|| format!("Failed to brotli compress {:?}", source))?;
        writer.flush()?;
    }

    Ok(())
}

pub fn precompress_distribution_directory(dist_dir: &Path) -> Result<()> {
    if !dist_dir.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(dist_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "gz" && ext != "br" {
                precompress_file(path)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_precompress_file_generates_valid_gz_and_br() {
        let temp = tempfile::tempdir().unwrap();
        let wasm_file = temp.path().join("app.wasm");
        let payload = b"\x00asm\x01\x00\x00\x00This is a test WebAssembly binary payload with repetition repetition repetition";
        std::fs::write(&wasm_file, payload).unwrap();

        precompress_file(&wasm_file).unwrap();

        let gz_file = temp.path().join("app.wasm.gz");
        let br_file = temp.path().join("app.wasm.br");
        assert!(gz_file.exists(), "app.wasm.gz must exist");
        assert!(br_file.exists(), "app.wasm.br must exist");

        // Verify gzip roundtrip decompression
        let gz_bytes = std::fs::read(&gz_file).unwrap();
        let mut gz_decoder = flate2::read::GzDecoder::new(&gz_bytes[..]);
        let mut decompressed_gz = Vec::new();
        gz_decoder.read_to_end(&mut decompressed_gz).unwrap();
        assert_eq!(decompressed_gz, payload);

        // Verify brotli roundtrip decompression
        let br_bytes = std::fs::read(&br_file).unwrap();
        let mut br_decoder = brotli::Decompressor::new(&br_bytes[..], 4096);
        let mut decompressed_br = Vec::new();
        br_decoder.read_to_end(&mut decompressed_br).unwrap();
        assert_eq!(decompressed_br, payload);
    }

    #[test]
    fn test_precompress_skips_unsupported_extensions() {
        let temp = tempfile::tempdir().unwrap();
        let png_file = temp.path().join("image.png");
        std::fs::write(&png_file, b"fake png data").unwrap();

        precompress_file(&png_file).unwrap();

        assert!(!temp.path().join("image.png.gz").exists());
        assert!(!temp.path().join("image.png.br").exists());
    }

    #[test]
    fn test_precompress_distribution_directory_recursive() {
        let temp = tempfile::tempdir().unwrap();
        let sub_dir = temp.path().join("pkg");
        std::fs::create_dir_all(&sub_dir).unwrap();

        std::fs::write(temp.path().join("index.html"), b"<html>hello</html>").unwrap();
        std::fs::write(sub_dir.join("app.js"), b"console.log('test');").unwrap();

        precompress_distribution_directory(temp.path()).unwrap();

        assert!(temp.path().join("index.html.gz").exists());
        assert!(temp.path().join("index.html.br").exists());
        assert!(sub_dir.join("app.js.gz").exists());
        assert!(sub_dir.join("app.js.br").exists());
    }

    #[test]
    fn test_precompress_large_binary_payload() {
        let temp = tempfile::tempdir().unwrap();
        let wasm_file = temp.path().join("large_app.wasm");
        let payload: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        std::fs::write(&wasm_file, &payload).unwrap();

        precompress_file(&wasm_file).unwrap();

        let gz_file = temp.path().join("large_app.wasm.gz");
        let br_file = temp.path().join("large_app.wasm.br");
        assert!(gz_file.exists());
        assert!(br_file.exists());
        assert!(std::fs::metadata(&gz_file).unwrap().len() > 0);
        assert!(std::fs::metadata(&br_file).unwrap().len() > 0);

        let br_bytes = std::fs::read(&br_file).unwrap();
        let mut br_decoder = brotli::Decompressor::new(&br_bytes[..], 4096);
        let mut decompressed_br = Vec::new();
        br_decoder.read_to_end(&mut decompressed_br).unwrap();
        assert_eq!(decompressed_br, payload);
    }
}
