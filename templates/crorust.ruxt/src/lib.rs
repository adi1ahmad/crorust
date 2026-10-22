#![recursion_limit = "256"]

pub mod app;
pub mod manual;
pub mod markdown;
pub mod navigation;
pub mod platform;
pub mod site;
pub mod ui;
pub mod workspace;

pub use app::RuxtApp;
pub use manual::RuxtManualWindow;
pub use markdown::Editor;
pub use workspace::views::{DisplayCalibration, WidgetShowcase};
pub use workspace::{is_mobile, AppView, View, ViewItem, ViewItem as Demo, WindowManager};

pub const LOREM_IPSUM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";

pub const LOREM_IPSUM_LONG: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.\n\nCurabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam various, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula eu tempor congue, eros est euismod turpis, id tincidunt sapien risus a quam. Maecenas fermentum consequat mi. Donec fermentum. Pellentesque malesuada nulla a mi. Duis sapien sem, aliquet nec, commodo eget, consequat quis, neque. Aliquam faucibus, elit ut dictum aliquet, felis nisl adipiscing sapien, sed malesuada diam lacus eget erat. Cras mollis scelerisque nunc. Nullam arcu. Aliquam consequat. Curabitur augue lorem, dapibus quis, laoreet et, pretium ac, nisi. Aenean magna nisl, mollis quis, molestie eu, feugiat in, orci. In hac habitasse platea dictumst.";

pub(crate) fn seconds_since_midnight() -> f64 {
    jiff::Zoned::now()
        .time()
        .duration_since(jiff::civil::Time::midnight())
        .as_secs_f64()
}

pub fn rust_view_ui(ui: &mut egui::Ui, code: &str) {
    let language = "rs";
    let theme = egui_extras::syntax_highlighting::CodeTheme::from_memory(ui.ctx(), ui.style());
    egui_extras::syntax_highlighting::code_view_ui(ui, &theme, code, language);
}



#[macro_export]
macro_rules! crorust_github_link_file {
    () => {
        $crate::crorust_github_link_file!("(source code)")
    };
    ($label: expr) => {
        egui::github_link_file!(
            "https://github.com/adi1ahmad/crorust/blob/main/templates/crorust.ruxt/",
            egui::RichText::new($label).small()
        )
    };
}

#[macro_export]
macro_rules! crorust_github_link_file_line {
    () => {
        $crate::crorust_github_link_file_line!("(source code)")
    };
    ($label: expr) => {
        egui::github_link_file_line!(
            "https://github.com/adi1ahmad/crorust/blob/main/templates/crorust.ruxt/",
            egui::RichText::new($label).small()
        )
    };
}

pub use crorust_github_link_file as egui_github_link_file;
pub use crorust_github_link_file_line as egui_github_link_file_line;
