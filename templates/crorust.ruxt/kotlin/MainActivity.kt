package crorust.ruxt

import android.os.Bundle
import com.google.androidgamesdk.GameActivity
import crorust.ruxt.dialog.ActivityProvider
import crorust.ruxt.dialog.FileDialogProxy
import crorust.ruxt.dialog.SaveDialogProxy
import crorust.ruxt.permissions.PermissionProxy

class MainActivity : GameActivity() {

    // ── Lifecycle ─────────────────────────────────────────────────────────────

    override fun onCreate(savedInstanceState: Bundle?) {
        // Ensure intent extras is not null for vendor framework inspection
        if (intent.extras == null) {
            intent.putExtras(Bundle())
        }

        // Programmatically guarantee no window title bar or action bar across all Android versions
        requestWindowFeature(android.view.Window.FEATURE_NO_TITLE)
        supportRequestWindowFeature(android.view.Window.FEATURE_NO_TITLE)

        // Kotlin-side init MUST happen before super.onCreate(), which triggers
        // the native C glue → android_main(). Rust may call back via JNI immediately.
        ActivityProvider.setActivity(this)
        FileDialogProxy.registerLauncher(this)
        SaveDialogProxy.registerLauncher(this)
        materializeSampleFiles()
        super.onCreate(savedInstanceState)

        supportActionBar?.hide()

        // Safe after super.onCreate(): DecorView is now attached and initialized
        hideSystemUI()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        // Only re-hide system bars when window gains focus and IME is not actively requested
        if (hasFocus) {
            hideSystemUI()
        }
    }

    private fun hideSystemUI() {
        try {
            supportActionBar?.hide()
            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                androidx.core.view.WindowCompat.setDecorFitsSystemWindows(window, false)
                val controller = window.decorView.windowInsetsController ?: window.insetsController
                controller?.let {
                    // Hide status and navigation bars only — explicitly preserve IME
                    it.hide(
                        android.view.WindowInsets.Type.statusBars() or
                        android.view.WindowInsets.Type.navigationBars()
                    )
                    it.systemBarsBehavior =
                        android.view.WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                }
            } else {
                @Suppress("DEPRECATION")
                window.decorView.systemUiVisibility = (
                    android.view.View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                    or android.view.View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                    or android.view.View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                    or android.view.View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                    or android.view.View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                    or android.view.View.SYSTEM_UI_FLAG_FULLSCREEN
                )
            }
        } catch (e: Exception) {
            android.util.Log.w("MainActivity", "Failed to hide system UI", e)
        }
    }

    private fun materializeSampleFiles() {
        try {
            val targetDir = getExternalFilesDir(null) ?: filesDir
            val sampleFile = java.io.File(targetDir, "rendermd.md")
            if (!sampleFile.exists()) {
                val assetStream = try {
                    assets.open("rendermd.md")
                } catch (_: Exception) {
                    assets.open("assets/rendermd.md")
                }
                assetStream.use { input ->
                    sampleFile.outputStream().use { output ->
                        input.copyTo(output)
                    }
                }
                android.util.Log.i("MainActivity", "Materialized sample file to ${sampleFile.absolutePath}")
            }
        } catch (e: Exception) {
            android.util.Log.w("MainActivity", "Failed to materialize sample files", e)
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        // Ensure outState has non-empty bundle data so vendor hooks (MIUI AppScout) don't crash
        outState.putString("app", "crorust.ruxt")
    }

    // ── Callback forwarders ───────────────────────────────────────────────────

    private var lastStateText: String = ""

    override fun stateChanged(state: com.google.androidgamesdk.gametextinput.State?, reset: Boolean) {
        super.stateChanged(state, reset)
        if (state == null) return
        val currentText = state.text ?: ""
        if (reset) {
            lastStateText = currentText
            return
        }
        if (currentText != lastStateText) {
            if (currentText.startsWith(lastStateText)) {
                val appended = currentText.substring(lastStateText.length)
                if (appended.isNotEmpty()) {
                    crorust.ruxt.bridge.RustBridge.onTextInputCommit(appended)
                }
            } else if (lastStateText.startsWith(currentText)) {
                val deletedCount = lastStateText.length - currentText.length
                for (i in 0 until deletedCount) {
                    crorust.ruxt.bridge.RustBridge.onKeyAction(android.view.KeyEvent.KEYCODE_DEL, true)
                }
            } else {
                for (i in 0 until lastStateText.length) {
                    crorust.ruxt.bridge.RustBridge.onKeyAction(android.view.KeyEvent.KEYCODE_DEL, true)
                }
                if (currentText.isNotEmpty()) {
                    crorust.ruxt.bridge.RustBridge.onTextInputCommit(currentText)
                }
            }
            lastStateText = currentText
        }
    }

    override fun dispatchKeyEvent(event: android.view.KeyEvent): Boolean {
        // Intercept soft keyboard / hardware key events
        if (event.action == android.view.KeyEvent.ACTION_DOWN) {
            val unicodeChar = event.unicodeChar
            if (unicodeChar > 0 && unicodeChar != 10 && unicodeChar != 13) {
                val chars = String(Character.toChars(unicodeChar))
                crorust.ruxt.bridge.RustBridge.onTextInputCommit(chars)
            } else if (event.keyCode == android.view.KeyEvent.KEYCODE_DEL) {
                crorust.ruxt.bridge.RustBridge.onKeyAction(event.keyCode, true)
            } else if (event.keyCode == android.view.KeyEvent.KEYCODE_ENTER) {
                crorust.ruxt.bridge.RustBridge.onTextInputCommit("\n")
            }
        }
        return super.dispatchKeyEvent(event)
    }

    override fun onRequestPermissionsResult(
        requestCode: Int, permissions: Array<String>, grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        PermissionProxy.handleResult(requestCode, permissions, grantResults)
    }

    companion object {
        init {
            // Load the Rust .so — name matches Cargo [lib] name.
            // Companion init runs during class loading, before onCreate() is called,
            // so libruxt.so is available when super.onCreate() triggers the
            // native C glue that calls android_main().
            System.loadLibrary("ruxt")
        }
    }
}
