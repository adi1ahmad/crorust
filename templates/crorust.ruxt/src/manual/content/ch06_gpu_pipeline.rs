use crate::manual::widgets::ManualPlaygroundState;
use egui::epaint::{self, CircleShape, RectShape, TessellationOptions, Tessellator};
use egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Shape, Stroke, StrokeKind,
    Ui, pos2, vec2,
};
use std::sync::Arc;

pub fn render_chapter(
    ui: &mut Ui,
    state: &mut ManualPlaygroundState,
    _target_to_launch: &mut Option<&'static str>,
) {
    // 1. Chapter Header Banner
    ui.heading(
        RichText::new("⚡ The Rendering Pipeline: From Shapes to Hardware Pixels")
            .size(24.0)
            .strong(),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Demystify high-level vector shapes, epaint 20-byte vertex tessellation, \
             1.0px edge-feathered anti-aliasing, and wgpu hardware render passes.",
        )
        .size(13.5)
        .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    // 2. High-Level Architectural SVG Diagram
    let viewer = crate::manual::widgets::diagram_viewer::InteractiveDiagramViewer::new(
        "ch06_gpu_pipeline",
        egui::include_image!("../../../assets/manual/svg/ch06_gpu_pipeline.svg"),
        include_str!("../../../assets/manual/diagrams/ch06_gpu_pipeline.mmd"),
        egui::vec2(721.0, 2494.20),
    );
    viewer.show(ui, state.diagram_state_mut("ch06_gpu_pipeline"));
    ui.add_space(16.0);

    // 3. Subsection 6.1: High-Level Vector Shapes
    render_sub_6_1(ui);
    ui.add_space(20.0);

    // 4. Subsection 6.2: Tessellation & Edge-Feathering AA
    render_sub_6_2(ui);
    ui.add_space(20.0);

    // 5. Subsection 6.3: Hardware Rendering with wgpu
    render_sub_6_3(ui);
    ui.add_space(24.0);

    // 6. Interactive Lab: Tessellation Wireframe Viewer
    render_wireframe_viewer_lab(ui, state);
}

// ----------------------------------------------------------------------------
// Subsection 6.1: High-Level Vector Shapes
// ----------------------------------------------------------------------------

fn render_sub_6_1(ui: &mut Ui) {
    ui.heading(RichText::new("6.1 High-Level Vector Shapes (egui::Shape)").size(18.0));
    ui.add_space(6.0);

    ui.label(
        "Immediate-mode UI closures do NOT issue hardware draw calls while evaluating widgets. \
         Issuing thousands of minuscule draw calls per second directly to Vulkan, Metal, or DirectX \
         would stall the GPU driver and destroy battery life.",
    );
    ui.add_space(6.0);

    ui.label(
        "Instead, calling painter methods records abstract vector instructions into a flat list of egui::Shape enum variants:\n\
         • Shape::Noop — Zero-cost empty placeholder.\n\
         • Shape::Vec(Vec<Shape>) — Nested composite group of shapes.\n\
         • Shape::Circle(CircleShape) — Center, radius, fill color, and perimeter stroke.\n\
         • Shape::LineSegment { points: [Pos2; 2], stroke: Stroke } — Single 2D line segment.\n\
         • Shape::Path(PathShape) — Arbitrary multi-point polyline or closed polygon.\n\
         • Shape::Rect(RectShape) — Axis-aligned rectangle with corner radiuses, fill, stroke, and stroke kind.\n\
         • Shape::Text(TextShape) — Positioned glyph runs referencing the dynamic font atlas.\n\
         • Shape::Mesh(Arc<Mesh>) — Pre-triangulated indexed triangle mesh.\n\
         • Shape::QuadraticBezier, Shape::CubicBezier — Parametric curved splines.\n\n\
         Deferred Shape Collection: Shapes are buffered in CPU RAM until the entire UI pass completes. \
         They are then sorted by Order, grouped by scissor clip rect, and submitted to tessellation in bulk.",
    );
    ui.add_space(8.0);

    crate::rust_view_ui(
        ui,
        r#"use egui::{pos2, Color32, Stroke, StrokeKind};
use egui::epaint::{CircleShape, RectShape, Shape};

// Emitting high-level vector shapes
let mut shapes = Vec::new();

// 1. Circle Primitive
shapes.push(Shape::Circle(CircleShape {
    center: pos2(120.0, 80.0),
    radius: 32.0,
    fill: Color32::from_rgb(31, 111, 235),
    stroke: Stroke::new(1.5, Color32::WHITE),
}));

// 2. Rounded Rectangle with Inset Stroke
shapes.push(Shape::Rect(RectShape::new(
    Rect::from_min_size(pos2(40.0, 40.0), vec2(160.0, 90.0)),
    8.0, // corner radius
    Color32::from_rgb(20, 24, 33),
    Stroke::new(1.0, Color32::from_rgb(70, 80, 95)),
    StrokeKind::Inside,
)));

// Add all shapes in a single atomic submission to the painter
ui.painter().extend(shapes);"#,
    );
}

// ----------------------------------------------------------------------------
// Subsection 6.2: Tessellation & Edge-Feathering AA
// ----------------------------------------------------------------------------

fn render_sub_6_2(ui: &mut Ui) {
    ui.heading(RichText::new("6.2 Tessellation & Edge-Feathering AA (epaint)").size(18.0));
    ui.add_space(6.0);

    ui.label(
        "Modern GPU rasterizers cannot consume circles, rounded rectangles, or bezier curves directly; \
         they only digest indexed triangles. epaint::Tessellator transforms mathematical vector shapes into \
         dense GPU vertex and index buffers.",
    );
    ui.add_space(6.0);

    ui.label(
        RichText::new("The 20-Byte Packed Vertex Stride (epaint::Vertex):")
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(
        "Every single vertex emitted by egui occupies exactly 20 bytes in memory with zero padding bytes:\n\
         • pos: Pos2 -> [f32; 2] (8 bytes) — 2D logical screen coordinates [x, y].\n\
         • uv: Pos2 -> [f32; 2] (8 bytes) — Normalized font atlas or image texture coordinates [u, v] in [0.0, 1.0].\n\
         • color: Color32 -> [u8; 4] (4 bytes) — Pre-multiplied sRGB color channels [R, G, B, A].\n\
         This compact 20-byte footprint allows three complete vertices to fit inside a single 64-byte L1 CPU/GPU cache line.",
    );
    ui.add_space(8.0);

    ui.label(
        RichText::new("Why egui Beats MSAA: Analytic 1.0px Edge-Feathering Anti-Aliasing:")
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(
        "Traditional Multi-Sample Anti-Aliasing (MSAA 4x or 8x) allocates massive 4x/8x multi-sampled framebuffers, \
         burning through VRAM bandwidth and draining mobile battery. epaint solves anti-aliasing analytically in geometry:\n\
         1. The solid interior body of a shape is triangulated with alpha = 1.0.\n\
         2. An outer ribbon of triangles exactly 1.0 pixel wide is extruded outward along the normal vectors of the perimeter.\n\
         3. The outer boundary vertices of this ribbon have alpha set to alpha = 0.0.\n\
         4. Hardware rasterization linearly interpolates alpha from 1.0 to 0.0 across the 1-pixel fringe, achieving \
            silky sub-pixel anti-aliasing on standard 1x buffers with zero MSAA performance overhead.",
    );
    ui.add_space(8.0);

    crate::rust_view_ui(
        ui,
        r#"#[repr(C)]
pub struct Vertex {
    pub pos: Pos2,       // [f32; 2] -> 8 bytes (Logical Screen Position)
    pub uv: Pos2,        // [f32; 2] -> 8 bytes (Atlas Texture Coordinates)
    pub color: Color32,  // [u8; 4]  -> 4 bytes (Pre-multiplied sRGB [R, G, B, A])
}

// Memory layout verification
assert_eq!(std::mem::size_of::<Vertex>(), 20);

// Triangulating a shape with epaint:
let mut mesh = epaint::Mesh::default();
let mut tessellator = epaint::Tessellator::new(
    ui.ctx().pixels_per_point(),
    epaint::TessellationOptions::default(),
    ui.fonts(|f| f.font_image_size()),
    vec![],
);
tessellator.tessellate_circle(&circle_shape, &mut mesh);"#,
    );
}

// ----------------------------------------------------------------------------
// Subsection 6.3: Hardware Rendering with wgpu
// ----------------------------------------------------------------------------

fn render_sub_6_3(ui: &mut Ui) {
    ui.heading(RichText::new("6.3 Hardware Rendering with wgpu (egui-wgpu)").size(18.0));
    ui.add_space(6.0);

    ui.label(
        "The egui-wgpu backend consumes ClippedPrimitive batches from epaint and translates them \
         into native wgpu::RenderPass draw calls on the GPU.",
    );
    ui.add_space(6.0);

    ui.label(
        "1. Dynamic Staging Buffers: Vertex (VBO) and Index (IBO) buffers are uploaded to GPU staging memory \
         using queue.write_buffer() without pipeline stalls, dynamically growing to accommodate large meshes.\n\
         2. Orthographic Projection Matrix: Transforms 2D logical points into Normalized Device Coordinates \
         (NDC [-1.0, 1.0]) with an inverted Y-axis:\n\
         X_ndc = (2.0 * x / screen_width) - 1.0\n\
         Y_ndc = 1.0 - (2.0 * y / screen_height)\n\
         3. Hardware Scissor Testing: Logical clip rectangles are multiplied by the DPI scale factor to compute \
         physical pixel scissor bounds. render_pass.set_scissor_rect() instructs the GPU rasterizer to discard \
         off-screen fragments at zero fragment shader cost.\n\
         4. Pre-Multiplied Alpha: Colors are stored with RGB channels pre-multiplied by alpha. Combined with \
         wgpu blend mode { src: One, dst: OneMinusSrcAlpha, op: Add }, this prevents dark halos around \
         antialiased text and semi-transparent edges.",
    );
    ui.add_space(8.0);

    crate::rust_view_ui(
        ui,
        r#"// Orthographic Projection Matrix Uniform (Column-Major)
let ortho_matrix = [
    [2.0 / screen_width, 0.0, 0.0, 0.0],
    [0.0, -2.0 / screen_height, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0, 1.0],
];

// wgpu Hardware Render Pass Execution Loop
for primitive in clipped_primitives {
    let clip_rect = primitive.clip_rect;
    
    // Convert logical points to physical pixels for hardware scissor testing
    let x = (clip_rect.min.x * scale_factor).max(0.0) as u32;
    let y = (clip_rect.min.y * scale_factor).max(0.0) as u32;
    let width = (clip_rect.width() * scale_factor) as u32;
    let height = (clip_rect.height() * scale_factor) as u32;
    
    render_pass.set_scissor_rect(x, y, width, height);
    render_pass.draw_indexed(primitive.index_range, 0, 0..1);
}"#,
    );
}

// ----------------------------------------------------------------------------
// Interactive Lab: Tessellation Wireframe Viewer
// ----------------------------------------------------------------------------

fn render_wireframe_viewer_lab(ui: &mut Ui, state: &mut ManualPlaygroundState) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(Stroke::new(1.0, Color32::from_rgb(0, 163, 196)))
        .inner_margin(16.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("🧪 Interactive Lab: Tessellation Wireframe Viewer")
                    .strong()
                    .size(15.0)
                    .color(Color32::from_rgb(0, 163, 196)),
            );
            ui.add_space(4.0);
            ui.label(
                "Toggle wireframe mode below to inspect the real-time triangle mesh generated by epaint::Tessellator. \
                 Notice the 1.0px edge-feathering extrusion ribbon highlighted with cyan dots:",
            );
            ui.add_space(12.0);

            // Controls Toolbar
            let is_narrow = ui.available_width() < 420.0;
            if is_narrow {
                ui.vertical(|ui| {
                    ui.checkbox(&mut state.wireframe_enabled, "🔲 Render Triangle Wireframes");
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Shape:");
                        let mut shape_idx = if state.wireframe_tessellation_subdivisions == 0 {
                            0
                        } else {
                            1
                        };
                        if ui.radio_value(&mut shape_idx, 0, "Circle").clicked() {
                            state.wireframe_tessellation_subdivisions = 0;
                        }
                        if ui
                            .radio_value(&mut shape_idx, 1, "Rounded Rectangle")
                            .clicked()
                        {
                            state.wireframe_tessellation_subdivisions = 1;
                        }
                    });
                    ui.add(
                        egui::Slider::new(&mut state.slider_value, 30.0..=90.0).text("Radius / Size"),
                    );
                });
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut state.wireframe_enabled, "🔲 Render Triangle Wireframes");
                    ui.separator();
                    ui.label("Shape:");
                    let mut shape_idx = if state.wireframe_tessellation_subdivisions == 0 {
                        0
                    } else {
                        1
                    };
                    if ui.radio_value(&mut shape_idx, 0, "Circle").clicked() {
                        state.wireframe_tessellation_subdivisions = 0;
                    }
                    if ui
                        .radio_value(&mut shape_idx, 1, "Rounded Rectangle")
                        .clicked()
                    {
                        state.wireframe_tessellation_subdivisions = 1;
                    }
                    ui.separator();
                    ui.add(
                        egui::Slider::new(&mut state.slider_value, 30.0..=90.0).text("Radius / Size"),
                    );
                });
            }
            ui.add_space(12.0);

            // Canvas Surface for Shape & Wireframe Rendering
            let canvas_height = 220.0;
            let (response, painter) =
                ui.allocate_painter(vec2(ui.available_width(), canvas_height), Sense::hover());
            let rect = response.rect;
            let center = rect.center();

            // Background canvas backdrop
            painter.rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(15, 18, 24));
            painter.rect_stroke(
                rect,
                CornerRadius::same(6),
                Stroke::new(1.0, Color32::from_rgb(40, 48, 62)),
                StrokeKind::Inside,
            );

            // Real-Time Tessellation
            let pixels_per_point = ui.ctx().pixels_per_point();
            let mut tessellator = Tessellator::new(
                pixels_per_point,
                TessellationOptions::default(),
                ui.fonts(|f| f.font_image_size()),
                vec![],
            );
            let mut mesh = epaint::Mesh::default();

            if state.wireframe_tessellation_subdivisions == 0 {
                let circle_shape = CircleShape {
                    center,
                    radius: state.slider_value,
                    fill: Color32::from_rgb(31, 111, 235),
                    stroke: Stroke::new(2.0, Color32::from_rgb(120, 180, 255)),
                };
                tessellator.tessellate_circle(circle_shape, &mut mesh);
            } else {
                let shape_rect = Rect::from_center_size(
                    center,
                    vec2(state.slider_value * 2.2, state.slider_value * 1.3),
                );
                let rect_shape = RectShape::new(
                    shape_rect,
                    CornerRadius::same(16),
                    Color32::from_rgb(38, 140, 78),
                    Stroke::new(2.0, Color32::from_rgb(110, 220, 145)),
                    StrokeKind::Inside,
                );
                tessellator.tessellate_rect(&rect_shape, &mut mesh);
            }

            let num_triangles = mesh.triangles().count();
            let num_vertices = mesh.vertices.len();
            let num_indices = mesh.indices.len();
            let memory_bytes = num_vertices * 20 + num_indices * 4;

            // Paint either the shaded mesh or the decomposed wireframe lines
            if !state.wireframe_enabled {
                painter.add(Shape::mesh(Arc::new(mesh)));
            } else {
                // 1. Translucent underlying shaded fill
                let mut faint_mesh = mesh.clone();
                for v in &mut faint_mesh.vertices {
                    v.color = Color32::from_rgba_unmultiplied(
                        v.color.r(),
                        v.color.g(),
                        v.color.b(),
                        35,
                    );
                }
                painter.add(Shape::mesh(Arc::new(faint_mesh)));

                // 2. Bright Magenta Triangle Edge Wireframes
                let wire_stroke = Stroke::new(1.0, Color32::from_rgb(255, 0, 128));
                for tri in mesh.triangles() {
                    let p0 = mesh.vertices[tri[0] as usize].pos;
                    let p1 = mesh.vertices[tri[1] as usize].pos;
                    let p2 = mesh.vertices[tri[2] as usize].pos;

                    painter.line_segment([p0, p1], wire_stroke);
                    painter.line_segment([p1, p2], wire_stroke);
                    painter.line_segment([p2, p0], wire_stroke);
                }

                // 3. Highlight 1.0px Edge-Feathering Vertices (where alpha == 0)
                let mut feathered_count = 0;
                for v in &mesh.vertices {
                    if v.color.a() == 0 {
                        feathered_count += 1;
                        painter.circle_filled(v.pos, 2.0, Color32::from_rgb(0, 220, 255));
                    }
                }

                // Overlay Wireframe Legend
                painter.text(
                    pos2(rect.min.x + 12.0, rect.min.y + 12.0),
                    Align2::LEFT_TOP,
                    format!("Cyan Dots: 1.0px AA Ribbon Vertices ({feathered_count})"),
                    FontId::monospace(11.0),
                    Color32::from_rgb(0, 220, 255),
                );
            }

            ui.add_space(10.0);

            // Live Diagnostics Grid
            let is_narrow_diag = ui.available_width() < 420.0;
            if is_narrow_diag {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Mesh Metrics:").strong().small());
                    ui.label(format!("• Triangles: {num_triangles}"));
                    ui.label(format!("• Vertices: {num_vertices} (20 bytes/vertex)"));
                    ui.label(format!("• Indices: {num_indices} (4 bytes/index)"));
                    ui.label(format!("• GPU VRAM: {memory_bytes} bytes"));
                });
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Mesh Metrics:").strong().small());
                    ui.label(format!("• Triangles: {num_triangles}"));
                    ui.label(format!("• Vertices: {num_vertices} (20 bytes/vertex)"));
                    ui.label(format!("• Indices: {num_indices} (4 bytes/index)"));
                    ui.label(format!("• GPU VRAM: {memory_bytes} bytes"));
                });
            }
        });
}
