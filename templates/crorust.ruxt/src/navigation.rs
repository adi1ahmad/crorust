use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum AppMode {
    #[default]
    Home,
    Workspace,
}

impl AppMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Home => "🏠 Home & crorust Platform",
            Self::Workspace => "💻 Workspace & ruxt Framework",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CrorustChapter {
    #[default]
    Overview,
    Installation,
    Crates,
    CliCommands,
    ToolchainsAndSysroots,
    PackagingInternals,
    DevicesAndDebugging,
}

impl CrorustChapter {
    pub const ALL: &'static [Self] = &[
        Self::Overview,
        Self::Installation,
        Self::Crates,
        Self::CliCommands,
        Self::ToolchainsAndSysroots,
        Self::PackagingInternals,
        Self::DevicesAndDebugging,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Self::Overview => "Architecture Overview & 3 Tiers",
            Self::Installation => "Multi-Host Installation Guide",
            Self::Crates => "In-Tree Crates (8 Crates)",
            Self::CliCommands => "CLI Commands Reference",
            Self::ToolchainsAndSysroots => "Toolchains & Hermetic Sysroots",
            Self::PackagingInternals => "Packaging & Cryptographic Signing",
            Self::DevicesAndDebugging => "Hardware Devices & Debugging",
        }
    }

    pub fn slug(&self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Installation => "installation",
            Self::Crates => "crates",
            Self::CliCommands => "commands",
            Self::ToolchainsAndSysroots => "toolchains",
            Self::PackagingInternals => "packaging",
            Self::DevicesAndDebugging => "devices",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.slug() == slug)
    }
}

pub use crate::manual::chapter::RuxtTutorialChapter;
pub type RuxtChapter = RuxtTutorialChapter;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NavigationState {
    pub mode: AppMode,
    pub crorust_chapter: CrorustChapter,
    pub ruxt_chapter: RuxtChapter,
    #[serde(skip)]
    pub pending_hash_sync: bool,
}

impl NavigationState {
    pub fn navigate_to_mode(&mut self, mode: AppMode) {
        if self.mode != mode {
            self.mode = mode;
            self.sync_url_hash();
        }
    }

    pub fn navigate_to_crorust(&mut self, chapter: CrorustChapter) {
        self.mode = AppMode::Home;
        self.crorust_chapter = chapter;
        self.pending_hash_sync = true;
        self.sync_url_hash();
    }

    pub fn navigate_to_ruxt(&mut self, chapter: RuxtChapter) {
        self.mode = AppMode::Workspace;
        self.ruxt_chapter = chapter;
        self.pending_hash_sync = true;
        self.sync_url_hash();
    }

    pub fn current_hash(&self) -> String {
        match self.mode {
            AppMode::Home => {
                if self.crorust_chapter == CrorustChapter::Overview {
                    "#/".to_owned()
                } else {
                    format!("#/docs/crorust/{}", self.crorust_chapter.slug())
                }
            }
            AppMode::Workspace => {
                if self.ruxt_chapter == RuxtTutorialChapter::Windowing {
                    "#/workspace".to_owned()
                } else {
                    format!("#/docs/ruxt/{}", self.ruxt_chapter.slug())
                }
            }
        }
    }

    pub fn apply_hash(&mut self, hash: &str) {
        let clean = hash.trim_start_matches('#').trim_start_matches('/');
        let segments: Vec<&str> = clean.split('/').filter(|s| !s.is_empty()).collect();

        match segments.as_slice() {
            [] => {
                self.mode = AppMode::Home;
                self.crorust_chapter = CrorustChapter::Overview;
                self.pending_hash_sync = true;
            }
            ["workspace", ..] | ["showcase", ..] => self.mode = AppMode::Workspace,
            ["docs", "ruxt", slug] => {
                self.mode = AppMode::Workspace;
                if let Some(ch) = RuxtTutorialChapter::from_slug(slug) {
                    self.ruxt_chapter = ch;
                    self.pending_hash_sync = true;
                }
            }
            ["docs", "ruxt", ..] => self.mode = AppMode::Workspace,
            ["docs", "crorust", slug] => {
                self.mode = AppMode::Home;
                if let Some(ch) = CrorustChapter::from_slug(slug) {
                    self.crorust_chapter = ch;
                    self.pending_hash_sync = true;
                }
            }
            ["docs", "crorust"] => {
                self.mode = AppMode::Home;
                self.crorust_chapter = CrorustChapter::Overview;
                self.pending_hash_sync = true;
            }
            _ => {
                self.mode = AppMode::Home;
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn sync_url_hash(&self) {
        if let Some(window) = web_sys::window() {
            let target_hash = self.current_hash();
            if let Ok(location) = window.location().hash() {
                if location != target_hash {
                    let _ = window.location().set_hash(&target_hash);
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sync_url_hash(&self) {}

    #[cfg(target_arch = "wasm32")]
    pub fn poll_browser_hash(&mut self) {
        if let Some(window) = web_sys::window() {
            if let Ok(hash) = window.location().hash() {
                if !hash.is_empty() && hash != self.current_hash() {
                    self.apply_hash(&hash);
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn poll_browser_hash(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_slug_roundtrip() {
        let mut nav = NavigationState::default();
        nav.apply_hash("#/docs/crorust/crates");
        assert_eq!(nav.mode, AppMode::Home);
        assert_eq!(nav.crorust_chapter, CrorustChapter::Crates);
        assert_eq!(nav.current_hash(), "#/docs/crorust/crates");

        nav.apply_hash("#/docs/ruxt/gpu-pipeline");
        assert_eq!(nav.mode, AppMode::Workspace);
        assert_eq!(nav.ruxt_chapter, RuxtTutorialChapter::GpuPipeline);
        assert_eq!(nav.current_hash(), "#/docs/ruxt/gpu-pipeline");

        nav.apply_hash("#/showcase");
        assert_eq!(nav.mode, AppMode::Workspace);

        nav.apply_hash("#/");
        assert_eq!(nav.mode, AppMode::Home);
        assert_eq!(nav.current_hash(), "#/");
    }
}
