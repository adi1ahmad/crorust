use egui::Image;
use poll_promise::Promise;

struct Resource {
    response: ehttp::Response,
    text: Option<String>,
    image: Option<Image<'static>>,
    colored_text: Option<ColoredText>,
}

impl Resource {
    fn from_response(ctx: &egui::Context, response: ehttp::Response) -> Self {
        let content_type = response.content_type().unwrap_or_default();
        if content_type.starts_with("image/") {
            ctx.include_bytes(response.url.clone(), response.bytes.clone());
            let image = Image::from_uri(response.url.clone());

            Self {
                response,
                text: None,
                colored_text: None,
                image: Some(image),
            }
        } else {
            let text = response.text();
            let colored_text = text.and_then(|text| syntax_highlighting(ctx, &response, text));
            let text = text.map(|text| text.to_owned());

            Self {
                response,
                text,
                colored_text,
                image: None,
            }
        }
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct NetworkClient {
    url: String,

    #[cfg_attr(feature = "serde", serde(skip))]
    promise: Option<Promise<ehttp::Result<Resource>>>,
}

impl NetworkClient {
    pub fn new() -> Self {
        Self {
            url: "https://raw.githubusercontent.com/adi1ahmad/crorust/main/README.md".to_owned(),
            promise: None,
        }
    }

    pub fn window_ui(&mut self, ui: &mut egui::Ui) {
        if self.url.is_empty() {
            self.url =
                "https://raw.githubusercontent.com/adi1ahmad/crorust/main/README.md".to_owned();
        }

        let prev_url = self.url.clone();
        let trigger_fetch = ui_url(ui, &mut self.url);

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label("Asynchronous HTTP requests via ");
            ui.hyperlink_to("ehttp", "https://github.com/emilk/ehttp");
            ui.label(" inside crorust.");
        });

        if trigger_fetch {
            let ctx = ui.ctx().clone();
            let (sender, promise) = Promise::new();
            let request = ehttp::Request::get(&self.url);
            ehttp::fetch(request, move |response| {
                ctx.forget_image(&prev_url);
                ctx.request_repaint();
                let resource = response.map(|response| Resource::from_response(&ctx, response));
                sender.send(resource);
            });
            self.promise = Some(promise);
        }

        ui.separator();

        if let Some(promise) = &self.promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(resource) => {
                        ui_resource(ui, resource);
                    }
                    Err(error) => {
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            if error.is_empty() {
                                "Request failed"
                            } else {
                                error
                            },
                        );
                    }
                }
            } else {
                ui.spinner();
            }
        }
    }
}

impl crate::Demo for NetworkClient {
    fn name(&self) -> &'static str {
        "🌐 Network Client"
    }

    fn show(&mut self, ui: &mut egui::Ui, open: &mut bool) {
        egui::Window::new(self.name())
            .open(open)
            .default_size([580.0, 620.0])
            .vscroll(true)
            .resizable(true)
            .constrain_to(ui.available_rect_before_wrap())
            .show(ui, |ui| {
                self.window_ui(ui);
            });
    }
}

impl crate::AppView for NetworkClient {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.window_ui(ui);
        });
    }
}

fn ui_url(ui: &mut egui::Ui, url: &mut String) -> bool {
    let mut trigger_fetch = false;

    ui.horizontal(|ui| {
        let label = ui.label("URL:");
        trigger_fetch |= ui
            .add(egui::TextEdit::singleline(url).desired_width(f32::INFINITY))
            .labelled_by(label.id)
            .lost_focus();
    });

    if cfg!(target_arch = "wasm32") {
        ui.label("HINT: paste an accessible CORS-enabled URL into the field above.");
    }

    ui.horizontal(|ui| {
        if ui.button("Load crorust README").clicked() {
            *url = "https://raw.githubusercontent.com/adi1ahmad/crorust/main/README.md".to_owned();
            trigger_fetch = true;
        }
        if ui.button("Random image").clicked() {
            let seed = ui.input(|i| i.time);
            let side = 640;
            *url = format!("https://picsum.photos/seed/{seed}/{side}");
            trigger_fetch = true;
        }
    });

    trigger_fetch
}

fn ui_resource(ui: &mut egui::Ui, resource: &Resource) {
    let Resource {
        response,
        text,
        image,
        colored_text,
    } = resource;

    ui.monospace(format!("url:          {}", response.url));
    ui.monospace(format!(
        "status:       {} ({})",
        response.status, response.status_text
    ));
    ui.monospace(format!(
        "content-type: {}",
        response.content_type().unwrap_or_default()
    ));
    ui.monospace(format!(
        "size:         {:.1} kB",
        response.bytes.len() as f32 / 1000.0
    ));

    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            egui::CollapsingHeader::new("Response headers")
                .default_open(false)
                .show(ui, |ui| {
                    egui::Grid::new("response_headers")
                        .spacing(egui::vec2(ui.spacing().item_spacing.x * 2.0, 0.0))
                        .show(ui, |ui| {
                            for (k, v) in &response.headers {
                                ui.label(k);
                                ui.label(v);
                                ui.end_row();
                            }
                        })
                });

            ui.separator();

            if let Some(text) = &text {
                if ui.button("📋 Copy Response Text").clicked() {
                    ui.copy_text(text.clone());
                }
                ui.separator();
            }

            if let Some(image) = image {
                ui.add(image.clone());
            } else if let Some(colored_text) = colored_text {
                colored_text.ui(ui);
            } else if let Some(text) = &text {
                ui.add(egui::Label::new(text).selectable(true));
            } else {
                ui.monospace("[binary content]");
            }
        });
}

fn syntax_highlighting(
    ctx: &egui::Context,
    response: &ehttp::Response,
    text: &str,
) -> Option<ColoredText> {
    let extension_and_rest: Vec<&str> = response.url.rsplitn(2, '.').collect();
    let extension = extension_and_rest.first()?;
    let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(&ctx.global_style());
    Some(ColoredText(egui_extras::syntax_highlighting::highlight(
        ctx,
        &ctx.global_style(),
        &theme,
        text,
        extension,
    )))
}

struct ColoredText(egui::text::LayoutJob);

impl ColoredText {
    pub fn ui(&self, ui: &mut egui::Ui) {
        let mut job = self.0.clone();
        job.wrap.max_width = ui.available_width();
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        ui.add(egui::Label::new(galley).selectable(true));
    }
}
