use crate::manual::widgets::ManualPlaygroundState;
use egui::{Color32, RichText, Stroke, Ui};

pub fn render_chapter(
    ui: &mut Ui,
    state: &mut ManualPlaygroundState,
    _target_to_launch: &mut Option<&'static str>,
) {
    // 1. Chapter Header Banner
    ui.heading(
        RichText::new("Chapter 3: The Immediate-Mode GUI (IMGUI) Architecture")
            .size(24.0)
            .strong(),
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "Deconstruct the stateless execution pulse, the mathematical projection UI = f(State), \
             cross-framework paradigms, deterministic 64-bit Id hashing, and generational memory garbage collection.",
        )
        .size(13.5)
        .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    // 2. High-Level Concept Diagram (SVG)
    let viewer = crate::manual::widgets::diagram_viewer::InteractiveDiagramViewer::new(
        "ch03_imgui_lifecycle",
        egui::include_image!("../../../assets/manual/svg/ch03_imgui_lifecycle.svg"),
        include_str!("../../../assets/manual/diagrams/ch03_imgui_lifecycle.mmd"),
        egui::vec2(704.61, 2185.03),
    );
    viewer.show(ui, state.diagram_state_mut("ch03_imgui_lifecycle"));
    ui.add_space(16.0);

    // 3. Subsections
    render_subsection_3_1(ui);
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(20.0);

    render_subsection_3_2(ui);
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(20.0);

    render_subsection_3_3(ui, state);
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(20.0);

    // 4. Interactive Lab: Live ID Collision Playground
    render_id_collision_playground(ui, state);
}

fn render_subsection_3_1(ui: &mut Ui) {
    ui.heading(
        RichText::new("3.1 Retained Mode vs. Immediate Mode: A Clear Mental Model")
            .size(18.0)
            .strong(),
    );
    ui.add_space(8.0);

    ui.label(
        "In traditional retained-mode GUI toolkits (Win32, Qt, HTML DOM, AppKit, UIKit), \
         widgets are persistent, stateful heap-allocated object trees. The programmer's data model \
         and the GUI toolkit's widget tree exist as two separate parallel hierarchies that must be continually \
         synchronized through imperative event callbacks. When a single callback fails to fire or races with \
         another thread, the data model and UI diverge — resulting in the classic 'stale-state / split-brain' defect.",
    );

    ui.add_space(10.0);
    ui.label(
        RichText::new("The Mental Model: The Film Projector vs. Puppets on Strings")
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(
        "• Retained Mode (Puppets on Strings): You construct physical marionettes (widgets) and manipulate their \
         poses via attached strings (callbacks). When a string snaps, the puppet remains frozen in an invalid state.\n\
         • Immediate Mode (The Film Projector): There are no marionettes. The interface is a pure optical projection \
         emitted from your application state 60 times a second:",
    );

    ui.add_space(6.0);
    ui.monospace("    UI_t = f(State_t)");
    ui.add_space(6.0);

    ui.label(
        "Every single frame, the entire visible interface is evaluated from scratch. If application state changes, \
         the very next frame automatically reflects the new reality without listeners, diffing passes, or manual DOM mutation.",
    );

    ui.add_space(12.0);
    ui.label(RichText::new("The Cross-Framework Rosetta Stone").strong());
    ui.add_space(4.0);

    // Rosetta Stone Comparison Grid (horizontally scrollable on narrow viewports)
    egui::ScrollArea::horizontal()
        .id_salt("rosetta_stone_table_scroll")
        .auto_shrink([false, false])
        .max_width(ui.available_width())
        .show(ui, |ui| {
            egui::Grid::new("rosetta_stone_table")
                .striped(true)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Framework").strong());
                    ui.label(RichText::new("Paradigm").strong());
                    ui.label(RichText::new("Where Nodes Live").strong());
                    ui.label(RichText::new("State Sync Mechanism").strong());
                    ui.label(RichText::new("Primary Bug Pattern").strong());
                    ui.end_row();

                    ui.label("Qt / Win32 / DOM");
                    ui.label("Retained Mode");
                    ui.label("Long-lived heap objects");
                    ui.label("Imperative callbacks (onClick)");
                    ui.label("Stale State / Split-Brain");
                    ui.end_row();

                    ui.label("React / Web");
                    ui.label("Retained VDOM");
                    ui.label("Ephemeral VDOM diffed to DOM");
                    ui.label("Reconciliation engine (Fiber)");
                    ui.label("Re-render Cascades / Hook Hell");
                    ui.end_row();

                    ui.label("SwiftUI");
                    ui.label("Retained View Tree");
                    ui.label("Ephemeral View structs -> NSView");
                    ui.label("Attribute graph dependency diff");
                    ui.label("Opaque update cycle drops");
                    ui.end_row();

                    ui.label("Flutter");
                    ui.label("Retained 3-Tree");
                    ui.label("Widget, Element, RenderObject");
                    ui.label("setState() marks element dirty");
                    ui.label("Heavy Widget boilerplate");
                    ui.end_row();

                    ui.label(
                        RichText::new("Ruxt (egui)")
                            .strong()
                            .color(Color32::from_rgb(31, 111, 235)),
                    );
                    ui.label(RichText::new("Pure Immediate").strong());
                    ui.label("Nowhere (Transient stack)");
                    ui.label("Direct parameter binding");
                    ui.label("ID Collision in loops");
                    ui.end_row();
                });
        });

    ui.add_space(14.0);
    ui.label(
        RichText::new("Code Comparison: Retained Boilerplate vs. Immediate Branching").strong(),
    );
    ui.add_space(4.0);

    crate::rust_view_ui(
        ui,
        r#"// Retained Mode (HTML / DOM / Qt) — Stale state vulnerability:
button.addEventListener("click", () => {
    state.count += 1;
    label.innerText = `Count: ${state.count}`; // Miss this line -> UI is out of sync!
});

// Ruxt Immediate Mode — Mathematically impossible to go out of sync:
if ui.button("Increment").clicked() {
    state.count += 1;
}
ui.label(format!("Count: {}", state.count)); // Automatically renders new state next frame!"#,
    );
}

fn render_subsection_3_2(ui: &mut Ui) {
    ui.heading(
        RichText::new("3.2 The Frame Execution Loop & The Three-Phase Pulse")
            .size(18.0)
            .strong(),
    );
    ui.add_space(8.0);

    ui.label(
        "Each tick of the engine executes a deterministic 3-phase pulse that transforms physical hardware \
         events into GPU vector draw commands:",
    );

    ui.add_space(8.0);
    ui.label(
        "1. Phase 1: Input Ingestion (RawInput): The platform adapter packages operating system inputs \
         (mouse coordinates, key presses, touch contacts, IME composition strings, viewport size, and delta time) \
         into a single egui::RawInput snapshot.\n\
         2. Phase 2: Immediate-Mode Pulse (app.update): The application code executes sequentially. Widgets allocate \
         screen bounding boxes, test pointer intersection, record interaction responses, and append draw commands (Vec<Shape>).\n\
         3. Phase 3: Output Emission (FullOutput): The engine partitions shapes into clipped draw primitives, extracts platform \
         commands (cursor shapes, clipboard text, external URL launches), and computes the exact duration until the next repaint (repaint_delay).",
    );

    ui.add_space(12.0);
    ui.label(
        RichText::new("Continuous vs. Reactive Demand-Driven 0% CPU Sleep")
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(
        "A common fear among engineers new to immediate-mode GUIs is that re-evaluating the UI function must melt laptop \
         batteries and peg CPU cores at 100%. In Ruxt, immediate-mode execution is demand-driven and reactive.\n\n\
         When the interface is static (no mouse movement, no key presses, no active animations), the main thread yields to \
         the operating system kernel and sleeps in native blocking primitives (epoll_wait on Linux, GetMessageW on Windows, \
         CFRunLoopRunInMode on macOS, ALooper_pollOnce on Android). CPU and GPU consumption drop to exactly 0.0%.\n\n\
         Execution resumes only when an OS hardware event arrives, or when code explicitly schedules a redraw via \
         ctx.request_repaint() or ctx.request_repaint_after(Duration).",
    );

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"// The eframe application loop maps directly to this reactive pulse:
impl eframe::App for MyRuxtApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My Reactive Application");
            ui.label(format!("Server Latency: {} ms", self.ping_ms));
            
            if ui.button("Ping Gateway").clicked() {
                self.trigger_network_ping(ctx.clone());
            }
        });
        // When update returns, egui inspects repaint_delay.
        // If no repaints were requested, the OS thread enters 0% CPU kernel sleep!
    }
}"#,
    );
}

fn render_subsection_3_3(ui: &mut Ui, state: &mut ManualPlaygroundState) {
    ui.heading(
        RichText::new("3.3 State Retention in a Stateless World: egui::Id & egui::Memory")
            .size(18.0)
            .strong(),
    );
    ui.add_space(8.0);

    ui.label(
        "If widgets exist only as transient stack frames for microseconds during app.update(), \
         where does a TextEdit preserve its cursor selection? Where does a ScrollArea store its vertical scroll offset? \
         Where does a CollapsingHeader remember whether it is expanded or collapsed?",
    );

    ui.add_space(10.0);
    ui.label(
        RichText::new("Deterministic 64-Bit ID Hashing")
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(
        "egui answers this paradox with deterministic identity. Every stateful widget computes a 64-bit identifier (egui::Id):\n\n\
         child_id = parent_id.with(salt)\n\n\
         The ID is hashed using high-entropy non-cryptographic algorithms (AHash / MurmurHash64) seeded with the parent container's \
         ID and the widget's label. State is stored across frames in egui::Memory, an internal dictionary mapping egui::Id to type-erased values.",
    );

    ui.add_space(12.0);
    let viewer = crate::manual::widgets::diagram_viewer::InteractiveDiagramViewer::new(
        "ch03_sub_id_hashing",
        egui::include_image!("../../../assets/manual/svg/ch03_sub_id_hashing.svg"),
        include_str!("../../../assets/manual/diagrams/ch03_sub_id_hashing.mmd"),
        egui::vec2(647.11, 1122.34),
    );
    viewer.show(ui, state.diagram_state_mut("ch03_sub_id_hashing"));
    ui.add_space(10.0);

    ui.label(
        RichText::new("The ID Collision Trap in Dynamic Loops")
            .strong()
            .color(Color32::from_rgb(239, 68, 68)),
    );
    ui.label(
        "When rendering dynamic lists with identical labels (e.g. for item in items { ui.button(\"Delete\"); }), \
         every button receives the identical parent ID and the identical salt (\"Delete\"). Consequently, both evaluate \
         to the exact same 64-bit ID! Clicking or hovering one widget highlights or activates both.\n\n\
         To resolve collisions, Ruxt provides three idiomatic patterns:",
    );

    ui.add_space(6.0);
    crate::rust_view_ui(
        ui,
        r#"// Solution 1: Container Scope Push (Recommended for complex rows)
ui.push_id(item.id, |ui| {
    if ui.button("Delete").clicked() { /* delete item */ }
});

// Solution 2: Label Salt Syntax (egui strips '##...' from visual display)
if ui.button(format!("Delete##{}", item.id)).clicked() { /* ... */ }

// Solution 3: Explicit Builder ID Salt
ui.add(egui::Button::new("Delete").id_salt(item.id));"#,
    );

    ui.add_space(10.0);
    ui.label(RichText::new("Generational Garbage Collection").strong());
    ui.label(
        "To avoid unbounded memory growth when items are removed from lists, egui::Memory tracks a last_frame_used \
         counter for every cached value. During each frame pulse, Memory::collect_garbage() sweeps entries not accessed \
         during the current frame, automatically reclaiming memory.",
    );
}

fn render_id_collision_playground(ui: &mut Ui, state: &mut ManualPlaygroundState) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(Stroke::new(1.0, Color32::from_rgb(31, 111, 235)))
        .inner_margin(14.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("🧪 Interactive Lab: Live ID Collision Playground")
                    .strong()
                    .size(16.0)
                    .color(Color32::from_rgb(31, 111, 235)),
            );
            ui.add_space(6.0);
            ui.label(
                "Toggle unique ID scoping below to witness how identical widget labels trigger \
                 state collisions in egui::Memory, and how push_id and id_salt resolve them:",
            );

            ui.add_space(10.0);
            ui.checkbox(
                &mut state.collision_scoping_enabled,
                RichText::new("Enable Unique ID Scoping (ui.push_id)").strong(),
            );

            ui.add_space(12.0);

            let id_a = if state.collision_scoping_enabled {
                ui.make_persistent_id("item_0").with("Action Item")
            } else {
                ui.make_persistent_id("Action Item")
            };

            let id_b = if state.collision_scoping_enabled {
                ui.make_persistent_id("item_1").with("Action Item")
            } else {
                ui.make_persistent_id("Action Item")
            };

            // High-Contrast Accessible Diagnostic Banner
            if state.collision_scoping_enabled {
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(13, 40, 24))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(34, 197, 94)))
                    .inner_margin(12.0)
                    .corner_radius(6.0)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("✅ SCOPED IDS ACTIVE: Distinct 64-bit Identifiers")
                                .strong()
                                .color(Color32::from_rgb(74, 222, 128)),
                        );
                        ui.label(
                            RichText::new(format!("Item 0: egui::Id(0x{:016X})", id_a.value()))
                                .monospace()
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(format!("Item 1: egui::Id(0x{:016X})", id_b.value()))
                                .monospace()
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(
                                "Expanding one header has zero effect on the other. State is completely isolated.",
                            )
                            .color(Color32::from_rgb(220, 252, 231)),
                        );
                    });
            } else {
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(35, 12, 14))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(239, 68, 68)))
                    .inner_margin(12.0)
                    .corner_radius(6.0)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("⚠️ ID COLLISION ACTIVE: Duplicate 64-bit Identifiers!")
                                .strong()
                                .color(Color32::from_rgb(248, 113, 113)),
                        );
                        ui.label(
                            RichText::new(format!("Item 0: egui::Id(0x{:016X})", id_a.value()))
                                .monospace()
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(format!("Item 1: egui::Id(0x{:016X})", id_b.value()))
                                .monospace()
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(
                                "Notice that both items share the exact same hash! Try expanding Item 0 below: \
                                 Item 1 will expand simultaneously because egui::Memory cannot distinguish them!",
                            )
                            .color(Color32::from_rgb(254, 202, 202)),
                        );
                    });
            }

            ui.add_space(12.0);

            // Render the items
            if state.collision_scoping_enabled {
                ui.push_id("collision_lab_item_0", |ui| {
                    let resp = egui::CollapsingHeader::new("Action Item (Scoped ID: Index 0)")
                        .open(Some(state.item_0_expanded))
                        .show(ui, |ui| {
                            ui.label("Payload contents for Item 0. State is completely isolated.");
                        });
                    if resp.header_response.clicked() {
                        state.item_0_expanded = !state.item_0_expanded;
                    }
                });
                ui.push_id("collision_lab_item_1", |ui| {
                    let resp = egui::CollapsingHeader::new("Action Item (Scoped ID: Index 1)")
                        .open(Some(state.item_1_expanded))
                        .show(ui, |ui| {
                            ui.label("Payload contents for Item 1. State is completely isolated.");
                        });
                    if resp.header_response.clicked() {
                        state.item_1_expanded = !state.item_1_expanded;
                    }
                });
            } else {
                let mut toggled = false;
                ui.push_id("collision_lab_unscoped_0", |ui| {
                    let resp = egui::CollapsingHeader::new("Action Item (Collided State: Item 0)")
                        .open(Some(state.shared_expanded))
                        .show(ui, |ui| {
                            ui.label("Payload contents for Item 0 (Sharing Memory State with Item 1!).");
                        });
                    if resp.header_response.clicked() {
                        toggled = true;
                    }
                });
                ui.push_id("collision_lab_unscoped_1", |ui| {
                    let resp = egui::CollapsingHeader::new("Action Item (Collided State: Item 1)")
                        .open(Some(state.shared_expanded))
                        .show(ui, |ui| {
                            ui.label("Payload contents for Item 1 (Sharing Memory State with Item 0!).");
                        });
                    if resp.header_response.clicked() {
                        toggled = true;
                    }
                });
                if toggled {
                    state.shared_expanded = !state.shared_expanded;
                }
            }
        });
}
