pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}

pub trait ViewItem {
    fn is_enabled(&self, _ctx: &egui::Context) -> bool {
        true
    }

    fn name(&self) -> &'static str;

    fn show(&mut self, ui: &mut egui::Ui, open: &mut bool);

    fn logic(&mut self, _ctx: &egui::Context) {}
}

pub trait AppView {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame);

    fn logic(&mut self, _ctx: &egui::Context) {}

    #[cfg(feature = "glow")]
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {}
}
