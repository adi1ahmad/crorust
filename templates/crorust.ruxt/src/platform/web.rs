use crate::RuxtApp;
use eframe::wasm_bindgen::{self, prelude::*};

#[derive(Clone)]
#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[wasm_bindgen]
impl WebHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let log_level = if cfg!(debug_assertions) {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Debug
        };
        eframe::WebLogger::init(log_level).ok();

        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    #[wasm_bindgen]
    pub async fn start(
        &self,
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<(), wasm_bindgen::JsValue> {
        self.runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(RuxtApp::new(cc)))),
            )
            .await
    }

    #[wasm_bindgen]
    pub fn destroy(&self) {
        self.runner.destroy();
    }

    #[wasm_bindgen]
    pub fn has_panicked(&self) -> bool {
        self.runner.has_panicked()
    }

    #[wasm_bindgen]
    pub fn panic_message(&self) -> Option<String> {
        self.runner.panic_summary().map(|s| s.message())
    }

    #[wasm_bindgen]
    pub fn panic_callstack(&self) -> Option<String> {
        self.runner.panic_summary().map(|s| s.callstack())
    }
}

#[cfg(target_arch = "wasm32")]
pub fn trigger_web_file_picker() {
    use wasm_bindgen::JsCast;
    use web_sys::{FileReader, HtmlInputElement};

    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };

    let (input, needs_setup) = if let Some(existing) =
        document.get_element_by_id("ruxt_hidden_file_picker")
    {
        let Ok(inp) = existing.dyn_into::<HtmlInputElement>() else {
            return;
        };
        (inp, false)
    } else {
        let Ok(element) = document.create_element("input") else {
            return;
        };
        element.set_id("ruxt_hidden_file_picker");
        let Ok(inp) = element.dyn_into::<HtmlInputElement>() else {
            return;
        };
        inp.set_type("file");
        inp.set_accept(".png,.jpg,.jpeg,.svg,.webp,.gif,.bmp,.md,.markdown,.txt,.toml,.rs,.json");
        let _ = inp.style().set_property("display", "none");
        if let Some(body) = document.body() {
            let _ = body.append_child(&inp);
        }
        (inp, true)
    };

    // Reset previous selection so selecting the same file again still fires change
    input.set_value("");

    if needs_setup {
        let on_change = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let Some(target) = event.target() else {
                return;
            };
            let Ok(input) = target.dyn_into::<HtmlInputElement>() else {
                return;
            };
            if let Some(files) = input.files()
                && let Some(file) = files.get(0)
            {
                let file_name = file.name();
                let Ok(reader) = FileReader::new() else {
                    return;
                };
                let reader_clone = reader.clone();

                let on_load = Closure::once_into_js(move |_: web_sys::Event| {
                    if let Ok(result) = reader_clone.result() {
                        let array = js_sys::Uint8Array::new(&result);
                        let bytes = array.to_vec();
                        super::dialog::deliver_picked_file(super::dialog::PickedFile::new(
                            file_name, bytes,
                        ));
                    }
                });

                reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
                let _ = reader.read_as_array_buffer(&file);
            }
        }) as Box<dyn FnMut(_)>);

        input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
        on_change.forget();
    }

    input.click();
}
