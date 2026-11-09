use crate::manual::widgets::ManualPlaygroundState;
use egui::{Align2, Color32, FontId, Response, RichText, Sense, Stroke, Ui, Widget, pos2, vec2};

pub fn render_chapter(
    ui: &mut Ui,
    state: &mut ManualPlaygroundState,
    _target_to_launch: &mut Option<&'static str>,
) {
    // 1. Chapter Header Banner
    ui.heading(
        RichText::new("📐 UI Geometry, Layout Engine & Custom Widgets")
            .size(24.0)
            .strong(),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Master 2D spatial coordinate algebra, layout cursor progression, two-frame convergence, \
             layer depth sorting, and custom widget authoring.",
        )
        .size(13.5)
        .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    // 2. High-Level Architectural SVG Diagram
    let viewer = crate::manual::widgets::diagram_viewer::InteractiveDiagramViewer::new(
        "ch05_layout_geometry",
        egui::include_image!("../../../assets/manual/svg/ch05_layout_geometry.svg"),
        include_str!("../../../assets/manual/diagrams/ch05_layout_geometry.mmd"),
        egui::vec2(1236.20, 812.0),
    );
    viewer.show(ui, state.diagram_state_mut("ch05_layout_geometry"));
    ui.add_space(16.0);

    // 3. Subsection 5.1: The Geometry Toolkit
    render_sub_5_1(ui);
    ui.add_space(20.0);

    // 4. Subsection 5.2: Layout Cursor Progression & Convergence
    render_sub_5_2(ui);
    ui.add_space(20.0);

    // 5. Subsection 5.3: Layers, Z-Ordering & Clip Rectangles
    render_sub_5_3(ui);
    ui.add_space(20.0);

    // 6. Subsection 5.4: Custom Widget Authoring & The Widget Trait
    render_sub_5_4(ui);
    ui.add_space(24.0);

    // 7. Interactive Lab: Custom Widget Workshop (RotaryKnob)
    render_rotary_knob_lab(ui, state);
}

// ----------------------------------------------------------------------------
// Subsection 5.1: The Geometry Toolkit
// ----------------------------------------------------------------------------

fn render_sub_5_1(ui: &mut Ui) {
    ui.heading(RichText::new("5.1 The Geometry Toolkit: Pos2, Vec2, Rect, and Sense").size(18.0));
    ui.add_space(6.0);

    ui.label(
        "Immediate-mode GUI layouts rely on explicit 2D spatial coordinate algebra. All coordinates \
         in egui are represented in logical points (DPI-normalized), with the origin (0.0, 0.0) anchored \
         at the top-left corner of the viewport. +X extends rightward, and +Y extends downward.",
    );
    ui.add_space(6.0);

    ui.label(
        "egui differentiates strictly between absolute points and relative displacement vectors:\n\
         • Pos2: A concrete spatial location [x, y].\n\
         • Vec2: A directional displacement vector [Δx, Δy].\n\
         • Rect: An axis-aligned bounding box defined by min: Pos2 (top-left) and max: Pos2 (bottom-right).\n\
         • Sense: An interaction consumption bitmask that instructs the event router whether a widget \
           responds to hover, clicks, or dragging.",
    );
    ui.add_space(8.0);

    crate::rust_view_ui(
        ui,
        r#"use egui::{pos2, vec2, Pos2, Vec2, Rect, Sense};

// 1. Point and Vector Algebra
let p1: Pos2 = pos2(100.0, 50.0);
let delta: Vec2 = vec2(20.0, -10.0);
let p2: Pos2 = p1 + delta;          // Pos2 + Vec2 = Pos2 -> (120.0, 40.0)
let diff: Vec2 = p2 - p1;           // Pos2 - Pos2 = Vec2 -> (20.0, -10.0)

// 2. Axis-Aligned Bounding Box (Rect)
let rect = Rect::from_min_size(p1, vec2(200.0, 80.0));
assert!(rect.contains(pos2(150.0, 70.0)));
let expanded = rect.expand(4.0);     // Grows in all 4 directions
let shrunk = rect.shrink(2.0);       // Padding inward

// 3. Interaction Bitmask (Sense)
let hover_only = Sense::hover();
let clickable = Sense::click();
let draggable = Sense::drag();
let interactive = Sense::click_and_drag(); // Union of click and drag"#,
    );
}

// ----------------------------------------------------------------------------
// Subsection 5.2: Layout Cursor Progression & Convergence
// ----------------------------------------------------------------------------

fn render_sub_5_2(ui: &mut Ui) {
    ui.heading(RichText::new("5.2 Layout Cursor Progression & Two-Frame Convergence").size(18.0));
    ui.add_space(6.0);

    ui.label(
        "Inside every container (Panel, Window, or Child UI), egui tracks layout placement using an internal \
         cursor (ui.cursor()). The cursor stores the bounding rectangle available for the very next widget.",
    );
    ui.add_space(6.0);

    ui.label(
        "When a widget calls ui.allocate_exact_size(size, sense):\n\
         1. The widget is placed at the current cursor position (cursor.min).\n\
         2. The cursor advances along the layout direction (downward in vertical layouts, rightward in \
            horizontal layouts) by the widget size plus ui.spacing().item_spacing.\n\
         3. ui.min_rect() expands to encompass the newly placed widget, tracking the cumulative bounding \
            envelope of all allocated children.\n\
         4. ui.max_rect() remains the immutable boundary constraint of the container.",
    );
    ui.add_space(8.0);

    ui.label(
        RichText::new("The Two-Frame Layout Convergence Mechanism:")
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(
        "Because immediate-mode UI executes top-to-bottom in a single pass per frame, dynamic containers \
         (such as auto-sizing floating windows, word-wrapped labels, or collapsing headers) cannot know \
         their exact dimensions before their children execute. egui resolves this via two-frame convergence:\n\
         • Frame 1 (Measurement): The container executes with previous or estimated bounds. As children \
           execute, they allocate space and expand ui.min_rect(). At the end of Frame 1, egui caches this \
           total envelope into egui::Memory.\n\
         • Frame 2 (Stabilization): The container reads the cached bounding envelope from Memory at the start \
           of the frame, sizing its background and borders to snugly fit its contents with zero jitter.",
    );
    ui.add_space(8.0);

    crate::rust_view_ui(
        ui,
        r#"// Space Allocation and Cursor Advancement
let desired_size = vec2(120.0, 32.0);
let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

// Inspecting Container Bounds:
let cursor_before = ui.cursor(); // Next widget's insertion point
let content_envelope = ui.min_rect(); // Bounding box of all widgets so far
let max_available = ui.max_rect(); // Outer container limits"#,
    );
}

// ----------------------------------------------------------------------------
// Subsection 5.3: Layers, Z-Ordering & Clip Rectangles
// ----------------------------------------------------------------------------

fn render_sub_5_3(ui: &mut Ui) {
    ui.heading(
        RichText::new("5.3 Layers, Z-Ordering & Clip Rectangles (LayerId, Order)").size(18.0),
    );
    ui.add_space(6.0);

    ui.label(
        "Immediate-mode draw commands are not emitted directly to hardware. Instead, painter commands \
         are grouped into discrete Layers identified by a LayerId (a combination of an Order tier and \
         a 64-bit Id).",
    );
    ui.add_space(6.0);

    ui.label(
        "egui divides visual depth into 8 deterministic stacking tiers (Order):\n\
         1. Order::Background — Canvas backdrops, grid paper, desktop wallpaper.\n\
         2. Order::Middle — Standard panel contents and inline widgets.\n\
         3. Order::Foreground — Drag-and-drop ghost representations and selection lassos.\n\
         4. Order::PanelResize — Splitters and panel resizing grab handles.\n\
         5. Order::Windows — Movable, floating virtual windows.\n\
         6. Order::Popups — Context menus, combo-box dropdowns.\n\
         7. Order::Tooltip — Floating hover tooltips.\n\
         8. Order::Debug — Diagnostic wireframe overlays.\n\n\
         Within the Order::Windows tier, clicking on a window promotes its LayerId to the front of the tier, \
         providing seamless OS-like window focus without rebuilding widget trees.",
    );
    ui.add_space(8.0);

    ui.label(
        RichText::new("Scissor Clip Rectangles:")
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(
        "To prevent widgets inside scroll areas or child panels from drawing outside their designated bounds, \
         egui enforces scissor clip rectangles. Nested clip rects intersect mathematically:\n\
         R_effective = R_parent ∩ R_child\n\
         If R_effective has zero area (min.x >= max.x or min.y >= max.y), the widget is culled and \
         emits zero draw commands.",
    );
    ui.add_space(8.0);

    crate::rust_view_ui(
        ui,
        r#"use egui::{LayerId, Order, Id};

// Emit vector shapes into a custom layer (e.g. Floating Tooltip)
let tooltip_layer = LayerId::new(Order::Tooltip, Id::new("my_custom_tooltip"));
let painter = ui.ctx().layer_painter(tooltip_layer);

// Scissor Clip Rect Intersection
let parent_clip = ui.clip_rect();
let child_rect = Rect::from_min_size(pos2(100.0, 100.0), vec2(200.0, 150.0));
let effective_clip = parent_clip.intersection(child_rect);

// Restrict painter commands to the intersected scissor bounds
let scoped_painter = ui.painter().with_clip_rect(effective_clip);"#,
    );
}

// ----------------------------------------------------------------------------
// Subsection 5.4: Custom Widget Authoring & The Widget Trait
// ----------------------------------------------------------------------------

fn render_sub_5_4(ui: &mut Ui) {
    ui.heading(RichText::new("5.4 Custom Widget Authoring & The Widget Trait").size(18.0));
    ui.add_space(6.0);

    ui.label(
        "Every built-in control in egui (Button, Slider, Checkbox, TextEdit) implements the Widget trait:\n\
         pub trait Widget { fn ui(self, ui: &mut Ui) -> Response; }\n\
         Writing custom widgets follows an invariant 4-step sequence:",
    );
    ui.add_space(6.0);

    ui.label(
        "1. Space Allocation: Call ui.allocate_exact_size(desired_size, sense) to reserve screen real estate.\n\
         2. Interaction Interrogation: Check response.hovered(), response.clicked(), response.dragged(), or read response.drag_delta().\n\
         3. Styling & Themes: Query ui.style().interact(&response) to extract theme-aware colors, strokes, and corner radiuses.\n\
         4. Vector Painting: Emit vector shapes into ui.painter().with_clip_rect(rect) to render custom geometry.",
    );
    ui.add_space(8.0);

    crate::rust_view_ui(
        ui,
        r#"pub struct CustomBadge {
    label: &'static str,
    color: Color32,
}

impl Widget for CustomBadge {
    fn ui(self, ui: &mut Ui) -> Response {
        // 1. Space Allocation
        let desired_size = vec2(80.0, 24.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        // 2. Interaction & Styling
        let visuals = ui.style().interact(&response);
        let bg_fill = if response.hovered() { self.color } else { visuals.bg_fill };

        // 3. Vector Painting (only if visible within screen scissor bounds)
        if ui.is_rect_visible(rect) {
            let painter = ui.painter().with_clip_rect(rect);
            painter.rect_filled(rect, CornerRadius::same(4), bg_fill);
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                self.label,
                FontId::proportional(12.0),
                visuals.text_color(),
            );
        }

        response
    }
}"#,
    );
}

// ----------------------------------------------------------------------------
// Custom Widget Implementation: RotaryKnob
// ----------------------------------------------------------------------------

pub struct RotaryKnob<'a> {
    value: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
    diameter: f32,
    detents: usize,
    title: &'static str,
}

impl<'a> RotaryKnob<'a> {
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>) -> Self {
        Self {
            value,
            range,
            diameter: 64.0,
            detents: 10,
            title: "Volume",
        }
    }

    pub fn diameter(mut self, diameter: f32) -> Self {
        self.diameter = diameter;
        self
    }

    pub fn detents(mut self, detents: usize) -> Self {
        self.detents = detents;
        self
    }

    pub fn title(mut self, title: &'static str) -> Self {
        self.title = title;
        self
    }
}

impl<'a> Widget for RotaryKnob<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let desired_size = vec2(self.diameter, self.diameter + 24.0);
        let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        let min_val = *self.range.start();
        let max_val = *self.range.end();
        let span = (max_val - min_val).max(1e-5);

        // Interaction Handling: Vertical & Horizontal dragging
        if response.dragged() {
            let delta = response.drag_delta();
            let sensitivity = 0.004;
            let change = (-delta.y + delta.x) * sensitivity * span;
            let mut new_val = (*self.value + change).clamp(min_val, max_val);

            // Detent snapping if near a notch
            if self.detents > 0 {
                let step = span / self.detents as f32;
                let nearest_notch = ((new_val - min_val) / step).round() * step + min_val;
                if (new_val - nearest_notch).abs() < step * 0.15 {
                    new_val = nearest_notch;
                }
            }

            if (new_val - *self.value).abs() > f32::EPSILON {
                *self.value = new_val;
                response.mark_changed();
            }
        }

        // Render custom geometry only if rect is visible
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            let painter = ui.painter().with_clip_rect(rect);

            let knob_center = pos2(rect.center().x, rect.min.y + self.diameter * 0.5);
            let radius = self.diameter * 0.42;

            // Angular geometry: -135° to +135° (270° sweep)
            let start_angle = -135.0_f32.to_radians();
            let end_angle = 135.0_f32.to_radians();
            let sweep_angle = end_angle - start_angle;
            let norm_val = ((*self.value - min_val) / span).clamp(0.0, 1.0);
            let current_angle = start_angle + norm_val * sweep_angle;

            // 1. Radial Glow Highlight on Hover or Drag
            if response.hovered() || response.dragged() {
                let glow_color = if response.dragged() {
                    Color32::from_rgba_unmultiplied(31, 111, 235, 70)
                } else {
                    Color32::from_rgba_unmultiplied(31, 111, 235, 30)
                };
                painter.circle_filled(knob_center, radius + 5.0, glow_color);
            }

            // 2. Detent Tick Marks around perimeter
            if self.detents > 0 {
                for i in 0..=self.detents {
                    let t = i as f32 / self.detents as f32;
                    let tick_angle = start_angle + t * sweep_angle - std::f32::consts::FRAC_PI_2;
                    let dir = vec2(tick_angle.cos(), tick_angle.sin());
                    let inner_pt = knob_center + dir * (radius + 2.0);
                    let outer_pt = knob_center + dir * (radius + 5.0);
                    let tick_color = if t <= norm_val {
                        Color32::from_rgb(31, 111, 235)
                    } else {
                        ui.visuals().weak_text_color()
                    };
                    painter.line_segment([inner_pt, outer_pt], Stroke::new(1.0, tick_color));
                }
            }

            // 3. Knob Body (Outer Bevel and Inner Core Disc)
            let bg_color = ui.visuals().extreme_bg_color;
            painter.circle_filled(knob_center, radius, bg_color);
            painter.circle_stroke(
                knob_center,
                radius,
                Stroke::new(1.5, visuals.bg_stroke.color),
            );

            // 4. Active Value Arc
            let arc_radius = radius * 0.85;
            let arc_steps = (24.0 * norm_val).max(2.0) as usize;
            let arc_stroke = Stroke::new(3.0, Color32::from_rgb(31, 111, 235));
            for i in 0..arc_steps {
                let t0 = i as f32 / arc_steps as f32;
                let t1 = (i + 1) as f32 / arc_steps as f32;
                let a0 = start_angle + t0 * norm_val * sweep_angle - std::f32::consts::FRAC_PI_2;
                let a1 = start_angle + t1 * norm_val * sweep_angle - std::f32::consts::FRAC_PI_2;
                let p0 = knob_center + vec2(a0.cos(), a0.sin()) * arc_radius;
                let p1 = knob_center + vec2(a1.cos(), a1.sin()) * arc_radius;
                painter.line_segment([p0, p1], arc_stroke);
            }

            // 5. Directional Pointer Notch / Needle
            let needle_angle = current_angle - std::f32::consts::FRAC_PI_2;
            let needle_dir = vec2(needle_angle.cos(), needle_angle.sin());
            let needle_start = knob_center + needle_dir * (radius * 0.25);
            let needle_end = knob_center + needle_dir * (radius * 0.75);
            let needle_color = if response.dragged() {
                Color32::WHITE
            } else {
                Color32::from_rgb(220, 230, 245)
            };
            painter.line_segment([needle_start, needle_end], Stroke::new(2.5, needle_color));

            // 6. Title and Numerical Value Readout
            let text_pos = pos2(rect.center().x, rect.min.y + self.diameter + 8.0);
            let label_text = format!("{}: {:.2}", self.title, *self.value);
            painter.text(
                text_pos,
                Align2::CENTER_CENTER,
                label_text,
                FontId::proportional(11.0),
                visuals.text_color(),
            );
        }

        response
    }
}

// ----------------------------------------------------------------------------
// Interactive Lab: Custom Widget Workshop
// ----------------------------------------------------------------------------

fn render_rotary_knob_lab(ui: &mut Ui, state: &mut ManualPlaygroundState) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(Stroke::new(1.0, Color32::from_rgb(31, 111, 235)))
        .inner_margin(16.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("🧪 Interactive Lab: Custom Widget Workshop (RotaryKnob)")
                    .strong()
                    .size(15.0)
                    .color(Color32::from_rgb(31, 111, 235)),
            );
            ui.add_space(4.0);
            ui.label(
                "Drag the rotary knob vertically or horizontally to adjust parameters in real time. \
                 Notice detent snapping, active value arc progression, and radial glow feedback:",
            );
            ui.add_space(14.0);

            let is_narrow = ui.available_width() < 420.0;
            let render_knob_and_diag = |ui: &mut Ui| {
                // Live Custom Widget Instance
                let knob_resp = ui.add(
                    RotaryKnob::new(&mut state.knob_value, 0.0..=100.0)
                        .diameter(80.0)
                        .detents(10)
                        .title("Output Gain"),
                );

                if !is_narrow {
                    ui.add_space(24.0);
                } else {
                    ui.add_space(10.0);
                }

                // Diagnostics and Parameter Controls
                ui.vertical(|ui| {
                    ui.label(RichText::new("Widget Diagnostics:").strong().small());
                    ui.label(format!("• Live Value: {:.2} dB", state.knob_value));
                    ui.label(format!("• Dragging Active: {}", knob_resp.dragged()));
                    ui.label(format!("• Hovered: {}", knob_resp.hovered()));
                    ui.label(format!("• Response Changed: {}", knob_resp.changed()));

                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Preset: 0.0").clicked() {
                            state.knob_value = 0.0;
                        }
                        if ui.button("Preset: 50.0").clicked() {
                            state.knob_value = 50.0;
                        }
                        if ui.button("Preset: 100.0").clicked() {
                            state.knob_value = 100.0;
                        }
                    });
                });
            };

            if is_narrow {
                ui.vertical(render_knob_and_diag);
            } else {
                ui.horizontal(render_knob_and_diag);
            }

            ui.add_space(10.0);
            ui.label(
                RichText::new(
                    "Usage in your application: ui.add(RotaryKnob::new(&mut self.gain, 0.0..=100.0));",
                )
                .small()
                .color(ui.visuals().weak_text_color()),
            );
        });
}
