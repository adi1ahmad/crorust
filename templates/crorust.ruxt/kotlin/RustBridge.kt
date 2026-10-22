package crorust.ruxt.bridge

import crorust.ruxt.dialog.ActivityProvider
import crorust.ruxt.dialog.FileDialogProxy
import crorust.ruxt.dialog.SaveDialogProxy
import crorust.ruxt.permissions.PermissionProxy

/**
 * JNI boundary between Rust and Kotlin.
 *
 * Direction split:
 * - Kotlin → Rust: external fun declarations (implemented with #[no_mangle] in Rust)
 * - Rust → Kotlin: @JvmStatic methods (called via env.call_static_method from Rust)
 *
 * Threading: methods may be called from any thread.
 * Methods that touch Android UI MUST post to the main thread.
 */
object RustBridge {

    // ── Kotlin → Rust callbacks ───────────────────────────────────────────────

    /**
     * Called when SAF multi-file picker result is available.
     * `paths` is null if the user cancelled, or an empty array if the user
     * dismissed the picker without selecting anything. Each non-null entry
     * is an absolute path to a local cache copy of the picked file's content
     * (created by FileDialogProxy.toPath).
     */
    external fun onFilesSelected(paths: Array<String>?)

    /** Called when SAF create-document picker result is available. null = cancelled. */
    external fun onSaveUri(uri: String?)

    /** Called when a runtime permission request completes. */
    external fun onPermissionResult(permission: String, granted: Boolean)

    /** Called when text input characters are committed from the soft keyboard / IME. */
    external fun onTextInputCommit(text: String)

    /** Called when a key action (such as Backspace or Enter) occurs. */
    external fun onKeyAction(keyCode: Int, isDown: Boolean)

    // ── Rust → Kotlin triggers ────────────────────────────────────────────────

    /** Triggers the SAF file picker. Result delivered via onFileSelected. */
    @JvmStatic
    fun requestFilePicker(mimeType: String) {
        ActivityProvider.getActivity()?.runOnUiThread {
            FileDialogProxy.open(mimeType)
        }
    }

    /** Triggers the SAF create-document picker. Result delivered via onSaveUri. */
    @JvmStatic
    fun requestSavePicker(suggestedName: String) {
        ActivityProvider.getActivity()?.runOnUiThread {
            SaveDialogProxy.open(suggestedName)
        }
    }

    /** Flushes bytes to the given content URI's OutputStream via ContentResolver. */
    @JvmStatic
    fun writeBytesToUri(uri: String, bytes: ByteArray) {
        val ctx = ActivityProvider.getActivity() ?: run {
            android.util.Log.e("RustBridge", "writeBytesToUri: no activity")
            return
        }
        try {
            val out = ctx.contentResolver.openOutputStream(android.net.Uri.parse(uri))
                ?: run {
                    android.util.Log.e("RustBridge", "writeBytesToUri: openOutputStream returned null")
                    return
                }
            out.use { it.write(bytes) }
        } catch (e: Exception) {
            android.util.Log.e("RustBridge", "writeBytesToUri failed", e)
        }
    }

    /** Requests a runtime permission. Result delivered via onPermissionResult. */
    @JvmStatic
    fun requestPermission(permission: String) {
        ActivityProvider.getActivity()?.runOnUiThread {
            PermissionProxy.request(permission)
        }
    }

    /** Shows the soft keyboard / IME. */
    @JvmStatic
    fun showKeyboard() {
        val act = ActivityProvider.getActivity() ?: return
        act.runOnUiThread {
            try {
                if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                    val controller = act.window.decorView.windowInsetsController
                        ?: act.window.insetsController
                    controller?.show(android.view.WindowInsets.Type.ime())
                } else {
                    val imm = act.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                        as? android.view.inputmethod.InputMethodManager
                    imm?.showSoftInput(act.window.decorView, android.view.inputmethod.InputMethodManager.SHOW_IMPLICIT)
                }
            } catch (e: Exception) {
                android.util.Log.w("RustBridge", "showKeyboard failed", e)
            }
        }
    }

    /** Hides the soft keyboard / IME. */
    @JvmStatic
    fun hideKeyboard() {
        val act = ActivityProvider.getActivity() ?: return
        act.runOnUiThread {
            try {
                if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                    val controller = act.window.decorView.windowInsetsController
                        ?: act.window.insetsController
                    controller?.hide(android.view.WindowInsets.Type.ime())
                } else {
                    val imm = act.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                        as? android.view.inputmethod.InputMethodManager
                    val view = act.currentFocus ?: act.window.decorView
                    imm?.hideSoftInputFromWindow(view.windowToken, 0)
                }
            } catch (e: Exception) {
                android.util.Log.w("RustBridge", "hideKeyboard failed", e)
            }
        }
    }
}
