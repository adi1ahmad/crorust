use anyhow::{Context, Result};
use image::imageops::FilterType;
use image::io::Reader as ImageReader;
use image::{DynamicImage, ImageOutputFormat, RgbaImage};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub struct RasterResizer {
    source_path: PathBuf,
    raw_bytes: Vec<u8>,
    source_hash: String,
    decoded_cache: OnceLock<DynamicImage>,
    optimized: bool,
    cache_dir: Option<PathBuf>,
}

impl RasterResizer {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let raw_bytes = std::fs::read(path).with_context(|| {
            format!(
                "RasterResizer failed reading source asset `{}`",
                path.display()
            )
        })?;
        let source_hash = format!("{:x}", Sha256::digest(&raw_bytes));

        let reader = ImageReader::new(Cursor::new(&raw_bytes))
            .with_guessed_format()
            .with_context(|| {
                format!(
                    "RasterResizer failed detecting format of `{}`",
                    path.display()
                )
            })?;
        let (w, h) = reader.into_dimensions().with_context(|| {
            format!(
                "RasterResizer unable to probe dimensions of `{}`",
                path.display()
            )
        })?;
        anyhow::ensure!(w == h, "Application icon must be square (received {w}x{h})");
        anyhow::ensure!(
            w >= 512,
            "Application icon must be at least 512x512 pixels (received {w}x{h})"
        );

        Ok(Self {
            source_path: path.to_path_buf(),
            raw_bytes,
            source_hash,
            decoded_cache: OnceLock::new(),
            optimized: false,
            cache_dir: None,
        })
    }

    pub fn with_cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(dir.into());
        self
    }

    pub fn set_cache_dir(&mut self, dir: Option<PathBuf>) {
        self.cache_dir = dir;
    }

    pub fn source_hash(&self) -> &str {
        &self.source_hash
    }

    fn compact_pixel_depth(img: &mut DynamicImage) {
        let rgba = img.to_rgba8();
        let mut uniform_gray = true;
        let mut full_opacity = true;

        for &[r, g, b, a] in rgba.as_chunks::<4>().0 {
            if r != g || g != b {
                uniform_gray = false;
            }
            if a != 255 {
                full_opacity = false;
            }
            if !uniform_gray && !full_opacity {
                return;
            }
        }

        *img = match (uniform_gray, full_opacity) {
            (true, true) => DynamicImage::ImageLuma8(img.to_luma8()),
            (true, false) => DynamicImage::ImageLumaA8(img.to_luma_alpha8()),
            (false, true) => DynamicImage::ImageRgb8(img.to_rgb8()),
            (false, false) => return,
        };
    }

    fn with_decoded_image<F, R>(&self, action: F) -> Result<R>
    where
        F: FnOnce(&DynamicImage) -> Result<R>,
    {
        if let Some(decoded) = self.decoded_cache.get() {
            return action(decoded);
        }

        let mut decoded = ImageReader::new(Cursor::new(&self.raw_bytes))
            .with_guessed_format()
            .with_context(|| {
                format!(
                    "RasterResizer failed identifying image format for `{}`",
                    self.source_path.display()
                )
            })?
            .decode()
            .with_context(|| {
                format!(
                    "RasterResizer failed decoding bitmap at `{}`",
                    self.source_path.display()
                )
            })?;

        if self.optimized {
            Self::compact_pixel_depth(&mut decoded);
        }

        let _ = self.decoded_cache.set(decoded);
        action(
            self.decoded_cache
                .get()
                .expect("decoded image must be cached"),
        )
    }

    pub fn encode_optimized_png(&mut self) {
        if !self.optimized {
            self.optimized = true;
            if let Some(img) = self.decoded_cache.get_mut() {
                Self::compact_pixel_depth(img);
            }
        }
    }

    fn resolve_cache_artifact_path(&self, spec: TransformConstraint) -> Option<PathBuf> {
        let base = self.cache_dir.as_ref()?;
        let filter_name = match spec.filter {
            FilterType::Nearest => "nearest",
            FilterType::Triangle => "triangle",
            FilterType::CatmullRom => "catmullrom",
            FilterType::Gaussian => "gaussian",
            FilterType::Lanczos3 => "lanczos3",
        };
        let mode_suffix = if self.optimized { "opt" } else { "raw" };
        let artifact_name = format!(
            "w{}_h{}_s{}_{}_{}.png",
            spec.target_width, spec.target_height, spec.scaled_size, filter_name, mode_suffix
        );
        Some(base.join(&self.source_hash).join(artifact_name))
    }

    pub fn generate_resampled_variant<W: Write + Seek>(
        &self,
        writer: &mut W,
        spec: TransformConstraint,
    ) -> Result<()> {
        if let Some(cached_target) = self.resolve_cache_artifact_path(spec) {
            if cached_target.is_file() {
                if let Ok(bytes) = std::fs::read(&cached_target) {
                    writer.write_all(&bytes)?;
                    return Ok(());
                }
            }

            let mut encoded = Vec::new();
            {
                let mut sink = Cursor::new(&mut encoded);
                self.render_uncached(&mut sink, spec)?;
            }

            if let Some(parent) = cached_target.parent() {
                let _ = std::fs::create_dir_all(parent);
                let tmp_file = parent.join(format!(
                    ".resample-{}-{}",
                    std::process::id(),
                    cached_target
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("variant.png")
                ));
                if std::fs::write(&tmp_file, &encoded).is_ok() {
                    let _ = std::fs::rename(&tmp_file, &cached_target);
                }
            }

            writer.write_all(&encoded)?;
            return Ok(());
        }

        self.render_uncached(writer, spec)
    }

    fn render_uncached<W: Write + Seek>(
        &self,
        writer: &mut W,
        spec: TransformConstraint,
    ) -> Result<()> {
        self.with_decoded_image(|source| {
            let resampled = source.resize(spec.scaled_size, spec.scaled_size, spec.filter);
            if spec.scaled_size == spec.target_width && spec.scaled_size == spec.target_height {
                resampled.write_to(writer, ImageOutputFormat::Png)?;
            } else {
                let offset_x = (spec.target_width.saturating_sub(spec.scaled_size)) / 2;
                let offset_y = (spec.target_height.saturating_sub(spec.scaled_size)) / 2;
                let mut surface = RgbaImage::new(spec.target_width, spec.target_height);
                image::imageops::overlay(
                    &mut surface,
                    &resampled,
                    offset_x as i64,
                    offset_y as i64,
                );
                surface.write_to(writer, ImageOutputFormat::Png)?;
            }
            Ok(())
        })
    }

    pub fn write_variant<W: Write + Seek>(
        &self,
        writer: &mut W,
        spec: TransformConstraint,
    ) -> Result<()> {
        self.generate_resampled_variant(writer, spec)
    }

    pub fn render_to_buffer(&self, spec: TransformConstraint) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        let mut cursor = Cursor::new(&mut output);
        self.generate_resampled_variant(&mut cursor, spec)?;
        Ok(output)
    }

    pub fn optimize(&mut self) {
        self.encode_optimized_png();
    }

    pub fn write<W: Write + Seek>(&self, w: &mut W, opts: TransformConstraint) -> Result<()> {
        self.generate_resampled_variant(w, opts)
    }

    pub fn try_to_vec(&self, opts: TransformConstraint) -> Result<Vec<u8>> {
        self.render_to_buffer(opts)
    }

    pub fn to_vec(&self, opts: TransformConstraint) -> Vec<u8> {
        self.render_to_buffer(opts).expect("render failed")
    }

    /// Renders multiple icon constraints in parallel across Rayon worker threads.
    pub fn render_batch_parallel(
        &self,
        constraints: &[TransformConstraint],
    ) -> Result<Vec<(TransformConstraint, Vec<u8>)>> {
        constraints
            .par_iter()
            .map(|&spec| {
                let bytes = self.render_to_buffer(spec)?;
                Ok((spec, bytes))
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResampleConfigurationBuilder {
    base_width: u32,
    base_height: u32,
    scaling_ratio: f32,
    inset_fraction: f32,
    sampling_filter: FilterType,
}

impl ResampleConfigurationBuilder {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            base_width: width,
            base_height: height,
            scaling_ratio: 1.0,
            inset_fraction: 0.0,
            sampling_filter: FilterType::Lanczos3,
        }
    }

    pub fn with_scale_factor(mut self, factor: f32) -> Self {
        self.scaling_ratio = factor;
        self
    }

    pub fn with_inset_padding(mut self, fraction: f32) -> Self {
        self.inset_fraction = fraction;
        self
    }

    pub fn with_sampling_filter(mut self, filter: FilterType) -> Self {
        self.sampling_filter = filter;
        self
    }

    pub fn construct_constraint(self) -> TransformConstraint {
        let outer_w = (self.base_width as f32 * self.scaling_ratio).round() as u32;
        let outer_h = (self.base_height as f32 * self.scaling_ratio).round() as u32;
        let square_dim = outer_w.min(outer_h);
        let safe_inset =
            (square_dim as f32 * (1.0 - self.inset_fraction.clamp(0.0, 0.95))).round() as u32;

        TransformConstraint {
            target_width: outer_w,
            target_height: outer_h,
            scaled_size: safe_inset.max(1),
            filter: self.sampling_filter,
        }
    }

    pub fn padding(self, fraction: f32) -> Self {
        self.with_inset_padding(fraction)
    }

    pub fn scale(self, factor: f32) -> Self {
        self.with_scale_factor(factor)
    }

    pub fn filter(self, filter: FilterType) -> Self {
        self.with_sampling_filter(filter)
    }

    pub fn build(self) -> TransformConstraint {
        self.construct_constraint()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformConstraint {
    pub target_width: u32,
    pub target_height: u32,
    pub scaled_size: u32,
    pub filter: FilterType,
}

impl TransformConstraint {
    pub fn square(dimension: u32) -> Self {
        Self {
            target_width: dimension,
            target_height: dimension,
            scaled_size: dimension,
            filter: FilterType::Lanczos3,
        }
    }

    pub fn new(size: u32) -> Self {
        Self::square(size)
    }

    pub fn with_filter(mut self, filter: FilterType) -> Self {
        self.filter = filter;
        self
    }

    pub fn filter(&self) -> FilterType {
        self.filter
    }

    pub fn target_width(&self) -> u32 {
        self.target_width
    }

    pub fn target_height(&self) -> u32 {
        self.target_height
    }

    pub fn scaled_size(&self) -> u32 {
        self.scaled_size
    }
}
