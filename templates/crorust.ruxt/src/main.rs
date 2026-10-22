#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::never_loop)]

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
fn main() {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--profile" => {
                #[cfg(feature = "puffin")]
                start_puffin_server();

                #[cfg(not(feature = "puffin"))]
                panic!("Unknown argument: {arg} - enable 'puffin' feature to profile.");
            }
            _ => {
                panic!("Unknown argument: {arg}");
            }
        }
    }

    {

        let mut rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                "debug".to_owned()
            } else {
                "info".to_owned()
            }
        });
        for loud_crate in ["naga", "wgpu_core", "wgpu_hal"] {
            if !rust_log.contains(&format!("{loud_crate}=")) {
                use core::fmt::Write as _;
                write!(rust_log, ",{loud_crate}=warn").ok();
            }
        }

        #[expect(unsafe_code)]
        unsafe {
            std::env::set_var("RUST_LOG", rust_log);
        }
    }

    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 1024.0])
            .with_drag_and_drop(true)
            .with_title("crorust.ruxt"),

        #[cfg(feature = "wgpu")]
        renderer: eframe::Renderer::Wgpu,

        ..Default::default()
    };

    let result = eframe::run_native(
        "ruxt",
        options,
        Box::new(|cc| Ok(Box::new(ruxt::RuxtApp::new(cc)))),
    );

    if let Err(err) = result {
        eprintln!("Error running ruxt: {err}");
        std::process::exit(1);
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "puffin"))]
fn start_puffin_server() {
    puffin::set_scopes_on(true);
    if let Ok(server) = puffin_http::Server::new("127.0.0.1:8585") {
        log::info!("Puffin server running on 127.0.0.1:8585");
        core::mem::forget(server);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window found")
            .document()
            .expect("No document found");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find element with id 'the_canvas_id'")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("Element 'the_canvas_id' was not an HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(ruxt::RuxtApp::new(cc)))),
            )
            .await;

        match start_result {
            Ok(_) => {
                if let Some(loading) = document.get_element_by_id("loading_indicator") {
                    loading.remove();
                }
            }
            Err(e) => {
                if let Some(loading) = document.get_element_by_id("loading_indicator") {
                    loading.set_inner_html(&format!(
                        "<p style=\"color: #ff6b6b;\">Failed to initialize ruxt: {e:?}</p>"
                    ));
                }
                log::error!("Failed to start eframe: {e:?}");
            }
        }
    });
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn main() {
    // Mobile entry points are driven via library bindings / event loops
}
