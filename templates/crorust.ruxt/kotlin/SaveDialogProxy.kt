package crorust.ruxt.dialog

import android.net.Uri
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import crorust.ruxt.bridge.RustBridge

/**
 * Opens the Android SAF create-document picker (for SAVE flows) and routes
 * the result to RustBridge.
 *
 * Uses ActivityResultContracts.CreateDocument() with a suggested filename.
 * The chosen content URI is passed to Rust, which encodes the data and
 * calls back into RustBridge.writeBytesToUri to flush it.
 *
 * registerLauncher() must be called from MainActivity.onCreate() before
 * super.onCreate(), alongside FileDialogProxy.registerLauncher.
 */
class SafeCreateDocument(mimeType: String) : ActivityResultContracts.CreateDocument(mimeType) {
    override fun createIntent(context: android.content.Context, input: String): android.content.Intent {
        return super.createIntent(context, input).apply {
            // Attach an empty bundle extra so MIUI's ActivityThread.deliverResultsIfNeeded
            // inspection never encounters a null extras Bundle.
            putExtras(android.os.Bundle())
        }
    }
}

object SaveDialogProxy {

    private var launcher: ActivityResultLauncher<String>? = null

    /** Call from MainActivity.onCreate() before super.onCreate(). */
    @JvmStatic
    fun registerLauncher(activity: ComponentActivity) {
        launcher = activity.registerForActivityResult(
            SafeCreateDocument("application/octet-stream")
        ) { uri: Uri? ->
            android.util.Log.i("SaveDialogProxy", "onSaveUri: $uri")
            RustBridge.onSaveUri(uri?.toString())
        }
    }

    @JvmStatic
    fun open(suggestedName: String) {
        val l = launcher ?: run {
            android.util.Log.e("SaveDialogProxy", "open() called before registerLauncher()")
            RustBridge.onSaveUri(null)
            return
        }
        android.util.Log.i("SaveDialogProxy", "open() launching save picker with suggestedName: $suggestedName")
        l.launch(suggestedName)
    }
}
