use egui::ImageFit;
use egui::Slider;
use egui::Vec2;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct MediaViewer {
    current_uri: String,
    uri_edit_text: String,
    image_options: egui::ImageOptions,
    chosen_fit: ChosenFit,
    fit: ImageFit,
    maintain_aspect_ratio: bool,
    max_size: Vec2,
    alt_text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
enum ChosenFit {
    ExactSize,
    Fraction,
    OriginalSize,
}

impl ChosenFit {
    fn as_str(&self) -> &'static str {
        match self {
            Self::ExactSize => "exact size",
            Self::Fraction => "fraction",
            Self::OriginalSize => "original size",
        }
    }
}

impl Default for MediaViewer {
    fn default() -> Self {
        Self {
            current_uri: "https://picsum.photos/seed/1.759706314/1024".to_owned(),
            uri_edit_text: "https://picsum.photos/seed/1.759706314/1024".to_owned(),
            image_options: egui::ImageOptions::default(),
            chosen_fit: ChosenFit::Fraction,
            fit: ImageFit::Fraction(Vec2::splat(1.0)),
            maintain_aspect_ratio: true,
            max_size: Vec2::splat(2048.0),
            alt_text: "Loaded Image".to_owned(),
        }
    }
}

impl MediaViewer {
    pub fn window_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let label = ui.label("URI:");
            ui.text_edit_singleline(&mut self.uri_edit_text)
                .labelled_by(label.id);
            if ui.small_button("✔ Load").clicked() {
                ui.ctx().forget_image(&self.current_uri);
                self.uri_edit_text = self.uri_edit_text.trim().to_owned();
                self.current_uri = self.uri_edit_text.clone();
            }

            if let Some(path) = crate::platform::poll_picked_file() {
                self.uri_edit_text = format!("file://{}", path.display());
                self.current_uri = self.uri_edit_text.clone();
            }

            if ui.button("Choose File…").clicked()
                && let Some(path) = crate::platform::pick_file()
            {
                self.uri_edit_text = format!("file://{}", path.display());
                self.current_uri = self.uri_edit_text.clone();
            }
        });

        ui.collapsing("Transform & Fit Options", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Fit Mode:");
                egui::ComboBox::from_id_salt("fit_mode_combo")
                    .selected_text(self.chosen_fit.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.chosen_fit,
                            ChosenFit::ExactSize,
                            ChosenFit::ExactSize.as_str(),
                        );
                        ui.selectable_value(
                            &mut self.chosen_fit,
                            ChosenFit::Fraction,
                            ChosenFit::Fraction.as_str(),
                        );
                        ui.selectable_value(
                            &mut self.chosen_fit,
                            ChosenFit::OriginalSize,
                            ChosenFit::OriginalSize.as_str(),
                        );
                    });
            });

            match self.chosen_fit {
                ChosenFit::ExactSize => {
                    if !matches!(self.fit, ImageFit::Exact(_)) {
                        self.fit = ImageFit::Exact(Vec2::splat(128.0));
                    }
                    let ImageFit::Exact(size) = &mut self.fit else {
                        unreachable!()
                    };
                    ui.add(Slider::new(&mut size.x, 0.0..=2048.0).text("Width"));
                    ui.add(Slider::new(&mut size.y, 0.0..=2048.0).text("Height"));
                }
                ChosenFit::Fraction => {
                    if !matches!(self.fit, ImageFit::Fraction(_)) {
                        self.fit = ImageFit::Fraction(Vec2::splat(1.0));
                    }
                    let ImageFit::Fraction(fract) = &mut self.fit else {
                        unreachable!()
                    };
                    ui.add(Slider::new(&mut fract.x, 0.0..=1.0).text("Width Fraction"));
                    ui.add(Slider::new(&mut fract.y, 0.0..=1.0).text("Height Fraction"));
                }
                ChosenFit::OriginalSize => {
                    if !matches!(self.fit, ImageFit::Original { .. }) {
                        self.fit = ImageFit::Original { scale: 1.0 };
                    }
                    let ImageFit::Original { scale } = &mut self.fit else {
                        unreachable!()
                    };
                    ui.add(Slider::new(scale, 0.1..=4.0).text("Scale"));
                }
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.color_edit_button_srgba(&mut self.image_options.tint);
                ui.label("Color Tint");
                ui.color_edit_button_srgba(&mut self.image_options.bg_fill);
                ui.label("Background Fill");
            });

            ui.add_space(4.0);
            ui.checkbox(&mut self.maintain_aspect_ratio, "Lock Aspect Ratio");

            if ui.button("Clear Texture Cache").clicked() {
                ui.ctx().forget_all_images();
            }
        });

        ui.separator();

        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let mut image = egui::Image::from_uri(&self.current_uri);
            image = image.uv(self.image_options.uv);
            image = image.bg_fill(self.image_options.bg_fill);
            image = image.tint(self.image_options.tint);
            let (angle, origin) = self
                .image_options
                .rotation
                .map_or((0.0, Vec2::splat(0.5)), |(rot, origin)| {
                    (rot.angle(), origin)
                });
            image = image.rotate(angle, origin);
            match self.fit {
                ImageFit::Original { scale } => image = image.fit_to_original_size(scale),
                ImageFit::Fraction(fract) => image = image.fit_to_fraction(fract),
                ImageFit::Exact(size) => image = image.fit_to_exact_size(size),
            }
            image = image.maintain_aspect_ratio(self.maintain_aspect_ratio);
            image = image.max_size(self.max_size);
            if !self.alt_text.is_empty() {
                image = image.alt_text(&self.alt_text);
            }

            ui.add_sized(ui.available_size(), image);
        });
    }
}

impl crate::Demo for MediaViewer {
    fn name(&self) -> &'static str {
        "🖼 Media Viewer"
    }

    fn show(&mut self, ui: &mut egui::Ui, open: &mut bool) {
        egui::Window::new(self.name())
            .open(open)
            .default_size([640.0, 520.0])
            .vscroll(false)
            .resizable(true)
            .constrain_to(ui.available_rect_before_wrap())
            .show(ui, |ui| {
                self.window_ui(ui);
            });
    }
}

impl crate::AppView for MediaViewer {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.window_ui(ui);
        });
    }
}
