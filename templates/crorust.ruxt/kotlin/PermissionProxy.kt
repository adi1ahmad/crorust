package crorust.ruxt.permissions

import android.content.pm.PackageManager
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import crorust.ruxt.bridge.RustBridge
import crorust.ruxt.dialog.ActivityProvider

private const val RC_PERMISSION = 101

/**
 * Handles Android 6+ runtime permission requests on behalf of Rust.
 * Call from UI thread only.
 */
object PermissionProxy {
    private var pendingPermission: String? = null

    @JvmStatic
    fun request(permission: String) {
        val activity = ActivityProvider.getActivity() ?: run {
            RustBridge.onPermissionResult(permission, false)
            return
        }
        if (ContextCompat.checkSelfPermission(activity, permission) == PackageManager.PERMISSION_GRANTED) {
            RustBridge.onPermissionResult(permission, true)
            return
        }
        pendingPermission = permission
        ActivityCompat.requestPermissions(activity, arrayOf(permission), RC_PERMISSION)
    }

    @JvmStatic
    fun handleResult(requestCode: Int, permissions: Array<String>, grantResults: IntArray) {
        if (requestCode != RC_PERMISSION) return
        val permission = pendingPermission ?: permissions.firstOrNull() ?: return
        val granted = grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED
        pendingPermission = null
        RustBridge.onPermissionResult(permission, granted)
    }
}
