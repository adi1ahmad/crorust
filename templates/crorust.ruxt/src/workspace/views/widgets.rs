#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
enum Enum {
    First,
    Second,
    Third,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct WidgetShowcase {
    enabled: bool,
    visible: bool,
    boolean: bool,
    opacity: f32,
    radio: Enum,
    scalar: f32,
    range: egui::Rangef,
    string: String,
    color: egui::Color32,
    animate_progress_bar: bool,

    #[cfg_attr(feature = "serde", serde(skip))]
    date: Option<jiff::civil::Date>,

    with_date_button: bool,
}

pub type WidgetGallery = WidgetShowcase;

impl Default for WidgetShowcase {
    fn default() -> Self {
        Self {
            enabled: true,
            visible: true,
            opacity: 1.0,
            boolean: false,
            radio: Enum::First,
            scalar: 42.0,
            range: egui::Rangef::new(90.0, 250.0),
            string: Default::default(),
            color: egui::Color32::LIGHT_BLUE.linear_multiply(0.5),
            animate_progress_bar: false,
            date: None,
            with_date_button: true,
        }
    }
}

impl WidgetShowcase {
    #[inline]
    pub fn with_date_button(mut self, with_date_button: bool) -> Self {
        self.with_date_button = with_date_button;
        self
    }
}

impl crate::Demo for WidgetShowcase {
    fn name(&self) -> &'static str {
        "📦 Widget Showcase"
    }

    fn show(&mut self, ui: &mut egui::Ui, open: &mut bool) {
        egui::Window::new(self.name())
            .open(open)
            .resizable(true)
            .scroll([true, true])
            .default_size([340.0, 480.0])
            .constrain_to(ui.available_rect_before_wrap())
            .show(ui, |ui| {
                use crate::View as _;
                self.ui(ui);
            });
    }
}

impl crate::View for WidgetShowcase {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let mut ui_builder = egui::UiBuilder::new();
        if !self.enabled {
            ui_builder = ui_builder.disabled();
        }
        if !self.visible {
            ui_builder = ui_builder.invisible();
        }

        let is_compact = crate::is_mobile(ui.ctx()) || ui.available_width() < 400.0;
        let grid_spacing = if is_compact {
            [16.0, 4.0]
        } else {
            [40.0, 4.0]
        };

        ui.scope_builder(ui_builder, |ui| {
            ui.multiply_opacity(self.opacity);

            egui::Grid::new("my_grid")
                .num_columns(2)
                .spacing(grid_spacing)
                .striped(true)
                .show(ui, |ui| {
                    self.gallery_grid_contents(ui);
                });
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.visible, "Visible")
                .on_hover_text("Uncheck to hide all the widgets.");
            if self.visible {
                ui.checkbox(&mut self.enabled, "Interactive")
                    .on_hover_text("Uncheck to inspect how the widgets look when disabled.");
                let drag_value = ui.add(
                    egui::DragValue::new(&mut self.opacity)
                        .speed(0.01)
                        .range(0.0..=1.0),
                );
                let label = ui.label("Opacity");
                (drag_value.labelled_by(label.id) | label)
                    .on_hover_text("Reduce this value to make widgets semi-transparent");
            }
        });

        ui.separator();

        ui.vertical_centered(|ui| {
            let tooltip_text = "The full egui documentation.\nYou can also click the different widgets names in the left column.";
            ui.hyperlink("https://docs.rs/egui/").on_hover_text(tooltip_text);
            ui.add(crate::egui_github_link_file!(
                "Source code of the widget showcase"
            ));
        });
    }
}

impl WidgetShowcase {
    fn gallery_grid_contents(&mut self, ui: &mut egui::Ui) {
        let Self {
            enabled: _,
            visible: _,
            opacity: _,
            boolean,
            radio,
            scalar,
            range,
            string,
            color,
            animate_progress_bar,
            date,
            with_date_button,
        } = self;

        ui.add(doc_link_label("Label", "label"));
        ui.label("Welcome to the widget gallery!");
        ui.end_row();

        ui.add(doc_link_label("Hyperlink", "Hyperlink"));
        use egui::special_emojis::GITHUB;
        ui.hyperlink_to(
            format!("{GITHUB} crorust on GitHub"),
            "https://github.com/adi1ahmad/crorust",
        );
        ui.end_row();

        ui.add(doc_link_label("TextEdit", "TextEdit"));
        ui.add(egui::TextEdit::singleline(string).hint_text("Write something here"));
        ui.end_row();

        ui.add(doc_link_label("Button", "button"));
        if ui.button("Click me!").clicked() {
            *boolean = !*boolean;
        }
        ui.end_row();

        ui.add(doc_link_label("Link", "link"));
        if ui.link("Click me!").clicked() {
            *boolean = !*boolean;
        }
        ui.end_row();

        ui.add(doc_link_label("Checkbox", "checkbox"));
        ui.checkbox(boolean, "Checkbox");
        ui.end_row();

        ui.add(doc_link_label("RadioButton", "radio"));
        ui.horizontal(|ui| {
            ui.radio_value(radio, Enum::First, "First");
            ui.radio_value(radio, Enum::Second, "Second");
            ui.radio_value(radio, Enum::Third, "Third");
        });
        ui.end_row();

        ui.add(doc_link_label("SelectableLabel", "SelectableLabel"));
        ui.horizontal(|ui| {
            ui.selectable_value(radio, Enum::First, "First");
            ui.selectable_value(radio, Enum::Second, "Second");
            ui.selectable_value(radio, Enum::Third, "Third");
        });
        ui.end_row();

        ui.add(doc_link_label("ComboBox", "ComboBox"));

        egui::ComboBox::from_label("Take your pick")
            .selected_text(format!("{radio:?}"))
            .show_ui(ui, |ui| {
                ui.selectable_value(radio, Enum::First, "First");
                ui.selectable_value(radio, Enum::Second, "Second");
                ui.selectable_value(radio, Enum::Third, "Third");
            });
        ui.end_row();

        let label = ui.add(doc_link_label("Slider", "Slider"));
        ui.add(egui::Slider::new(scalar, 0.0..=360.0).suffix("°"))
            .labelled_by(label.id);
        ui.end_row();

        let label = ui.add(doc_link_label("Range", "Slider"));
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut range.min, 0.0..=360.0).prefix("min: "));
            ui.add(egui::Slider::new(&mut range.max, 0.0..=360.0).prefix("max: "));
        })
        .response
        .labelled_by(label.id);
        ui.end_row();

        let label = ui.add(doc_link_label("DragValue", "DragValue"));
        ui.add(egui::DragValue::new(scalar).speed(1.0))
            .labelled_by(label.id);
        ui.end_row();

        ui.add(doc_link_label("ProgressBar", "ProgressBar"));
        let progress = *scalar / 360.0;
        let progress_bar = egui::ProgressBar::new(progress)
            .show_percentage()
            .animate(*animate_progress_bar);
        *animate_progress_bar = ui
            .add(progress_bar)
            .on_hover_text("The progress bar can be animated!")
            .hovered();
        ui.end_row();

        ui.add(doc_link_label("Color picker", "color_edit"));
        ui.color_edit_button_srgba(color);
        ui.end_row();

        ui.add(doc_link_label("Image", "Image"));
        let egui_icon = egui::include_image!("../../../assets/icon.svg");
        ui.add(egui::Image::new(egui_icon.clone()));
        ui.end_row();

        ui.add(doc_link_label(
            "Button with image",
            "Button::image_and_text",
        ));
        if ui.button((egui_icon, "Click me!")).clicked() {
            *boolean = !*boolean;
        }
        ui.end_row();

        if *with_date_button {
            let date = date.get_or_insert_with(|| jiff::Zoned::now().date());
            ui.add(doc_link_label_with_crate(
                "egui_extras",
                "DatePickerButton",
                "DatePickerButton",
            ));
            ui.add(egui_extras::DatePickerButton::new(date));
            ui.end_row();
        }

        ui.add(doc_link_label("Separator", "separator"));
        ui.separator();
        ui.end_row();

        ui.add(doc_link_label("CollapsingHeader", "collapsing"));
        ui.collapsing("Click to see what is hidden!", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("It's a ");
                ui.add(doc_link_label("Spinner", "spinner"));
                ui.add_space(4.0);
                ui.add(egui::Spinner::new());
            });
        });
        ui.end_row();

        ui.hyperlink_to(
            "Custom widget",
            super::toggle_switch::url_to_file_source_code(),
        );
        ui.add(super::toggle_switch::toggle(boolean)).on_hover_text(
            "It's easy to create your own widgets!\n\
            This toggle switch is just 15 lines of code.",
        );
        ui.end_row();
    }
}

fn doc_link_label<'a>(title: &'a str, search_term: &'a str) -> impl egui::Widget + 'a {
    doc_link_label_with_crate("egui", title, search_term)
}

fn doc_link_label_with_crate<'a>(
    crate_name: &'a str,
    title: &'a str,
    search_term: &'a str,
) -> impl egui::Widget + 'a {
    let url = format!("https://docs.rs/{crate_name}?search={search_term}");
    move |ui: &mut egui::Ui| {
        ui.hyperlink_to(title, url).on_hover_ui(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Search egui docs for");
                ui.code(search_term);
            });
        })
    }
}
