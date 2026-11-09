use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct PickedFile {
    pub name: String,
    pub bytes: Vec<u8>,
    pub mime_type: Option<String>,
}

impl PickedFile {
    pub fn new(name: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            bytes,
            mime_type: None,
        }
    }

    pub fn is_image(&self) -> bool {
        let name_lower = self.name.to_lowercase();
        name_lower.ends_with(".png")
            || name_lower.ends_with(".jpg")
            || name_lower.ends_with(".jpeg")
            || name_lower.ends_with(".svg")
            || name_lower.ends_with(".webp")
            || name_lower.ends_with(".gif")
            || name_lower.ends_with(".bmp")
    }

    pub fn is_markdown(&self) -> bool {
        let name_lower = self.name.to_lowercase();
        name_lower.ends_with(".md")
            || name_lower.ends_with(".markdown")
            || name_lower.ends_with(".txt")
            || name_lower.ends_with(".toml")
            || name_lower.ends_with(".rs")
            || name_lower.ends_with(".json")
    }

    /// Zero-copy borrow of file content as a UTF-8 string slice.
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes).ok()
    }
}

#[derive(Default)]
struct SharedDialogState {
    generation: u64,
    file: Option<Arc<PickedFile>>,
}

static SHARED_DIALOG_STATE: Mutex<SharedDialogState> = Mutex::new(SharedDialogState {
    generation: 0,
    file: None,
});

pub fn deliver_picked_file(file: PickedFile) {
    let mut state = SHARED_DIALOG_STATE.lock().unwrap();
    state.generation += 1;
    state.file = Some(Arc::new(file));
}

/// Explicitly clears the retained picked file from global static memory.
/// Call this once active views have ingested or copied the file content to prevent memory leaks.
pub fn clear_picked_file() {
    let mut state = SHARED_DIALOG_STATE.lock().unwrap();
    state.file = None;
}

pub fn deliver_picked_path(path: Option<PathBuf>) {
    if let Some(p) = path {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        let bytes = std::fs::read(&p).unwrap_or_default();
        deliver_picked_file(PickedFile::new(name, bytes));
    }
}

pub fn poll_picked_file_subscriber(last_seen_gen: &mut u64) -> Option<Arc<PickedFile>> {
    let state = SHARED_DIALOG_STATE.lock().unwrap();
    if state.generation > *last_seen_gen
        && let Some(ref file) = state.file
    {
        *last_seen_gen = state.generation;
        return Some(file.clone());
    }
    None
}

/// Request opening a native or browser file picker asynchronously.
pub fn pick_file_async() {
    #[cfg(all(
        feature = "media_viewer",
        not(target_arch = "wasm32"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    {
        std::thread::spawn(|| {
            let task = rfd::FileDialog::new()
                .add_filter(
                    "Images & Documents",
                    &[
                        "png", "jpg", "jpeg", "svg", "webp", "md", "markdown", "txt", "toml", "rs",
                        "json",
                    ],
                )
                .pick_file();

            if let Some(path) = task {
                deliver_picked_path(Some(path));
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        crate::platform::web::trigger_web_file_picker();
    }

    #[cfg(target_os = "android")]
    {
        crate::platform::android::request_file_picker("*/*");
    }

    #[cfg(target_os = "ios")]
    {
        // On iOS, asynchronous document picking will route through UIDocumentPickerViewController
    }
}

/// Legacy synchronous file picker for desktop targets.
pub fn pick_file() -> Option<PathBuf> {
    #[cfg(all(
        feature = "media_viewer",
        not(target_arch = "wasm32"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    {
        let p = rfd::FileDialog::new().pick_file();
        if let Some(ref path) = p {
            deliver_picked_path(Some(path.clone()));
        }
        p
    }

    #[cfg(all(
        not(feature = "media_viewer"),
        not(target_arch = "wasm32"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    {
        None
    }

    #[cfg(target_os = "ios")]
    {
        None
    }

    #[cfg(target_os = "android")]
    {
        pick_file_async();
        None
    }

    #[cfg(target_arch = "wasm32")]
    {
        pick_file_async();
        None
    }
}
