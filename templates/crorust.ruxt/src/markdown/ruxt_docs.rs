//! ruxt Framework Documentation Catalog
//!
//! Formatted EasyMark chapters providing the official developer guide
//! and living architecture manual for building cross-platform applications with ruxt.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum RuxtDocChapter {
    #[default]
    Overview,
    Lifecycle,
    Graphics,
    UiSystem,
    Theming,
}

impl RuxtDocChapter {
    pub const ALL: &'static [(RuxtDocChapter, &'static str, &'static str)] = &[
        (RuxtDocChapter::Overview, "📐 Architecture & Mental Model", "overview"),
        (RuxtDocChapter::Lifecycle, "🔄 App Creation & winit Lifecycle", "lifecycle"),
        (RuxtDocChapter::Graphics, "🎲 wgpu, glow & glam Graphics", "graphics"),
        (RuxtDocChapter::UiSystem, "⚡ Immediate-Mode UI & Layout", "ui-system"),
        (RuxtDocChapter::Theming, "🎨 Theming & High-DPI Scaling", "theming"),
    ];

    pub fn all() -> &'static [(RuxtDocChapter, &'static str, &'static str)] {
        Self::ALL
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Overview => "Architecture & Mental Model",
            Self::Lifecycle => "App Creation & winit Lifecycle",
            Self::Graphics => "wgpu, glow & glam Graphics Pipeline",
            Self::UiSystem => "Immediate-Mode UI & Layout Mechanics",
            Self::Theming => "Theming, Custom Palettes & High-DPI Scaling",
        }
    }

    pub fn target_window(&self) -> Option<&'static str> {
        match self {
            Self::Overview => Some("About crorust.ruxt"),
            Self::Lifecycle => Some("Window Resize Test"),
            Self::Graphics => Some("🎲 3D Viewport"),
            Self::UiSystem => Some("📦 Widget Showcase"),
            Self::Theming => Some("🎨 Display Calibration"),
        }
    }

    pub fn markdown_content(&self) -> &'static str {
        match self {
            Self::Overview => CHAPTER_OVERVIEW,
            Self::Lifecycle => CHAPTER_LIFECYCLE,
            Self::Graphics => CHAPTER_GRAPHICS,
            Self::UiSystem => CHAPTER_UI_SYSTEM,
            Self::Theming => CHAPTER_THEMING,
        }
    }
}

const CHAPTER_OVERVIEW: &str = r#"
# 📐 Architecture & Mental Model

Welcome to *ruxt*, the modern, immediate-mode cross-platform GUI framework written in 100% pure Rust.

----------------

# 1. Retained-Mode vs Immediate-Mode

Most traditional UI toolkits (HTML DOM, Qt, Flutter, SwiftUI) are *retained-mode*. In retained-mode:
- You construct a tree of long-lived widget objects (buttons, panels, text views).
- When application state changes, you find those objects and update their properties.
- State exists in two places: your business logic and the UI tree, requiring two-way binding, change notifications, and complex synchronization.

In *immediate-mode* (the model powering egui and ruxt):
- There is no persistent widget tree.
- On every single frame (typically 60 or 120 times per second), your UI code runs from top to bottom.
- You declare what should be drawn right now based directly on current application state.
- If an item no longer exists in your state array, you simply do not call its drawing function; it vanishes instantly from the screen without memory cleanup or event unhooking.

```
+-------------------------------------------------------------+
|                     APPLICATION STATE                       |
|   (Pure Rust structs, vectors, enums, mutexes, atomics)     |
+-------------------------------------------------------------+
                              |
                              v  Every frame (eframe::App::ui)
+-------------------------------------------------------------+
|                      UI DECLARATION                         |
|   ui.heading("My App");                                     |
|   if ui.button("Click Me").clicked() { count += 1; }        |
+-------------------------------------------------------------+
                              |
                              v  Tessellation pass
+-------------------------------------------------------------+
|                     RENDER MESHES / GPU                     |
|   Vertex buffers, index buffers, clipping rectangles        |
+-------------------------------------------------------------+
```

----------------

# 2. Key Framework Components

- *egui*: The core pure-Rust immediate-mode GUI engine. Calculates layouts, handles text shaping, and tessellates primitives into GPU triangles.
- *eframe*: Official application framework wrapper that embeds egui into native operating system windows (via winit) and WebAssembly (via web-sys).
- *wgpu & glow*: Hardware-accelerated graphics pipelines (DirectX 12, Metal, Vulkan, OpenGL ES, WebGPU/WebGL).
- *crorust*: Hermetic build toolchain and format packager that compiles and bundles your application into native APK, AAB, DMG, MSIX, and AppImage artifacts without external IDEs.

----------------

# 3. State Management & Persistence

Immediate-mode does not mean stateless:
1. *Application State*: Stored in your root `App` struct. You control when and how it mutates.
2. *Transient Widget State*: egui maintains a lightweight internal `Memory` store keyed by unique `Id` values (e.g. cursor positions, scroll offsets, open/closed collapse headers).
3. *Cross-Session Persistence*: When the `persistence` feature is enabled, ruxt automatically serializes window layouts and settings to disk using `serde` (or to `localStorage` on WebAssembly).
"#;

const CHAPTER_LIFECYCLE: &str = r#"
# 🔄 App Creation & winit Lifecycle

Understanding how ruxt boots, creates native OS windows, and processes hardware events.

----------------

# 1. Native Desktop Boot Flow

On Linux (Wayland/X11), macOS, and Windows, execution starts in `src/main.rs`:

```rust
fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([380.0, 480.0])
            .with_title("ruxt application"),
        ..Default::default()
    };
    
    eframe::run_native(
        "crorust.ruxt",
        native_options,
        Box::new(|cc| Ok(Box::new(ruxt::RuxtApp::new(cc)))),
    )
}
```

1. *eframe::NativeOptions*: Configures initial window geometry, transparency, resizability, and GPU backends.
2. *CreationContext (cc)*: Passes access to the egui context, persistent storage handle, and integration hooks.
3. *Event Loop*: `eframe` hands control to `winit`. winit drives the OS message pump, translating mouse, keyboard, touch, and resize events into `egui::RawInput`.

----------------

# 2. WebAssembly (Browser) Lifecycle

In WebAssembly targets (`wasm32-unknown-unknown`):
- Execution begins via `wasm_bindgen` in `src/web.rs`.
- `eframe::WebRunner::new()` attaches an HTML5 `<canvas id="the_canvas_id">` to the browser DOM.
- The browser's native `requestAnimationFrame` loop drives UI updates, synchronizing frame rate with display refresh intervals.
- Canvas resizing is tracked dynamically via browser `ResizeObserver`.

----------------

# 3. Mobile Lifecycle (Android & iOS)

- *Android*: Driven by `android-activity` (`GameActivity`). Lifecycle transitions (`onPause`, `onResume`, `onDestroy`) are converted into winit suspension events. Soft keyboard input is routed through Android IME bridges.
- *iOS*: Window surface is bound via `UIKit` and Metal graphics layers, supporting high-refresh ProMotion displays and touch gestures.
"#;

const CHAPTER_GRAPHICS: &str = r#"
# 🎲 wgpu, glow & glam Graphics Pipeline

ruxt combines immediate-mode GUI with a high-performance 3D graphics pipeline.

----------------

# 1. Dual Backend Strategy

Different target platforms require different GPU capabilities. ruxt supports two complementary backends:

- *wgpu (Modern / Default)*:
  - Built on WebGPU, Vulkan, Metal, and DirectX 12.
  - Asynchronous pipeline creation and native compute shader capabilities.
  - Used on modern desktop (Linux, macOS, Windows) and WebGPU-enabled browsers.

- *glow (Universal Fallback)*:
  - Lightweight pure-Rust OpenGL ES 2.0 / 3.0 and WebGL 1 / 2 bindings.
  - Ensures compatibility on older hardware, virtual machines, and legacy browsers without WebGPU.

----------------

# 2. The 3D Viewport Custom Painter

ruxt demonstrates custom 3D mesh rendering rendered directly inside an egui window using `eframe::egui_glow` and `glam`:

```rust
// glam provides SIMD-accelerated math for 3D transforms:
use glam::{Mat4, Vec3, Quat};

let proj = Mat4::perspective_rh(fov, aspect_ratio, 0.1, 100.0);
let view = Mat4::look_at_rh(camera_pos, target, Vec3::Y);
let model = Mat4::from_rotation_y(rotation_angle);
let mvp = proj * view * model;
```

1. *Paint Callback*: In your window's `ui()` pass, call `ui.painter().add(egui::PaintCallback { ... })`.
2. *State Isolation*: egui safely restores GPU blend modes, viewport scissors, and shader state before and after custom callbacks.
3. *Zero Overhead*: Geometry shares the same GPU context as the UI controls without context switches.
"#;

const CHAPTER_UI_SYSTEM: &str = r#"
# ⚡ Immediate-Mode UI & Layout Mechanics

A developer guide to layout containers, widgets, and interactivity in ruxt.

----------------

# 1. Layout Containers

egui provides structured layout builders to organize your interface:

- *ui.horizontal(|ui| ...)*: Arranges widgets from left to right.
- *ui.vertical(|ui| ...)*: Stacks widgets top to bottom.
- *ui.columns(n, |cols| ...)*: Divides horizontal space into `n` equal columns.
- *egui::ScrollArea*: Handles vertical or horizontal scrolling with virtualized rendering for large lists.
- *egui::Grid*: Multi-row, multi-column aligned layouts with automatic cell sizing.
- *egui::Window*: Floating, movable, resizable, and collapsable workspace surfaces.

----------------

# 2. The Widget Flow: `ui.add` and `Response`

Every widget in egui implements the `egui::Widget` trait and returns an `egui::Response`:

```rust
let response = ui.add(egui::Button::new("Save File"));

if response.clicked() {
    println!("File saved!");
}
if response.hovered() {
    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
}
```

The `Response` struct provides complete interaction metadata:
- `.clicked()`, `.double_clicked()`, `.secondary_clicked()`
- `.hovered()`, `.has_focus()`
- `.dragged()`, `.drag_delta()`
- `.rect`: Exact screen-space bounding box of the widget.

----------------

# 3. Custom Widget Authoring

Creating your own interactive widget is straightforward:

```rust
pub struct RatingStars<'a> {
    rating: &'a mut usize,
    max_stars: usize,
}

impl<'a> egui::Widget for RatingStars<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, mut response) = ui.allocate_exact_size(
            egui::vec2(self.max_stars as f32 * 24.0, 24.0),
            egui::Sense::click(),
        );
        // Paint stars using ui.painter() and update *self.rating on click...
        response
    }
}
```
"#;

const CHAPTER_THEMING: &str = r#"
# 🎨 Theming, Custom Palettes & High-DPI Scaling

Crafting refined, accessible visual presentations across varied display hardware.

----------------

# 1. Visuals & Design Tokens

ruxt supports dynamic dark and light mode styling via `egui::Visuals`:

- *Dark Palette*: Deep zinc background (`#121316`), elevated window frames (`#1A1B1E`), and soft high-contrast borders (`#2D3139`).
- *Light Palette*: Crisp modern paper tones with subtle drop shadows.
- *Accent Tones*: Primary brand blue (`#1F6FEB`) used for active selection pills, focus rings, and primary action buttons.

```rust
let mut visuals = egui::Visuals::dark();
visuals.window_fill = egui::Color32::from_rgb(26, 27, 30);
visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 49, 57));
ui.ctx().set_visuals(visuals);
```

----------------

# 2. High-DPI & Retina Display Scaling

Modern displays feature vastly different physical pixel densities (from 96 DPI monitors to 400+ DPI smartphone screens):

- egui separates *points* (logical coordinates used in your layout code) from *pixels* (physical hardware display pixels).
- The ratio between points and pixels is `pixels_per_point`.
- On high-DPI screens, fonts and vector icons scale cleanly with subpixel anti-aliasing.
- Users can adjust UI scale at runtime with `ui.ctx().set_pixels_per_point(scale)`.

----------------

# 3. Accessibility via AccessKit

When the `accessibility` feature is enabled:
- ruxt integrates with `AccessKit` to provide semantic accessibility trees directly to operating system screen readers (VoiceOver, NVDA, Orca).
- Buttons, labels, text fields, and sliders automatically expose their semantic roles, values, and focus state.
"#;
