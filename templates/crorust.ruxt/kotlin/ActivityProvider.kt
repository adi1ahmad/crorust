package crorust.ruxt.dialog

import android.app.Activity
import java.lang.ref.WeakReference

/**
 * Holds a weak reference to the current Activity.
 * Set by MainActivity.onCreate before super.onCreate() triggers android_main.
 * WeakReference prevents memory leaks if Activity is destroyed.
 */
object ActivityProvider {
    private var ref: WeakReference<Activity>? = null

    @JvmStatic fun setActivity(a: Activity) { ref = WeakReference(a) }
    @JvmStatic fun getActivity(): Activity? = ref?.get()
}
