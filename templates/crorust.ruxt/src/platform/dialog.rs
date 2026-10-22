use std::path::PathBuf;
use std::sync::Mutex;

static PICKED_FILE_RECEIVER: Mutex<Option<std::sync::mpsc::Receiver<PathBuf>>> = Mutex::new(None);
static PICKED_FILE_SENDER: Mutex<Option<std::sync::mpsc::Sender<PathBuf>>> = Mutex::new(None);

fn ensure_channel() {
    let mut rx_lock = PICKED_FILE_RECEIVER.lock().unwrap();
    if rx_lock.is_none() {
        let (tx, rx) = std::sync::mpsc::channel();
        *PICKED_FILE_SENDER.lock().unwrap() = Some(tx);
        *rx_lock = Some(rx);
    }
}

pub fn pick_file() -> Option<PathBuf> {
    #[cfg(all(
        feature = "media_viewer",
        not(target_arch = "wasm32"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    {
        rfd::FileDialog::new().pick_file()
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
        ensure_channel();
        crate::platform::android::request_file_picker("*/*");
        None
    }

    #[cfg(all(not(target_os = "android"), not(target_os = "ios"), target_arch = "wasm32"))]
    {
        None
    }
}

pub fn poll_picked_file() -> Option<PathBuf> {
    ensure_channel();
    let lock = PICKED_FILE_RECEIVER.lock().unwrap();
    if let Some(rx) = lock.as_ref()
        && let Ok(path) = rx.try_recv()
    {
        return Some(path);
    }
    None
}

#[cfg(target_os = "android")]
pub(crate) fn deliver_picked_file(path: Option<PathBuf>) {
    ensure_channel();
    if let Some(p) = path {
        let lock = PICKED_FILE_SENDER.lock().unwrap();
        if let Some(tx) = lock.as_ref() {
            let _ = tx.send(p);
        }
    }
}
