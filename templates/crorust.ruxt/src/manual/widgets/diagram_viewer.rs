use egui::load::{SizeHint, TexturePoll};
use egui::{
    Color32, CornerRadius, CursorIcon, Frame, ImageSource, Rect, RichText, Sense, Stroke,
    TextureOptions, Ui, Vec2, vec2,
};

#[derive(Debug, Clone)]
pub struct DiagramViewerState {
    pub zoom: f32,
    pub pan: Vec2,
    pub is_fullscreen: bool,
    pub show_source: bool,
    pub pan_active: bool,
    pub last_interaction_time: f64,
    pub active_lod: f32,
}

impl DiagramViewerState {
    /// Resets all pan, zoom, and interaction state back to initial default view.
    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.is_fullscreen = false;
        self.show_source = false;
        self.pan_active = false;
        self.last_interaction_time = 0.0;
        self.active_lod = 1.5;
    }

    /// Clamps zoom level within safe rendering bounds [0.5, 4.0].
    pub fn clamp_zoom(&mut self) {
        self.zoom = self.zoom.clamp(0.5, 4.0);
    }
}

impl Default for DiagramViewerState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            is_fullscreen: false,
            show_source: false,
            pan_active: false,
            last_interaction_time: 0.0,
            active_lod: 1.5,
        }
    }
}

pub struct InteractiveDiagramViewer<'a> {
    id_source: &'static str,
    image_source: ImageSource<'a>,
    raw_mermaid: &'static str,
    native_size: Vec2,
    badges: &'a [(&'static str, &'static str)], // (Label, Target Section Anchor)
}

impl<'a> InteractiveDiagramViewer<'a> {
    pub fn new(
        id_source: &'static str,
        image_source: ImageSource<'a>,
        raw_mermaid: &'static str,
        native_size: Vec2,
    ) -> Self {
        Self {
            id_source,
            image_source,
            raw_mermaid,
            native_size,
            badges: &[],
        }
    }

    pub fn with_badges(mut self, badges: &'a [(&'static str, &'static str)]) -> Self {
        self.badges = badges;
        self
    }

    pub fn show(&self, ui: &mut Ui, state: &mut DiagramViewerState) -> Option<&'static str> {
        let mut clicked_badge = None;
        let is_mobile = crate::is_mobile(ui.ctx());

        ui.vertical(|ui| {
            // 1. Interactive Control Toolbar
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("🔍 Diagram Controls:").strong().small());

                if ui.button(RichText::new("➖").small()).clicked() {
                    state.zoom = (state.zoom - 0.2).max(0.5);
                }
                ui.label(
                    RichText::new(format!("{:.0}%", state.zoom * 100.0))
                        .monospace()
                        .small(),
                );
                if ui.button(RichText::new("➕").small()).clicked() {
                    state.zoom = (state.zoom + 0.2).min(4.0);
                }
                if ui.button(RichText::new("1:1 Reset").small()).clicked() {
                    state.zoom = 1.0;
                    state.pan = Vec2::ZERO;
                }
                if ui
                    .button(
                        RichText::new(if state.is_fullscreen {
                            "🗗 Exit Lightbox"
                        } else {
                            "⛶ Expand"
                        })
                        .small(),
                    )
                    .clicked()
                {
                    state.is_fullscreen = !state.is_fullscreen;
                }

                // On mobile, offer an explicit toggle so the user knows whether dragging pans the diagram or scrolls the page
                if is_mobile && !state.is_fullscreen {
                    let pan_btn_text = if state.pan_active {
                        "✋ Pan (Active)"
                    } else {
                        "👆 Scroll Page"
                    };
                    if ui
                        .selectable_label(state.pan_active, RichText::new(pan_btn_text).small())
                        .on_hover_text(if state.pan_active {
                            "Touch dragging pans diagram (page scroll locked)"
                        } else {
                            "Touch dragging scrolls manual page (diagram pan locked)"
                        })
                        .clicked()
                    {
                        state.pan_active = !state.pan_active;
                    }
                }

                if ui.button(RichText::new("📋 Copy Source").small()).clicked() {
                    ui.ctx().copy_text(self.raw_mermaid.to_owned());
                }
                if ui
                    .selectable_label(state.show_source, RichText::new("Source Code").small())
                    .clicked()
                {
                    state.show_source = !state.show_source;
                }
            });

            ui.add_space(4.0);

            // 2. Viewport Canvas with ClipRect
            let canvas_width = ui.available_width().max(280.0);
            let canvas_height =
                (self.native_size.y * (canvas_width / self.native_size.x)).clamp(220.0, 480.0);

            // On mobile touchscreens, if pan_active is false and we are not in fullscreen lightbox,
            // allocate Sense::click() so normal single-finger vertical swipes smoothly scroll the parent page!
            let sense = if is_mobile && !state.pan_active && !state.is_fullscreen {
                Sense::click()
            } else {
                Sense::click_and_drag()
            };

            let (rect, response) = ui.allocate_exact_size(vec2(canvas_width, canvas_height), sense);

            // Double tap / double click to reset zoom and pan back to 1:1
            if response.double_clicked() {
                state.zoom = 1.0;
                state.pan = Vec2::ZERO;
            }

            let current_time = ui.input(|i| i.time);
            let mut is_interacting = response.dragged();

            // Handle Drag Panning
            if response.dragged() {
                state.pan += response.drag_delta();
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }

            // Tapping on mobile while in scroll mode prompts lightbox modal for immersive inspection
            if response.clicked() && is_mobile && !state.pan_active {
                state.is_fullscreen = true;
            }

            // Handle Scroll Zooming with Ctrl / Cmd modifier on desktop
            if response.hovered() {
                let (modifiers, scroll) = ui.input(|i| (i.modifiers, i.smooth_scroll_delta.y));
                if (modifiers.ctrl || modifiers.command) && scroll.abs() > 0.0 {
                    is_interacting = true;
                    let zoom_factor = (scroll * 0.002).exp();
                    state.zoom = (state.zoom * zoom_factor).clamp(0.5, 4.0);
                }

                // Handle multi-touch pinch zoom (works seamlessly on mobile even when pan_active is false)
                if let Some(multi_touch) = ui.input(|i| i.multi_touch()) {
                    let zoom_delta = multi_touch.zoom_delta;
                    if zoom_delta > 0.0 && (zoom_delta - 1.0).abs() > 0.005 {
                        is_interacting = true;
                        state.zoom = (state.zoom * zoom_delta).clamp(0.5, 4.0);
                    }
                }
            }

            if is_interacting {
                state.last_interaction_time = current_time;
            } else if current_time - state.last_interaction_time > 0.25 {
                // Settle to a discrete LOD tier when the user pauses motion:
                let desired_lod = if state.zoom <= 1.0 {
                    1.5
                } else if state.zoom <= 1.6 {
                    2.0
                } else if state.zoom <= 2.4 {
                    2.5
                } else {
                    3.0
                };
                if (state.active_lod - desired_lod).abs() > 0.1 {
                    state.active_lod = desired_lod;
                }
            }

            // Draw Background and Scissor Scoped Image
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(6),
                if ui.visuals().dark_mode {
                    Color32::from_rgb(15, 23, 42)
                } else {
                    Color32::from_rgb(245, 247, 250)
                },
            );
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(6),
                Stroke::new(1.0, ui.visuals().window_stroke().color),
                egui::StrokeKind::Inside,
            );

            ui.scope(|ui| {
                ui.set_clip_rect(rect.intersect(ui.clip_rect()));

                // Use a fixed stable rasterization size hint based on native_size * active_lod.
                // Dynamic zoom/pan is performed purely via GPU quad transformation (ui.painter().image)
                // without invoking usvg/simplecss on the CPU during gestures!
                let target_raster_size =
                    if self.native_size.x > 2048.0 || self.native_size.y > 2048.0 {
                        self.native_size
                    } else {
                        self.native_size * state.active_lod
                    };

                let pixels_per_point = ui.pixels_per_point();
                let raster_w = (target_raster_size.x * pixels_per_point)
                    .round()
                    .clamp(1.0, 4096.0) as u32;
                let raster_h = (target_raster_size.y * pixels_per_point)
                    .round()
                    .clamp(1.0, 4096.0) as u32;

                let texture_res = self.image_source.clone().load(
                    ui.ctx(),
                    TextureOptions::LINEAR,
                    SizeHint::Size {
                        width: raster_w,
                        height: raster_h,
                        maintain_aspect_ratio: true,
                    },
                );

                // Derive effective layout dimensions from the real texture when ready,
                // avoiding any aspect-ratio mismatch between caller estimates and the SVG's true viewBox.
                let effective_size = match &texture_res {
                    Ok(TexturePoll::Ready { texture })
                        if texture.size.x > 0.0 && texture.size.y > 0.0 =>
                    {
                        texture.size
                    }
                    _ => self.native_size,
                };

                let fit_scale =
                    (canvas_width / effective_size.x).min(canvas_height / effective_size.y);
                let display_size = effective_size * fit_scale * state.zoom;
                let centered_pos = rect.center() - (display_size / 2.0) + state.pan;
                let image_rect = Rect::from_min_size(centered_pos, display_size);

                match texture_res {
                    Ok(TexturePoll::Ready { texture }) => {
                        ui.painter().image(
                            texture.id,
                            image_rect,
                            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    Ok(TexturePoll::Pending { .. }) => {
                        egui::Spinner::new().paint_at(ui, image_rect);
                    }
                    Err(_) => {
                        ui.painter().rect_filled(
                            image_rect,
                            CornerRadius::same(4),
                            Color32::from_black_alpha(40),
                        );
                    }
                }
            });

            // Hint for mobile users when scrolling over diagram
            if is_mobile && !state.pan_active && !state.is_fullscreen {
                ui.add_space(2.0);
                ui.label(
                    RichText::new("💡 Tap diagram or 'Expand' to inspect in fullscreen lightbox.")
                        .small()
                        .weak(),
                );
            }

            // 3. Optional Jump-Link Badges
            if !self.badges.is_empty() {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Jump to Subsystem:").small().weak());
                    for (label, anchor) in self.badges {
                        if ui
                            .button(RichText::new(format!("🔗 {label}")).small())
                            .clicked()
                        {
                            clicked_badge = Some(*anchor);
                        }
                    }
                });
            }

            // 4. Collapsible Raw Mermaid Source Box
            if state.show_source {
                ui.add_space(6.0);
                Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(
                        RichText::new("Mermaid Specification (.mmd):")
                            .strong()
                            .small(),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt(format!("{}_mermaid_source_scroll", self.id_source))
                        .max_height(140.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(self.raw_mermaid)
                                        .monospace()
                                        .size(11.5)
                                        .color(ui.visuals().text_color()),
                                )
                                .wrap(),
                            );
                        });
                });
            }
        });

        // 5. Fullscreen Lightbox Modal
        if state.is_fullscreen {
            let screen_rect = ui.ctx().content_rect();
            egui::Window::new(format!("🔍 Lightbox: {}", self.id_source))
                .fixed_rect(screen_rect.shrink(24.0))
                .collapsible(false)
                .open(&mut state.is_fullscreen)
                .show(ui.ctx(), |ui| {
                    ui.label(
                        "Drag to pan, scroll or pinch to zoom. Use '1:1 Reset' in toolbar to re-center.",
                    );
                    let available = ui.available_size();
                    let (full_rect, full_resp) =
                        ui.allocate_exact_size(available, Sense::click_and_drag());

                    if full_resp.double_clicked() {
                        state.zoom = 1.0;
                        state.pan = Vec2::ZERO;
                    }

                    let mut is_lightbox_interacting = full_resp.dragged();

                    if full_resp.dragged() {
                        state.pan += full_resp.drag_delta();
                    }
                    if full_resp.hovered() {
                        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                        if scroll.abs() > 0.0 {
                            is_lightbox_interacting = true;
                            let zoom_factor = (scroll * 0.002).exp();
                            state.zoom = (state.zoom * zoom_factor).clamp(0.5, 5.0);
                        }
                        if let Some(multi_touch) = ui.input(|i| i.multi_touch()) {
                            let zoom_delta = multi_touch.zoom_delta;
                            if zoom_delta > 0.0 && (zoom_delta - 1.0).abs() > 0.005 {
                                is_lightbox_interacting = true;
                                state.zoom = (state.zoom * zoom_delta).clamp(0.5, 5.0);
                            }
                        }
                    }

                    let current_time = ui.input(|i| i.time);
                    if is_lightbox_interacting {
                        state.last_interaction_time = current_time;
                    }

                    ui.scope(|ui| {
                        ui.set_clip_rect(full_rect.intersect(ui.clip_rect()));
                        let target_raster_size =
                            if self.native_size.x > 2048.0 || self.native_size.y > 2048.0 {
                                self.native_size
                            } else {
                                self.native_size * state.active_lod.max(2.0)
                            };

                        let pixels_per_point = ui.pixels_per_point();
                        let raster_w = (target_raster_size.x * pixels_per_point).round().clamp(1.0, 4096.0) as u32;
                        let raster_h = (target_raster_size.y * pixels_per_point).round().clamp(1.0, 4096.0) as u32;

                        let texture_res = self.image_source.clone().load(
                            ui.ctx(),
                            TextureOptions::LINEAR,
                            SizeHint::Size {
                                width: raster_w,
                                height: raster_h,
                                maintain_aspect_ratio: true,
                            },
                        );

                        let effective_size = match &texture_res {
                            Ok(TexturePoll::Ready { texture })
                                if texture.size.x > 0.0 && texture.size.y > 0.0 =>
                            {
                                texture.size
                            }
                            _ => self.native_size,
                        };

                        let avail_scale = (full_rect.width() / effective_size.x)
                            .min(full_rect.height() / effective_size.y);
                        let full_display = effective_size * avail_scale * state.zoom;
                        let full_pos = full_rect.center() - (full_display / 2.0) + state.pan;
                        let full_img_rect = Rect::from_min_size(full_pos, full_display);

                        match texture_res {
                            Ok(TexturePoll::Ready { texture }) => {
                                ui.painter().image(
                                    texture.id,
                                    full_img_rect,
                                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                            Ok(TexturePoll::Pending { .. }) => {
                                egui::Spinner::new().paint_at(ui, full_img_rect);
                            }
                            Err(_) => {
                                ui.painter().rect_filled(
                                    full_img_rect,
                                    CornerRadius::same(4),
                                    Color32::from_black_alpha(40),
                                );
                            }
                        }
                    });
                });
        }

        clicked_badge
    }
}
