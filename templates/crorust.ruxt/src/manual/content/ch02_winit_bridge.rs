use crate::manual::widgets::ManualPlaygroundState;
use egui::{Color32, Frame, RichText, Stroke, Ui};

/// Renders Chapter 2: The Adapter Bridge — egui-winit & PlatformOutput.
///
/// Deeply explains:
/// 1. The Adapter Pattern: Why `egui` core is headless and pure.
/// 2. Event Transformation: Pointer movements, touch phases, modifiers, and IME composition.
/// 3. The PlatformOutput feedback loop (OS cursor mutations, clipboard, IME candidate positioning).
/// 4. Interactive Lab: Live Cursor Icon & Clipboard Feedback Tester.
pub fn render_chapter(
    ui: &mut Ui,
    state: &mut ManualPlaygroundState,
    target_to_launch: &mut Option<&'static str>,
) {
    // -------------------------------------------------------------------------
    // Chapter Title & Subtitle Header
    // -------------------------------------------------------------------------
    ui.heading(
        RichText::new("🌉 Chapter 2: The Adapter Bridge — egui-winit & PlatformOutput")
            .size(22.0)
            .strong(),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "The egui core library is completely decoupled from operating system APIs, windowing systems, \
             and graphics runtimes. The egui-winit crate serves as an essential bidirectional adapter: it \
             translates raw OS window events into logical RawInput structs and channels egui's PlatformOutput \
             back to OS window mutating commands.",
        )
        .size(13.5)
        .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    // -------------------------------------------------------------------------
    // Architectural Diagram: Bidirectional Adapter Bridge
    // -------------------------------------------------------------------------
    Frame::group(ui.style())
        .fill(if ui.visuals().dark_mode {
            Color32::from_rgb(15, 23, 42)
        } else {
            Color32::from_rgb(245, 247, 250)
        })
        .stroke(Stroke::new(1.0, ui.visuals().window_stroke().color))
        .inner_margin(12.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("📊 Architectural Overview: The Bidirectional Adapter Bridge")
                    .strong(),
            );
            ui.add_space(6.0);
            let viewer = crate::manual::widgets::diagram_viewer::InteractiveDiagramViewer::new(
                "ch02_winit_bridge",
                egui::include_image!("../../../assets/manual/svg/ch02_winit_bridge.svg"),
                include_str!("../../../assets/manual/diagrams/ch02_winit_bridge.mmd"),
                egui::vec2(2450.72, 903.0),
            );
            viewer.show(ui, state.diagram_state_mut("ch02_winit_bridge"));
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Figure 2.1: egui-winit transforms raw physical window events into normalized logical RawInput, \
                     drives egui::Context::run(), and maps emitted PlatformOutput back into native OS window mutations.",
                )
                .small()
                .weak(),
            );
        });

    ui.add_space(18.0);

    // -------------------------------------------------------------------------
    // Subsection 2.1: The Adapter Pattern
    // -------------------------------------------------------------------------
    ui.heading(
        RichText::new("2.1 The Adapter Pattern: Why egui Knows Nothing About Windows").size(17.0),
    );
    ui.add_space(4.0);
    ui.label(
        "In software architecture, the Adapter Pattern allows incompatible interfaces to collaborate. \
         The core `egui` crate contains zero lines of code dealing with Win32 message loops, Cocoa runloops, \
         Wayland sockets, or graphics drivers. It is a pure, headless immediate-mode calculation engine: \
         given an immutable set of inputs (`RawInput`), it evaluates layout geometry and returns paint commands (`FullOutput`).",
    );
    ui.add_space(8.0);

    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(10.0)
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.label(RichText::new("📦 Persistent State Inside egui_winit::State:").strong());
            ui.add_space(4.0);
            ui.label(
                "Because egui is stateless across frames, the `egui_winit::State` adapter maintains necessary \
                 cross-frame continuity:\n\
                 • Modifier State: Tracks active Ctrl, Alt, Shift, and Command/Super keys.\n\
                 • Pointer Position & Velocity: Caches last known mouse coordinates and computes drag deltas.\n\
                 • Double-Click & Multi-Click Timers: Computes click counts based on system timing thresholds.\n\
                 • Multi-Touch Tracking: Tracks active finger touches and computes two-finger pinch-to-zoom scales.\n\
                 • High-DPI Scale Factor: Caches current display DPI and converts physical coordinates to logical points."
            );
        });

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"// Initializing the egui-winit adapter bridge
let mut egui_winit_state = egui_winit::State::new(
    egui_ctx.clone(),
    viewport_id,
    &window,
    Some(window.scale_factor() as f32),
    None, // Max texture side limit
);

// Translating a native window event into egui input
let response = egui_winit_state.on_window_event(&window, &window_event);
if response.repaint {
    window.request_redraw();
}"#,
    );

    ui.add_space(18.0);

    // -------------------------------------------------------------------------
    // Subsection 2.2: Event Transformation Pipeline
    // -------------------------------------------------------------------------
    ui.heading(RichText::new("2.2 Event Transformation Pipeline").size(17.0));
    ui.add_space(4.0);
    ui.label(
        "Every raw hardware event from the OS window server passes through a transformation pipeline \
         before egui processes it:",
    );
    ui.add_space(8.0);

    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(10.0)
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.label(RichText::new("🔄 Input Transformation Mechanics:").strong());
            ui.add_space(4.0);
            ui.label(
                "1. Pointer & Mouse Events:\n\
                   `WindowEvent::CursorMoved { position }` arrives in physical pixels (e.g. x: 3840.0, y: 2160.0). \
                   The bridge divides coordinates by the display scale factor (2.0) to emit `egui::Event::PointerMoved(Pos2(1920.0, 1080.0))`.\n\n\
                 2. Capacitive Multi-Touch:\n\
                   Mobile touch events arrive with lifecycle phases (`Started`, `Moved`, `Ended`, `Cancelled`). \
                   The bridge translates multi-touch finger contacts into pinch-to-zoom zoom factors and scroll deltas.\n\n\
                 3. Keyboard & Modifiers:\n\
                   Raw scan codes and virtual keys are resolved into unicode characters and `egui::Key` variants, \
                   carrying modifier bitmasks (`Modifiers { alt, ctrl, shift, command, ... }`).\n\n\
                 4. Input Method Editor (IME) Asian Script Composition:\n\
                   Languages such as Chinese, Japanese, Korean, and Arabic require multi-stage composition. \
                   • `Ime::Preedit(String, Option<(usize, usize)>)`: Temporary composing string shown with an underline.\n\
                   • `Ime::Commit(String)`: Final composed characters committed to the text buffer."
            );
        });

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"// Inside egui-winit event processing
match &event {
    WindowEvent::CursorMoved { position, .. } => {
        let logical_pos = egui::pos2(
            (position.x / scale_factor) as f32,
            (position.y / scale_factor) as f32,
        );
        raw_input.events.push(egui::Event::PointerMoved(logical_pos));
    }
    WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
        raw_input.events.push(egui::Event::Text(text.clone()));
    }
    _ => (),
}"#,
    );

    ui.add_space(18.0);

    // -------------------------------------------------------------------------
    // Subsection 2.3: Feeding Outputs Back to the Operating System
    // -------------------------------------------------------------------------
    ui.heading(
        RichText::new("2.3 Feeding Outputs Back to the Operating System (PlatformOutput)")
            .size(17.0),
    );
    ui.add_space(4.0);
    ui.label(
        "At the end of a frame, `egui::Context::run()` emits a `FullOutput` struct containing vector geometry \
         and a `PlatformOutput` record. The adapter translates `PlatformOutput` back into native operating system actions:",
    );
    ui.add_space(8.0);

    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(10.0)
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.label(RichText::new("📋 PlatformOutput Channels:").strong());
            ui.add_space(4.0);
            ui.label(
                "• `cursor_icon`: Mutates the OS mouse pointer via `window.set_cursor(icon)`.\n\
                 • `commands` (`OutputCommand::CopyText`): Dispatches text to the OS clipboard via `arboard` or Wayland data source.\n\
                 • `commands` (`OutputCommand::OpenUrl`): Invokes the default web browser via OS shell primitives.\n\
                 • `ime`: Informs the OS where the active text cursor is located via `window.set_ime_cursor_area()`, \
                 ensuring candidate popup windows float directly adjacent to active text rather than obscuring it.\n\
                 • `viewport_output.repaint_delay`: Communicates sleep duration back to the winit event loop."
            );
        });

    ui.add_space(18.0);

    // -------------------------------------------------------------------------
    // Interactive Lab: Live Cursor Icon & Clipboard Feedback Tester
    // -------------------------------------------------------------------------
    Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(Stroke::new(1.0, Color32::from_rgb(139, 92, 246)))
        .inner_margin(14.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("🧪 Interactive Lab: Live Cursor Icon & Clipboard Feedback Tester")
                    .strong()
                    .size(15.0)
                    .color(Color32::from_rgb(139, 92, 246)),
            );
            ui.add_space(4.0);
            ui.label(
                "This lab directly mutates egui's `PlatformOutput` on every frame to verify that your OS window \
                 server responds to cursor mutations and clipboard commands in real-time.",
            );
            ui.add_space(10.0);

            ui.label(RichText::new("1. Live Cursor Mutation Pads (Hover to Inspect):").strong());
            ui.label(
                "Hover over each card below. Under the hood, egui executes `ui.output_mut(|o| o.cursor_icon = cursor)` \
                 which egui-winit then relays to `winit::window::Window::set_cursor_icon`.",
            );
            ui.add_space(8.0);

            let cursor_pads: [(&str, egui::CursorIcon, &str); 8] = [
                ("Default", egui::CursorIcon::Default, "Standard OS arrow pointer"),
                (
                    "Pointing Hand",
                    egui::CursorIcon::PointingHand,
                    "Hyperlinks & interactive items",
                ),
                (
                    "Crosshair",
                    egui::CursorIcon::Crosshair,
                    "Canvas drawing & precision targets",
                ),
                ("Text I-Beam", egui::CursorIcon::Text, "Selectable text & input fields"),
                (
                    "Wait / Spinner",
                    egui::CursorIcon::Wait,
                    "Long-running asynchronous tasks",
                ),
                ("Grab", egui::CursorIcon::Grab, "Draggable nodes & pan viewports"),
                (
                    "Resize Horizontal",
                    egui::CursorIcon::ResizeHorizontal,
                    "Splitters & window borders",
                ),
                (
                    "Not Allowed",
                    egui::CursorIcon::NotAllowed,
                    "Disabled actions & drop barriers",
                ),
            ];

            let mut active_cursor = None;
            let is_mobile = ui.available_width() < 550.0;
            let num_cols = if is_mobile { 2 } else { 4 };

            egui::Grid::new("cursor_pads_grid")
                .num_columns(num_cols)
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    for (idx, (label, cursor, description)) in cursor_pads.iter().enumerate() {
                        let card = Frame::group(ui.style())
                            .fill(if ui.visuals().dark_mode {
                                Color32::from_rgb(26, 32, 44)
                            } else {
                                Color32::from_rgb(237, 242, 247)
                            })
                            .inner_margin(8.0)
                            .corner_radius(6.0)
                            .show(ui, |ui| {
                                let pad_width = if is_mobile {
                                    (ui.available_width() - 8.0).max(100.0) / 2.0
                                } else {
                                    130.0
                                };
                                ui.set_min_size(egui::vec2(pad_width, 56.0));
                                ui.vertical_centered(|ui| {
                                    ui.label(RichText::new(*label).strong().size(13.0));
                                    ui.label(RichText::new(*description).small().weak());
                                });
                            });

                        let response = ui.interact(
                            card.response.rect,
                            ui.make_persistent_id(label),
                            egui::Sense::hover(),
                        );
                        if response.hovered() {
                            ui.output_mut(|o| o.cursor_icon = *cursor);
                            active_cursor = Some(*label);
                        }

                        if (idx + 1) % num_cols == 0 {
                            ui.end_row();
                        }
                    }
                });

            ui.add_space(8.0);
            if let Some(name) = active_cursor {
                ui.label(
                    RichText::new(format!(
                        "⚡ Active Cursor Mutated to: `{name}` via PlatformOutput::cursor_icon"
                    ))
                    .color(Color32::from_rgb(16, 185, 129))
                    .strong(),
                );
            } else {
                ui.label(
                    RichText::new(
                        "Hover over any pad above to mutate the OS cursor icon.",
                    )
                    .italics()
                    .weak(),
                );
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);

            ui.label(
                RichText::new(
                    "2. System Clipboard Verification (PlatformOutput::commands):",
                )
                .strong(),
            );
            ui.label(
                "Enter text below and click 'Copy to System Clipboard'. The text is packaged as \
                 `OutputCommand::CopyText` and dispatched to the platform clipboard provider (e.g. arboard):",
            );
            ui.add_space(6.0);

            let mut clipboard_input = ui.data_mut(|d| {
                d.get_temp_mut_or_insert_with(
                    egui::Id::new("manual_clipboard_input"),
                    || "Hello from Ruxt PlatformOutput!".to_owned(),
                )
                .clone()
            });

            let mut clipboard_status = ui.data_mut(|d| {
                d.get_temp_mut_or_insert_with(egui::Id::new("manual_clipboard_status"), String::new)
                    .clone()
            });

            ui.horizontal_wrapped(|ui| {
                if ui.text_edit_singleline(&mut clipboard_input).changed() {
                    ui.data_mut(|d| {
                        d.insert_temp(
                            egui::Id::new("manual_clipboard_input"),
                            clipboard_input.clone(),
                        );
                    });
                }

                if ui.button("📋 Copy to Platform Clipboard").clicked() {
                    ui.ctx().copy_text(clipboard_input.clone());
                    clipboard_status = format!("✓ Copied \"{clipboard_input}\" to OS clipboard!");
                    ui.data_mut(|d| {
                        d.insert_temp(
                            egui::Id::new("manual_clipboard_status"),
                            clipboard_status.clone(),
                        );
                    });
                }
            });

            if !clipboard_status.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(&clipboard_status)
                        .color(Color32::from_rgb(16, 185, 129))
                        .strong(),
                );
            }
        });

    ui.add_space(20.0);
    ui.separator();
    ui.add_space(12.0);

    // -------------------------------------------------------------------------
    // Chapter Navigation Footer
    // -------------------------------------------------------------------------
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("Completed Chapter 2.")
                .strong()
                .color(Color32::from_rgb(16, 185, 129)),
        );
        if ui
            .button(RichText::new("Launch Clipboard Test Window ➔").strong())
            .clicked()
        {
            *target_to_launch = Some("Clipboard Test");
        }
    });
}
