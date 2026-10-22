pub mod view_trait;
pub mod views;
pub mod window_manager;

pub use view_trait::{AppView, View, ViewItem, ViewItem as Demo};
pub use window_manager::{WindowManager, DemoWindows};

pub fn is_mobile(ctx: &egui::Context) -> bool {
    let screen_size = ctx.content_rect().size();
    screen_size.x < 550.0
}
