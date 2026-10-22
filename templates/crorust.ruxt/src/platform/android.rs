use crate::RuxtApp;
use eframe::NativeOptions;
use jni::objects::{JClass, JObjectArray, JString};
use jni::{EnvUnowned, jni_sig, jni_str};
use std::sync::OnceLock;
use winit::platform::android::activity::AndroidApp;

static ANDROID_APP: OnceLock<AndroidApp> = OnceLock::new();

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let _ = ANDROID_APP.set(app.clone());

    let options = NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };

    eframe::run_native(
        "ruxt",
        options,
        Box::new(|cc| Ok(Box::new(RuxtApp::new(cc)))),
    )
    .unwrap();
}

#[cfg(target_os = "android")]
pub fn request_file_picker(mime_type: &str) {
    let Some(app) = ANDROID_APP.get() else {
        log::error!("request_file_picker: AndroidApp not initialized");
        return;
    };

    let vm_ptr = app.vm_as_ptr() as *mut jni::sys::JavaVM;
    if vm_ptr.is_null() {
        log::error!("request_file_picker: JavaVM pointer is null");
        return;
    }

    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr) };

    let res: Result<(), jni::errors::Error> = vm.attach_current_thread(|env| {
        let class = env.find_class(jni_str!("crorust/ruxt/bridge/RustBridge"))?;
        let j_mime = env.new_string(mime_type)?;
        env.call_static_method(
            &class,
            jni_str!("requestFilePicker"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[(&j_mime).into()],
        )?;
        Ok(())
    });

    if let Err(err) = res {
        log::error!("Failed to call RustBridge.requestFilePicker: {err:?}");
    }
}

#[cfg(target_os = "android")]
pub fn sync_ime_state(wants_ime: bool) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static CURRENT_IME_STATE: AtomicBool = AtomicBool::new(false);

    let prev = CURRENT_IME_STATE.swap(wants_ime, Ordering::Relaxed);
    if prev == wants_ime {
        return;
    }

    let Some(app) = ANDROID_APP.get() else {
        return;
    };

    if wants_ime {
        app.show_soft_input(false);
    } else {
        app.hide_soft_input(false);
    }

    let vm_ptr = app.vm_as_ptr() as *mut jni::sys::JavaVM;
    if vm_ptr.is_null() {
        return;
    }
    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr) };
    let _res: Result<(), jni::errors::Error> = vm.attach_current_thread(|env| {
        let class = env.find_class(jni_str!("crorust/ruxt/bridge/RustBridge"))?;
        let method_name = if wants_ime {
            jni_str!("showKeyboard")
        } else {
            jni_str!("hideKeyboard")
        };
        env.call_static_method(&class, method_name, jni_sig!("()V"), &[])?;
        Ok(())
    });
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_crorust_ruxt_bridge_RustBridge_onFilesSelected<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    paths: JObjectArray<'caller, JString<'caller>>,
) {
    let _ = unowned_env.with_env(|env| -> Result<(), jni::errors::Error> {
        if paths.is_null() {
            log::info!("RustBridge.onFilesSelected: user cancelled picker");
            super::dialog::deliver_picked_file(None);
            return Ok(());
        }

        let len = paths.len(env)?;
        if len > 0 {
            let jstr = paths.get_element(env, 0)?;
            let mutf8 = jstr.mutf8_chars(env)?;
            let path_str = mutf8.to_str();
            log::info!("RustBridge.onFilesSelected: received path: {}", path_str);
            super::dialog::deliver_picked_file(Some(std::path::PathBuf::from(path_str.as_ref())));
            return Ok(());
        }

        super::dialog::deliver_picked_file(None);
        Ok(())
    });
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_crorust_ruxt_bridge_RustBridge_onSaveUri<'caller>(
    _unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    _uri: JString<'caller>,
) {
    log::info!("RustBridge.onSaveUri received");
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_crorust_ruxt_bridge_RustBridge_onPermissionResult<'caller>(
    _unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    _permission: JString<'caller>,
    _granted: jni::sys::jboolean,
) {
    log::info!("RustBridge.onPermissionResult received");
}

static PENDING_IME_EVENTS: std::sync::Mutex<Vec<egui::Event>> = std::sync::Mutex::new(Vec::new());

pub fn drain_ime_events() -> Vec<egui::Event> {
    if let Ok(mut lock) = PENDING_IME_EVENTS.lock() {
        std::mem::take(&mut *lock)
    } else {
        Vec::new()
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_crorust_ruxt_bridge_RustBridge_onTextInputCommit<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    text: JString<'caller>,
) {
    let _ = unowned_env.with_env(|env| -> Result<(), jni::errors::Error> {
        if text.is_null() {
            return Ok(());
        }
        let chars = text.mutf8_chars(env)?;
        let text_str = chars.to_str();
        if !text_str.is_empty() {
            log::debug!("RustBridge.onTextInputCommit: {:?}", text_str);
            if let Ok(mut lock) = PENDING_IME_EVENTS.lock() {
                lock.push(egui::Event::Text(text_str.to_string()));
            }
        }
        Ok(())
    });
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_crorust_ruxt_bridge_RustBridge_onKeyAction<'caller>(
    _unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    key_code: jni::sys::jint,
    _is_down: jni::sys::jboolean,
) {

    if key_code == 67 {
        if let Ok(mut lock) = PENDING_IME_EVENTS.lock() {
            lock.push(egui::Event::Key {
                key: egui::Key::Backspace,
                physical_key: Some(egui::Key::Backspace),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
            lock.push(egui::Event::Key {
                key: egui::Key::Backspace,
                physical_key: Some(egui::Key::Backspace),
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }
}
