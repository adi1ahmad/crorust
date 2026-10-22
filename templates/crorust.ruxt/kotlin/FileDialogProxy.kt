package crorust.ruxt.dialog

import android.net.Uri
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import crorust.ruxt.bridge.RustBridge

/**
 * Opens the Android SAF file picker (multi-select) and routes the result to RustBridge.
 *
 * Uses the modern Activity Result API (registerForActivityResult) instead of
 * startActivityForResult/onActivityResult. The legacy path goes through
 * ActivityThread.deliverResultsIfNeeded which is patched on MIUI and throws
 * an NPE that silently swallows the result before onActivityResult is called.
 *
 * Uses ActivityResultContracts.OpenMultipleDocuments so the user can pick
 * several files at once. Each picked URI is materialized to a local cache
 * path via [toPath] before being passed to Rust.
 *
 * registerLauncher() must be called from MainActivity.onCreate() before super.onCreate().
 */
class SafeOpenMultipleDocuments : ActivityResultContracts.OpenMultipleDocuments() {
    override fun createIntent(context: android.content.Context, input: Array<String>): android.content.Intent {
        return super.createIntent(context, input).apply {
            // Attach an empty bundle extra so MIUI's ActivityThread.deliverResultsIfNeeded
            // inspection never encounters a null extras Bundle.
            putExtras(android.os.Bundle())
        }
    }
}

object FileDialogProxy {

    private var launcher: ActivityResultLauncher<Array<String>>? = null

    /** Call from MainActivity.onCreate() before super.onCreate(). */
    @JvmStatic
    fun registerLauncher(activity: ComponentActivity) {
        launcher = activity.registerForActivityResult(
            SafeOpenMultipleDocuments()
        ) { uris: List<Uri> ->
            android.util.Log.i("FileDialogProxy", "onFilesSelected uris count: ${uris.size}")
            val paths: Array<String> = uris.mapNotNull { toPath(it) }.toTypedArray()
            android.util.Log.i("FileDialogProxy", "materialized paths count: ${paths.size}")
            RustBridge.onFilesSelected(paths)
        }
    }

    @JvmStatic
    fun open(mimeType: String) {
        val l = launcher ?: run {
            android.util.Log.e("FileDialogProxy", "open() called before registerLauncher()")
            RustBridge.onFilesSelected(null)
            return
        }
        android.util.Log.i("FileDialogProxy", "open() launching picker with mime: $mimeType")
        l.launch(arrayOf(mimeType))
    }

    /**
     * Materialize a content URI to a local cache file. Returns the absolute path
     * of the cached copy, or null on failure.
     *
     * Each picked URI requires its own copy because Rust's std::fs::read works
     * with file paths, not content URIs. The cache dir is private to the app
     * and may be cleared by the system — files are best-effort transient
     * scratch space for the duration of the load.
     */
    private fun toPath(uri: Uri): String? {
        val ctx = ActivityProvider.getActivity()
        if (ctx == null) {
            android.util.Log.e("FileDialogProxy", "toPath: ActivityProvider.getActivity() returned null")
            return null
        }
        return try {
            val input = ctx.contentResolver.openInputStream(uri) ?: run {
                android.util.Log.e("FileDialogProxy", "toPath: openInputStream returned null")
                return null
            }
            val name = ctx.contentResolver.query(uri, null, null, null, null)?.use {
                if (it.moveToFirst()) {
                    val idx = it.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
                    if (idx >= 0) it.getString(idx) else null
                } else null
            } ?: "picked_file"
            val out = java.io.File(ctx.cacheDir, name)
            input.use { inStream ->
                out.outputStream().use { inStream.copyTo(it) }
            }
            out.absolutePath
        } catch (e: Exception) {
            android.util.Log.e("FileDialogProxy", "toPath: URI resolution failed", e)
            null
        }
    }
}
