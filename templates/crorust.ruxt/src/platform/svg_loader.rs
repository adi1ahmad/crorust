//! Universal Cross-Platform SVG Image Loader with Hermetic Font Engine
//!
//! Provides guaranteed vector graphic rendering and typography layout across Desktop (Linux, macOS, Windows),
//! Mobile (Android, iOS), and WebAssembly (WASM).
//!
//! ### The Problem Solved
//! `egui_extras::loaders::SvgLoader` relies exclusively on `fontdb::Database::load_system_fonts()`.
//! On WebAssembly (`wasm32-unknown-unknown`), there is no host filesystem, resulting in 0 loaded fonts
//! and blank text inside SVGs. Furthermore, on platforms lacking fontconfig aliases, generic CSS families
//! like `sans-serif` default to empty strings in `fontdb`, triggering `usvg` font matching warnings.
//!
//! ### The Solution
//! `UniversalSvgLoader` hermetically injects `epaint`'s statically embedded fonts (`Ubuntu-Light`, `Hack`)
//! into `fontdb` at startup, explicitly binds `Family::SansSerif` to `Ubuntu`, and supplements with host
//! system fonts on native targets.

use std::collections::HashMap;
use std::sync::Arc;

use egui::ColorImage;
use egui::load::{BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint};
use egui::mutex::Mutex;

struct CacheEntry {
    last_used: u64,
    result: Result<Arc<ColorImage>, String>,
}

struct LoaderState {
    pass_index: u64,
    cache: HashMap<String, HashMap<SizeHint, CacheEntry>>,
    options: resvg::usvg::Options<'static>,
}

/// Universal SVG image loader with built-in font database configuration.
pub struct UniversalSvgLoader {
    state: Mutex<LoaderState>,
}

impl UniversalSvgLoader {
    pub const ID: &'static str = egui::generate_loader_id!(UniversalSvgLoader);

    /// Creates a new `UniversalSvgLoader` with hermetically populated fonts.
    #[must_use]
    pub fn new() -> Self {
        let mut options = resvg::usvg::Options::default();

        // 1. Ingest embedded fonts from epaint to guarantee 100% offline, cross-platform
        // font availability across Web (WASM), Android, iOS, Windows, macOS, and Linux
        // without increasing application binary size:
        let default_fonts = egui::FontDefinitions::default();

        if let Some(font_data) = default_fonts.font_data.get("Ubuntu-Light") {
            options.fontdb_mut().load_font_data(font_data.font.to_vec());
            options.fontdb_mut().set_sans_serif_family("Ubuntu");
            options.fontdb_mut().set_serif_family("Ubuntu");
        }

        if let Some(mono_data) = default_fonts.font_data.get("Hack") {
            options.fontdb_mut().load_font_data(mono_data.font.to_vec());
            options.fontdb_mut().set_monospace_family("Hack");
        }

        // 2. Supplement with host OS system fonts on native desktop & mobile targets:
        #[cfg(not(target_arch = "wasm32"))]
        options.fontdb_mut().load_system_fonts();

        Self {
            state: Mutex::new(LoaderState {
                pass_index: 0,
                cache: HashMap::default(),
                options,
            }),
        }
    }
}

impl Default for UniversalSvgLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageLoader for UniversalSvgLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &egui::Context, uri: &str, size_hint: SizeHint) -> ImageLoadResult {
        if !egui::load::has_extension(uri, "svg") {
            return Err(LoadError::NotSupported);
        }

        let mut state = self.state.lock();
        let LoaderState {
            pass_index,
            cache,
            options,
        } = &mut *state;

        let bucket = cache.entry(uri.to_owned()).or_default();

        if let Some(entry) = bucket.get_mut(&size_hint) {
            entry.last_used = *pass_index;
            return match entry.result.clone() {
                Ok(image) => Ok(ImagePoll::Ready { image }),
                Err(err) => Err(LoadError::Loading(err)),
            };
        }

        match ctx.try_load_bytes(uri) {
            Ok(BytesPoll::Ready { bytes, .. }) => {
                let sanitized_bytes = sanitize_svg_bytes(&bytes);
                let result = egui_extras::image::load_svg_bytes_with_size(
                    &sanitized_bytes,
                    size_hint,
                    options,
                )
                .map(Arc::new);

                bucket.insert(
                    size_hint,
                    CacheEntry {
                        last_used: *pass_index,
                        result: result.clone(),
                    },
                );

                match result {
                    Ok(image) => Ok(ImagePoll::Ready { image }),
                    Err(err) => Err(LoadError::Loading(err)),
                }
            }
            Ok(BytesPoll::Pending { size }) => Ok(ImagePoll::Pending { size }),
            Err(err) => Err(err),
        }
    }

    fn forget(&self, uri: &str) {
        self.state.lock().cache.retain(|key, _| key != uri);
    }

    fn forget_all(&self) {
        self.state.lock().cache.clear();
    }

    fn byte_size(&self) -> usize {
        self.state
            .lock()
            .cache
            .values()
            .flat_map(|bucket| bucket.values())
            .map(|entry| match &entry.result {
                Ok(image) => image.pixels.len() * std::mem::size_of::<egui::Color32>(),
                Err(err) => err.len(),
            })
            .sum()
    }

    fn end_pass(&self, pass_index: u64) {
        const MAX_CACHE_BYTES: usize = 48 * 1024 * 1024; // 48 MiB safety boundary
        const STALE_PASS_THRESHOLD: u64 = 4;

        let mut state = self.state.lock();
        state.pass_index = pass_index;

        // 1. Evict temporary zoom variants within each bucket
        for bucket in state.cache.values_mut() {
            if bucket.len() >= 2 {
                bucket.retain(|_, entry| pass_index <= entry.last_used + 1);
            }
        }

        // 2. If memory quota is exceeded, purge stale buckets that haven't been used in recent frames
        let current_bytes: usize = state
            .cache
            .values()
            .flat_map(|b| b.values())
            .map(|entry| match &entry.result {
                Ok(img) => img.pixels.len() * std::mem::size_of::<egui::Color32>(),
                Err(err) => err.len(),
            })
            .sum();

        if current_bytes > MAX_CACHE_BYTES {
            state.cache.retain(|_uri, bucket| {
                bucket.retain(|_, entry| pass_index <= entry.last_used + STALE_PASS_THRESHOLD);
                !bucket.is_empty()
            });
        }

        // 3. Drop empty buckets
        state.cache.retain(|_, bucket| !bucket.is_empty());
    }
}

/// In-memory sanitizer removing un-dimensioned `<rect>` tags to avoid `usvg` warnings.
/// Robust against attribute ordering, whitespace differences, and trailing semicolons.
pub fn sanitize_svg_bytes(bytes: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    if !bytes.windows(5).any(|w| w == b"<rect") {
        return std::borrow::Cow::Borrowed(bytes);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => {
            let mut cleaned = String::with_capacity(s.len());
            let mut cursor = 0;

            while let Some(pos) = s[cursor..].find("<rect") {
                let tag_start = cursor + pos;
                cleaned.push_str(&s[cursor..tag_start]);

                if let Some(tag_end_rel) = s[tag_start..].find('>') {
                    let tag_end = tag_start + tag_end_rel + 1;
                    let tag_content = &s[tag_start..tag_end];

                    // Check if tag is an empty placeholder or stroke-none background rect
                    let trimmed = tag_content.trim();
                    let is_empty_rect = trimmed == "<rect/>"
                        || (trimmed.starts_with("<rect")
                            && trimmed.ends_with("/>")
                            && trimmed[5..trimmed.len() - 2].trim().is_empty());

                    let is_stroke_none = tag_content.contains("stroke:none")
                        || tag_content.contains("stroke: none")
                        || tag_content.contains("stroke: none;");

                    let is_background = tag_content.contains("class=\"background\"")
                        || tag_content.contains("class='background'");

                    let lacks_dimensions =
                        !tag_content.contains("width=") || !tag_content.contains("height=");

                    if is_empty_rect || ((is_stroke_none || is_background) && lacks_dimensions) {
                        // Skip appending this placeholder rect tag
                        cursor = tag_end;
                        continue;
                    }

                    cleaned.push_str(tag_content);
                    cursor = tag_end;
                } else {
                    cleaned.push_str(&s[tag_start..]);
                    cursor = s.len();
                    break;
                }
            }

            if cursor < s.len() {
                cleaned.push_str(&s[cursor..]);
            }

            if cleaned.len() == s.len() {
                std::borrow::Cow::Borrowed(bytes)
            } else {
                std::borrow::Cow::Owned(cleaned.into_bytes())
            }
        }
        _ => std::borrow::Cow::Borrowed(bytes),
    }
}
