use crate::manual::widgets::ManualPlaygroundState;
use egui::{Color32, CornerRadius, RichText, Stroke, Ui, vec2};

pub fn render_chapter(
    ui: &mut Ui,
    state: &mut ManualPlaygroundState,
    target_to_launch: &mut Option<&'static str>,
) {
    // 1. Chapter Header Banner
    ui.heading(
        RichText::new("Chapter 4: The Engine Core: Deep Dive into egui::Context")
            .size(24.0)
            .strong(),
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "Explore the central coordinator, Arc<ContextImpl> interior mutability, multithreaded \
             worker wakeups, repaint scheduling, dynamic glyph atlases, and AccessKit accessibility.",
        )
        .size(13.5)
        .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    // 2. High-Level Concept Diagram (SVG)
    let viewer = crate::manual::widgets::diagram_viewer::InteractiveDiagramViewer::new(
        "ch04_context_internals",
        egui::include_image!("../../../assets/manual/svg/ch04_context_internals.svg"),
        include_str!("../../../assets/manual/diagrams/ch04_context_internals.mmd"),
        egui::vec2(1468.03, 1047.20),
    );
    viewer.show(ui, state.diagram_state_mut("ch04_context_internals"));
    ui.add_space(16.0);

    // 3. Subsections
    render_subsection_4_1(ui);
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(20.0);

    render_subsection_4_2(ui);
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(20.0);

    render_subsection_4_3(ui);
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(20.0);

    render_subsection_4_4(ui);
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(20.0);

    // 4. Interactive Lab: Visuals & Style Theming Sandbox
    render_style_sandbox(ui, state, target_to_launch);
}

fn render_subsection_4_1(ui: &mut Ui) {
    ui.heading(
        RichText::new("4.1 What is egui::Context? & Interior Mutability")
            .size(18.0)
            .strong(),
    );
    ui.add_space(8.0);

    ui.label(
        "egui::Context is the central coordinator of the entire application. It manages inputs, \
         persists widget interaction state across frames, maintains visual themes, rasterizes font glyphs, \
         allocates GPU textures, and signals redraws to the platform event loop.",
    );

    ui.add_space(10.0);
    ui.label(
        RichText::new("Topology: Arc<RwLock<ContextImpl>>")
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(
        "Context is not a monolithic struct passed by mutable reference (&mut Context). Instead, \
         it is a lightweight, O(1) cheaply cloneable pointer wrapping an Arc<RwLock<ContextImpl>>.\n\n\
         Inside ContextImpl, fine-grained reader-writer locks (parking_lot::RwLock) synchronize access \
         to independent engine subsystems:\n\
         • InputState: Current and previous mouse positions, pressed keys, multi-touch pinch/pan.\n\
         • Memory: Persistent widget caches, focus chain, layer order, floating window geometries.\n\
         • Style & Visuals: Dark/Light color palettes, corner rounding, margins, text styles.\n\
         • Fonts: TrueType/OpenType font rasterizer and UV glyph texture atlas.\n\
         • TextureManager: Dynamic GPU textures, SVG cache, and lifetime tracking.\n\
         • RepaintQueue: Atomic thread wakeup broadcaster.",
    );

    ui.add_space(10.0);
    ui.label(
        RichText::new("Cross-Thread Safety & Worker Wakeups")
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(
        "Because Context implements Send + Sync, asynchronous Tokio tasks or background worker threads \
         can hold their own clone of Context. When background I/O or computation completes, the worker invokes \
         ctx.request_repaint(). This triggers the platform repaint callback, writing to an OS wakeup primitive \
         (eventfd on Linux, PostMessageW on Windows, CFRunLoopWakeUp on macOS), waking the main UI thread immediately.",
    );

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"// Spawning background Tokio task with a cloned Context:
let ctx_clone = ctx.clone();
tokio::spawn(async move {
    let payload = fetch_network_data().await;
    *DATA_STORE.lock() = Some(payload);
    
    // Atomically signal the OS event loop to wake Thread 0 and redraw!
    ctx_clone.request_repaint();
});"#,
    );
}

fn render_subsection_4_2(ui: &mut Ui) {
    ui.heading(
        RichText::new("4.2 Repaint Scheduling & Frame Throttling")
            .size(18.0)
            .strong(),
    );
    ui.add_space(8.0);

    ui.label(
        "To balance battery life against fluid motion, egui provides two distinct redraw signaling mechanisms:",
    );

    ui.add_space(8.0);
    ui.label(
        "• Immediate Redraws (ctx.request_repaint()): Sets an internal atomic flag and invokes the platform \
         wakeup callback. Used when user input arrives or background data finishes loading.\n\
         • Paced Redraws (ctx.request_repaint_after(Duration)): Schedules a wakeup after a specified duration. \
         Essential for smooth 60 FPS animations (16.6ms intervals), blinking text cursors (500ms intervals), \
         and physics simulations.",
    );

    ui.add_space(10.0);
    ui.label(
        RichText::new("Repaint Delay Calculation")
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(
        "During each frame pass, ContextImpl aggregates the lowest requested repaint delay across all viewports. \
         The emitted FullOutput delivers this delay to the platform event loop:\n\
         • delay == Duration::ZERO -> Event loop cycles immediately into next frame.\n\
         • delay < Duration::MAX -> Event loop sleeps via ControlFlow::WaitUntil(Instant::now() + delay).\n\
         • No repaint requested -> Event loop sleeps via ControlFlow::Wait (0.0% CPU usage).",
    );

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"// Smooth 60 FPS animation loop with paced repaints:
fn render_pulsing_indicator(ui: &mut Ui) {
    let time = ui.input(|i| i.time);
    let pulse = ((time * 3.0).sin() * 0.5 + 0.5) as f32;
    
    let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
    ui.painter().circle_filled(
        rect.center(),
        10.0 * pulse,
        egui::Color32::from_rgb(31, 111, 235),
    );
    
    // Request next frame in 16.6 milliseconds (~60 FPS):
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
}"#,
    );
}

fn render_subsection_4_3(ui: &mut Ui) {
    ui.heading(
        RichText::new("4.3 Styling, Visuals, and Accessibility (AccessKit)")
            .size(18.0)
            .strong(),
    );
    ui.add_space(8.0);

    ui.label(
        "Styling in egui is divided into two distinct structs:\n\
         • egui::Style: Layout spacing, text typography, interaction radius, window stroke, and scrollbar behavior.\n\
         • egui::Visuals: Color palettes, dark/light theme flags, background fills, and dynamic widget interaction states \
         (inactive, hovered, active, open).\n\n\
         Styles can be configured globally via ctx.set_style(style), or scoped to a specific container via ui.style_mut().",
    );

    ui.add_space(10.0);
    ui.label(
        RichText::new("Accessibility with AccessKit")
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(
        "Despite having no retained DOM, egui provides native screen reader accessibility. During each frame, \
         widgets declare semantic roles (Role::Button, Role::CheckBox, Role::TextInput), labels, and allowed actions \
         (Action::Click, Action::SetValue). AccessKit converts these into an accessibility tree dispatched to \
         native OS accessibility APIs (UI Automation on Windows, NSAccessibility on macOS, AT-SPI on Linux, and Web ARIA).",
    );

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"// Scoped style mutation affecting only widgets in this container:
ui.scope(|ui| {
    ui.style_mut().spacing.item_spacing = egui::vec2(16.0, 16.0);
    ui.visuals_mut().widgets.inactive.corner_radius = egui::CornerRadius::same(12);
    
    ui.button("Padded Rounded Button");
}); // Previous styles are restored when the scope drops!"#,
    );
}

fn render_subsection_4_4(ui: &mut Ui) {
    ui.heading(
        RichText::new("4.4 Dynamic Font Atlases & Texture Lifecycles")
            .size(18.0)
            .strong(),
    );
    ui.add_space(8.0);

    ui.label("egui manages graphical assets through high-performance dynamic caching:");

    ui.add_space(8.0);
    ui.label(
        "• Font System & Glyph Atlasing: Text glyphs are rasterized on demand using ab_glyph into a \
         single-channel alpha texture atlas (TextureId::default()). When new characters appear (e.g. CJK or emoji), \
         the atlas expands and emits an incremental texture delta (epaint::TexturesDelta) to the GPU.\n\
         • TextureManager & TextureHandle: Custom images (PNGs, JPEGs, procedural buffers) are loaded via \
         ctx.load_texture(\"name\", image_data, options), returning an RAII TextureHandle.\n\
         • RAII VRAM Deallocation: When TextureHandle drops, its destructor notifies TextureManager::free(id). \
         On the next frame pulse, TexturesDelta::free instructs the GPU backend (egui-wgpu) to release \
         the GPU texture and bind group from VRAM, guaranteeing zero GPU memory leaks.",
    );

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"// Loading raw RGBA image buffer into an RAII TextureHandle:
let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba_bytes);
let texture_handle = ctx.load_texture(
    "my_custom_image",
    color_image,
    egui::TextureOptions::LINEAR,
);

// Displaying the texture in UI:
ui.image(&texture_handle);
// When texture_handle is dropped, GPU VRAM is automatically reclaimed!"#,
    );
}

fn render_style_sandbox(
    ui: &mut Ui,
    state: &mut ManualPlaygroundState,
    target_to_launch: &mut Option<&'static str>,
) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(Stroke::new(1.0, Color32::from_rgb(31, 111, 235)))
        .inner_margin(14.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("🧪 Interactive Lab: Visuals & Style Theming Sandbox")
                    .strong()
                    .size(16.0)
                    .color(Color32::from_rgb(31, 111, 235)),
            );
            ui.add_space(6.0);
            ui.label(
                "Adjust corner rounding and item spacing sliders to observe how scoped style \
                 mutations dynamically reshape interactive widgets in real time:",
            );

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("Corner Radius:");
                ui.add(
                    egui::Slider::new(&mut state.style_corner_radius, 0.0..=24.0)
                        .suffix(" px")
                        .show_value(true),
                );
            });

            ui.horizontal(|ui| {
                ui.label("Item Spacing:");
                ui.add(
                    egui::Slider::new(&mut state.style_item_spacing, 2.0..=24.0)
                        .suffix(" px")
                        .show_value(true),
                );
            });

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(if ui.visuals().dark_mode {
                        "☀️ Switch to Light Mode"
                    } else {
                        "🌙 Switch to Dark Mode"
                    })
                    .clicked()
                {
                    if ui.visuals().dark_mode {
                        ui.ctx().set_visuals(egui::Visuals::light());
                    } else {
                        ui.ctx().set_visuals(egui::Visuals::dark());
                    }
                }

                if ui.button("↺ Reset Defaults").clicked() {
                    state.style_corner_radius = 8.0;
                    state.style_item_spacing = 6.0;
                    ui.ctx().set_visuals(egui::Visuals::dark());
                }
            });

            ui.add_space(12.0);

            // Scoped Style Preview Container
            let radius = CornerRadius::same(state.style_corner_radius.round() as u8);
            egui::Frame::group(ui.style())
                .fill(ui.visuals().window_fill)
                .stroke(ui.visuals().window_stroke)
                .inner_margin(12.0)
                .corner_radius(radius)
                .show(ui, |ui| {
                    ui.scope(|ui| {
                        ui.style_mut().spacing.item_spacing =
                            vec2(state.style_item_spacing, state.style_item_spacing);
                        ui.visuals_mut().widgets.inactive.corner_radius = radius;
                        ui.visuals_mut().widgets.hovered.corner_radius = radius;
                        ui.visuals_mut().widgets.active.corner_radius = radius;
                        ui.visuals_mut().widgets.open.corner_radius = radius;

                        ui.label(RichText::new("Live Themed Widget Container").strong());

                        ui.horizontal_wrapped(|ui| {
                            let _ = ui.button("Action Alpha");
                            let _ = ui.button("Action Beta");
                            ui.checkbox(&mut state.toggle_enabled, "Check Option");
                        });

                        ui.horizontal_wrapped(|ui| {
                            ui.label("User Name:");
                            ui.text_edit_singleline(&mut state.user_name);
                        });

                        ui.add(egui::ProgressBar::new(0.68).text("68% Dynamic Progress"));
                    });
                });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Next Step:").strong());
                ui.label("Explore UI Geometry, Layout Engines, and Custom Widgets in Chapter 5.");
                if ui
                    .button(RichText::new("Proceed to Chapter 5 ➔").strong())
                    .clicked()
                {
                    *target_to_launch = Some("📖 Chapter 5: Layout & Custom Widgets");
                }
            });
        });
}
