use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RuxtTutorialChapter {
    #[default]
    Windowing,
    WinitBridge,
    ImguiParadigm,
    ContextInternals,
    LayoutGeometry,
    GpuPipeline,
    RuxtJourney,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubsectionInfo {
    pub id: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
}

impl RuxtTutorialChapter {
    pub const ALL: &'static [Self] = &[
        Self::Windowing,
        Self::WinitBridge,
        Self::ImguiParadigm,
        Self::ContextInternals,
        Self::LayoutGeometry,
        Self::GpuPipeline,
        Self::RuxtJourney,
    ];

    pub fn number(&self) -> usize {
        match self {
            Self::Windowing => 1,
            Self::WinitBridge => 2,
            Self::ImguiParadigm => 3,
            Self::ContextInternals => 4,
            Self::LayoutGeometry => 5,
            Self::GpuPipeline => 6,
            Self::RuxtJourney => 7,
        }
    }

    pub fn pill_title(&self) -> &'static str {
        match self {
            Self::Windowing => "1. Windowing",
            Self::WinitBridge => "2. Bridge",
            Self::ImguiParadigm => "3. IMGUI",
            Self::ContextInternals => "4. Context",
            Self::LayoutGeometry => "5. Geometry",
            Self::GpuPipeline => "6. GPU Pipeline",
            Self::RuxtJourney => "7. Integration",
        }
    }

    pub fn full_title(&self) -> &'static str {
        match self {
            Self::Windowing => "1. The Platform Foundation: Windowing & The Event Loop",
            Self::WinitBridge => "2. The Bridge: Translating Window Events to GUI Input",
            Self::ImguiParadigm => "3. The Immediate-Mode GUI (IMGUI) Architecture",
            Self::ContextInternals => "4. The Engine Core: Deep Dive into egui::Context",
            Self::LayoutGeometry => "5. UI Geometry, Layout Engine & Custom Widgets",
            Self::GpuPipeline => "6. The Rendering Pipeline: From Shapes to Hardware Pixels",
            Self::RuxtJourney => "7. Integration in Ruxt: Multi-Platform Architecture & Testing",
        }
    }

    pub fn summary(&self) -> &'static str {
        match self {
            Self::Windowing => {
                "OS event loops, winit, ApplicationHandler, and background thread wakeup via EventLoopProxy."
            }
            Self::WinitBridge => {
                "Translating WindowEvent to RawInput, high-DPI scaling, and platform output dispatch."
            }
            Self::ImguiParadigm => {
                "Retained vs immediate mode mental models, frame execution, and 64-bit Id memory caching."
            }
            Self::ContextInternals => {
                "Interior mutability, Arc<ContextImpl>, repaint scheduling, and glyph font atlasing."
            }
            Self::LayoutGeometry => {
                "Rect algebra, cursor progression, Sense hit-testing, and custom Widget trait authoring."
            }
            Self::GpuPipeline => {
                "Shape generation, epaint anti-aliased tessellation, and egui-wgpu hardware render passes."
            }
            Self::RuxtJourney => {
                "The complete 9-stage journey, mobile/web/desktop nuances, and headless kittest testing."
            }
        }
    }

    pub fn read_time_minutes(&self) -> usize {
        match self {
            Self::Windowing => 10,
            Self::WinitBridge => 12,
            Self::ImguiParadigm => 15,
            Self::ContextInternals => 14,
            Self::LayoutGeometry => 16,
            Self::GpuPipeline => 18,
            Self::RuxtJourney => 15,
        }
    }

    pub fn subsections(&self) -> &'static [SubsectionInfo] {
        match self {
            Self::Windowing => &[
                SubsectionInfo {
                    id: "1.1",
                    title: "OS Message Loops & winit",
                    summary: "How OS window servers deliver events",
                },
                SubsectionInfo {
                    id: "1.2",
                    title: "ApplicationHandler & Threading",
                    summary: "Event loop pumping & thread safety",
                },
                SubsectionInfo {
                    id: "1.3",
                    title: "EventLoopProxy Wakeup",
                    summary: "Waking the UI thread from worker tasks",
                },
            ],
            Self::WinitBridge => &[
                SubsectionInfo {
                    id: "2.1",
                    title: "RawInput Translation",
                    summary: "Converting OS events to egui types",
                },
                SubsectionInfo {
                    id: "2.2",
                    title: "High-DPI Scale Factors",
                    summary: "Physical pixels to logical points",
                },
                SubsectionInfo {
                    id: "2.3",
                    title: "PlatformOutput Handling",
                    summary: "Cursors, IME position, and clipboard",
                },
            ],
            Self::ImguiParadigm => &[
                SubsectionInfo {
                    id: "3.1",
                    title: "Immediate vs Retained Mode",
                    summary: "The pure function f(state) -> UI",
                },
                SubsectionInfo {
                    id: "3.2",
                    title: "Frame Pass Lifecycle",
                    summary: "Input, update, and tessellation stages",
                },
                SubsectionInfo {
                    id: "3.3",
                    title: "Id Hashing & Memory Cache",
                    summary: "How stateless widgets remember state",
                },
            ],
            Self::ContextInternals => &[
                SubsectionInfo {
                    id: "4.1",
                    title: "Inside Arc<ContextImpl>",
                    summary: "Interior mutability and atomic locks",
                },
                SubsectionInfo {
                    id: "4.2",
                    title: "Repaint Scheduling & Animations",
                    summary: "Requesting immediate vs delayed frames",
                },
                SubsectionInfo {
                    id: "4.3",
                    title: "Texture Manager & Glyph Atlases",
                    summary: "Dynamic font rasterization and caching",
                },
            ],
            Self::LayoutGeometry => &[
                SubsectionInfo {
                    id: "5.1",
                    title: "Pos2, Rect & Sense Algebra",
                    summary: "Bounding boxes and interaction sensing",
                },
                SubsectionInfo {
                    id: "5.2",
                    title: "Cursor Advancement & Layouts",
                    summary: "Horizontal, vertical, and grid flow",
                },
                SubsectionInfo {
                    id: "5.3",
                    title: "Authoring Custom Widgets",
                    summary: "Implementing the egui::Widget trait",
                },
            ],
            Self::GpuPipeline => &[
                SubsectionInfo {
                    id: "6.1",
                    title: "Shape Generation & ui.painter()",
                    summary: "Emitting vector draw instructions",
                },
                SubsectionInfo {
                    id: "6.2",
                    title: "epaint Tessellation & AA",
                    summary: "Feathered anti-aliasing edge extrusion",
                },
                SubsectionInfo {
                    id: "6.3",
                    title: "egui-wgpu Render Passes",
                    summary: "Staging VBO/IBO and scissor clip rects",
                },
            ],
            Self::RuxtJourney => &[
                SubsectionInfo {
                    id: "7.1",
                    title: "The 9-Stage System Journey",
                    summary: "From hardware click to monitor photons",
                },
                SubsectionInfo {
                    id: "7.2",
                    title: "Cross-Platform Invariants",
                    summary: "Mobile 48dp, WASM RAF, Desktop multi-window",
                },
                SubsectionInfo {
                    id: "7.3",
                    title: "Automated Testing with egui_kittest",
                    summary: "Headless simulation and assertion",
                },
            ],
        }
    }

    pub fn next(&self) -> Option<Self> {
        let idx = self.number() - 1;
        Self::ALL.get(idx + 1).copied()
    }

    pub fn prev(&self) -> Option<Self> {
        let idx = self.number() - 1;
        if idx > 0 {
            Self::ALL.get(idx - 1).copied()
        } else {
            None
        }
    }

    pub fn slug(&self) -> &'static str {
        match self {
            Self::Windowing => "windowing",
            Self::WinitBridge => "bridge",
            Self::ImguiParadigm => "imgui",
            Self::ContextInternals => "context",
            Self::LayoutGeometry => "geometry",
            Self::GpuPipeline => "gpu-pipeline",
            Self::RuxtJourney => "journey",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.slug() == slug)
    }
}
