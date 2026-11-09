use crate::manual::widgets::ManualPlaygroundState;
use egui::{Color32, RichText, Ui, Vec2};

pub fn render_chapter(
    ui: &mut Ui,
    state: &mut ManualPlaygroundState,
    _target_to_launch: &mut Option<&'static str>,
) {
    ui.heading(
        RichText::new("🚀 Chapter 7: Integration in Ruxt: Multi-Platform Architecture & Testing")
            .size(22.0)
            .strong(),
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "This final synthesis chapter unifies the entire Ruxt architecture. We trace the complete \
             9-stage journey from a physical hardware click to monitor photons, explore runtime invariants \
             across Mobile, WebAssembly, and Desktop tiers, and master automated headless testing with egui_kittest.",
        )
        .size(13.5)
        .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    // Architectural System Journey Vector Diagram
    let viewer = crate::manual::widgets::diagram_viewer::InteractiveDiagramViewer::new(
        "ch07_ruxt_journey",
        egui::include_image!("../../../assets/manual/svg/ch07_ruxt_journey.svg"),
        include_str!("../../../assets/manual/diagrams/ch07_ruxt_journey.mmd"),
        egui::vec2(4750.0, 1356.0),
    );
    viewer.show(ui, state.diagram_state_mut("ch07_ruxt_journey"));
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Figure 7.1: The 9-Stage Sequential Pipeline from Hardware IRQ to Screen Photons (Zoom in to inspect detail)",
        )
        .small()
        .weak(),
    );
    ui.add_space(16.0);

    // =========================================================================
    // Subsection 7.1: The Complete 9-Stage System Journey
    // =========================================================================
    ui.heading(RichText::new("7.1 The Complete 9-Stage System Journey").size(17.0));
    ui.add_space(6.0);
    ui.label(
        "Every single interaction in Ruxt traverses a deterministic, reactive 9-stage pipeline. \
         Understanding each layer allows systems engineers to pinpoint latency, eliminate redraw \
         inefficiencies, and reason about cross-platform behavior with absolute clarity:",
    );
    ui.add_space(8.0);

    let stages = [
        (
            "1. Hardware Interrupt (IRQ)",
            "Physical contact closes a mouse microswitch or fingertip alters touchscreen capacitance. The controller emits an interrupt packet over USB/I2C. The OS kernel driver (evdev, win32k.sys, IOKit) decodes raw coordinates.",
        ),
        (
            "2. OS Window Server",
            "Compositor (Wayland Mutter/Sway, Windows DWM, macOS WindowServer, Android SurfaceFlinger) hit-tests window geometries and enqueues platform messages (WM_LBUTTONDOWN, wl_pointer.button, NSEvent, MotionEvent).",
        ),
        (
            "3. winit 0.30 Event Loop",
            "The main GUI thread wakes from kernel sleep (epoll_wait, GetMessageW, CFRunLoopRun, ALooper_pollAll). winit normalizes platform messages into WindowEvent::MouseInput or WindowEvent::Touch on Thread 0.",
        ),
        (
            "4. egui-winit Adapter Bridge",
            "Normalizes physical device pixels into logical points via DPI scale factor (points = pixels / scale_factor), tracks active keyboard modifiers (Cmd, Ctrl, Alt, Shift), and packages events into egui::RawInput.",
        ),
        (
            "5. egui::Context & UI Evaluation",
            "ctx.begin_pass() initiates the immediate-mode pass. app.update() executes top-to-bottom. ui.button(\"Deploy\").clicked() evaluates hit-testing against allocated Rect bounds and returns true in that exact frame.",
        ),
        (
            "6. Vector Shape Emission",
            "The button emits vector draw commands (Shape::Rect for background styling and Shape::Text for glyph runs) into ui.painter(), bounded by the active scissor clip rectangle.",
        ),
        (
            "7. epaint Tessellation & AA",
            "ctx.end_pass() invokes epaint::Tessellator. Shapes are triangulated into GPU vertex/index arrays with 20-byte vertex strides. Perimeter edges extrude a 1px alpha-feathered gradient ribbon for sub-pixel anti-aliasing without MSAA.",
        ),
        (
            "8. egui-wgpu Hardware Render Pass",
            "wgpu stages buffers into VRAM, uploads the orthographic projection uniform buffer with Y-flip into NDC [-1.0, 1.0], binds textures, sets hardware scissor rects, and encodes draw_indexed commands.",
        ),
        (
            "9. Display Presentation & Photons",
            "surface_texture.present() queues the swapchain frame. The display controller scans out pixels on the next VSync interval, emitting photons. The event loop returns to sleep (ControlFlow::Wait) at 0% CPU/GPU.",
        ),
    ];

    for (title, desc) in stages {
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .corner_radius(6.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(title)
                            .strong()
                            .color(ui.visuals().strong_text_color()),
                    );
                });
                ui.add_space(2.0);
                ui.label(RichText::new(desc).size(12.5));
            });
        ui.add_space(4.0);
    }

    ui.add_space(10.0);
    ui.label(
        RichText::new("⚡ Demand-Driven Execution: Zero Idle Waste")
            .strong()
            .color(Color32::from_rgb(31, 111, 235)),
    );
    ui.label(
        "Immediate-mode GUIs in Ruxt do NOT spin at 1000 FPS draining battery. When the interface is \
         static and no animations are active, the event loop blocks inside the OS kernel. CPU and GPU \
         consumption drop to exactly 0.00% until the next hardware interrupt arrives.",
    );

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"// The core demand-driven evaluation pulse:
impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("Deploy Application").clicked() {
                self.trigger_production_deploy();
            }
        });
        // If no animations or worker threads call ctx.request_repaint(),
        // execution pauses immediately until the next OS event wakes winit.
    }
}"#,
    );

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(16.0);

    // =========================================================================
    // Subsection 7.2: Cross-Platform Invariants in Ruxt
    // =========================================================================
    ui.heading(RichText::new("7.2 Cross-Platform Invariants in Ruxt").size(17.0));
    ui.add_space(6.0);
    ui.label(
        "Writing cross-platform Rust requires respecting strict platform constraints. Ruxt encapsulates \
         these nuances so engineers write code once that deploys flawlessly across Mobile, WebAssembly, and Desktop:",
    );
    ui.add_space(10.0);

    // Platform Matrix Vector Diagram
    let matrix_viewer = crate::manual::widgets::diagram_viewer::InteractiveDiagramViewer::new(
        "ch07_platform_matrix",
        egui::include_image!("../../../assets/manual/svg/ch07_platform_matrix.svg"),
        include_str!("../../../assets/manual/diagrams/ch07_platform_matrix.mmd"),
        egui::vec2(738.75, 1217.31),
    );
    matrix_viewer.show(ui, state.diagram_state_mut("ch07_platform_matrix"));
    ui.add_space(4.0);
    ui.label(
        RichText::new("Figure 7.2: The Multi-Platform Invariants & Architecture Matrix")
            .small()
            .weak(),
    );
    ui.add_space(14.0);

    // Mobile Section
    ui.label(
        RichText::new("📱 1. Mobile Invariants (Android & iOS)")
            .strong()
            .size(15.0),
    );
    ui.add_space(4.0);
    ui.label(
        "• 48dp Minimum Touch Target Ergonomics: Human fingertips average 7-10mm in physical width. \
         Touch controls smaller than 48dp trigger frequent misclicks and user frustration.\n\
         • Hardware Cutouts & Safe Areas: Notches, punch holes, and dynamic islands obscure corner geometry. \
         Ruxt queries `ui.ctx().content_rect()` to position windows strictly within safe margins.\n\
         • Virtual Soft Keyboard (IME) Avoidance: When a TextEdit gains focus, the virtual keyboard consumes \
         up to 50% of the screen. Ruxt auto-scrolls focused inputs into the visible upper half.\n\
         • 16KB Page Alignment on Android 15: Modern Android kernels enforce 16KB-page ELF segments for memory \
         bandwidth; crorust ensures page-aligned shared libraries (.so) and zero-copy mmap.",
    );

    ui.add_space(10.0);

    // Interactive Touch Target Sandbox
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(31, 111, 235)))
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("🧪 Interactive Lab: Touch Target Ergonomics Inspector")
                    .strong()
                    .color(Color32::from_rgb(31, 111, 235)),
            );
            ui.add_space(4.0);
            ui.label(
                "Toggle between standard desktop spacing and mobile 48dp touch targets to observe \
                 live button dimension enforcement:",
            );
            ui.add_space(8.0);

            ui.checkbox(
                &mut state.touch_target_ergonomics_active,
                "Enforce Mobile 48dp Minimum Target (ui.spacing_mut().interact_size = vec2(48, 48))",
            );
            ui.add_space(8.0);

            let old_interact_size = ui.spacing().interact_size;
            if state.touch_target_ergonomics_active {
                ui.spacing_mut().interact_size = Vec2::new(48.0, 48.0);
            } else {
                ui.spacing_mut().interact_size = Vec2::new(18.0, 18.0);
            }

            let btn_resp = ui.button("📱 Tap Target Inspection Button");
            let actual_size = btn_resp.rect.size();

            ui.spacing_mut().interact_size = old_interact_size;

            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "Allocated Button Bounding Rect: {:.1}pt × {:.1}pt (Height ≥ 48dp: {})",
                    actual_size.x,
                    actual_size.y,
                    if actual_size.y >= 48.0 {
                        "PASS ✓"
                    } else {
                        "FAIL (Desktop Size)"
                    }
                ))
                .small()
                .color(if actual_size.y >= 48.0 {
                    Color32::from_rgb(46, 160, 67)
                } else {
                    Color32::from_rgb(209, 36, 47)
                }),
            );
        });

    ui.add_space(14.0);

    // WebAssembly Section
    ui.label(
        RichText::new("🌐 2. WebAssembly Invariants (WASM in Browser)")
            .strong()
            .size(15.0),
    );
    ui.add_space(4.0);
    ui.label(
        "• Single-Threaded Event Loop: Browsers execute WebAssembly on the DOM main thread. \
         Calling `std::thread::sleep` freezes the entire tab. Ruxt yields control via `window.requestAnimationFrame`.\n\
         • Sandboxed Virtual Filesystem: Browsers forbid direct POSIX `std::fs` operations. Ruxt synthesizes \
         HTML `<input type=\"file\">` elements and reads streamed byte buffers through the `FileReader` API.\n\
         • Canvas DPR Synchronization: High-DPI screens have device pixel ratios > 1.0. The `<canvas>` buffer \
         resolution is scaled by DPR while CSS sizing remains constant, preventing blurry rendering.",
    );

    ui.add_space(14.0);

    // Desktop Section
    ui.label(
        RichText::new("💻 3. Desktop Invariants (Linux, macOS, Windows)")
            .strong()
            .size(15.0),
    );
    ui.add_space(4.0);
    ui.label(
        "• Virtual Multi-Window Workspaces: Full support for multiple overlapping, dockable, and resizable \
         windows with z-order elevation upon click.\n\
         • Asynchronous File Dialogs: Native OS open/save dialogs via `rfd` dispatched onto background Tokio \
         tasks to prevent blocking the GUI thread.\n\
         • OS Clipboard Integration: Bidirectional clipboard read/write via system clipboard providers.",
    );

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(16.0);

    // =========================================================================
    // Subsection 7.3: Automated Headless Testing with egui_kittest
    // =========================================================================
    ui.heading(RichText::new("7.3 Automated Headless Testing with egui_kittest").size(17.0));
    ui.add_space(6.0);
    ui.label(
        "Historically, GUI testing required slow, fragile integration setups: spinning up an X11 server, \
         opening physical OS windows, and diffing PNG framebuffers. Ruxt leverages `egui_kittest` to \
         execute 100% headless automated tests in CI without a GPU, window server, or display driver:",
    );
    ui.add_space(10.0);

    crate::rust_view_ui(
        ui,
        r#"#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use ruxt::manual::RuxtManualWindow;
    use ruxt::manual::chapter::RuxtTutorialChapter;

    #[test]
    fn test_all_chapters_render_without_panic() {
        let mut manual = RuxtManualWindow::default();

        for chapter in RuxtTutorialChapter::ALL {
            manual.selected_chapter = *chapter;

            // 1. Build an isolated, headless Harness (no GPU / no window)
            let mut harness = Harness::builder()
                .with_size(egui::vec2(1200.0, 900.0))
                .build_ui(|ui| {
                    manual.ui(ui);
                });

            // 2. Pump 3 full immediate-mode frames to settle layout convergence
            harness.run_steps(3);

            // 3. Assert geometry bounds and non-zero layout allocation
            let used_rect = harness.ctx.globally_used_rect();
            assert!(
                used_rect.height() > 300.0,
                "Chapter {} generated insufficient vertical layout",
                chapter.number()
            );
        }
    }
}"#,
    );

    ui.add_space(12.0);
    ui.label(
        "Headless testing in Ruxt validates three critical dimensions:\n\
         1. Zero Panics: Ensures every chapter, widget, and layout pass evaluates cleanly across all frames.\n\
         2. Mobile Geometry Invariants: Asserts that 360×640 portrait viewports generate zero horizontal overflow.\n\
         3. AccessKit Query Assertions: Queries the semantic accessibility tree to verify button text, labels, \
         and input interactions without hardcoded pixel coordinates.",
    );

    ui.add_space(16.0);

    // Graduation Banner
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(46, 160, 67)))
        .corner_radius(8.0)
        .inner_margin(14.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🎓").size(24.0));
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Congratulations! You Have Mastered Ruxt Internals")
                            .strong()
                            .size(15.0)
                            .color(Color32::from_rgb(46, 160, 67)),
                    );
                    ui.label(
                        "You now possess a complete systems-level understanding of the platform event loop, \
                         adapter bridges, immediate-mode state retention, engine coordination, 2D geometry algebra, \
                         GPU tessellation pipelines, and headless CI verification. You are ready to engineer \
                         world-class cross-platform software.",
                    );
                });
            });
        });
}
