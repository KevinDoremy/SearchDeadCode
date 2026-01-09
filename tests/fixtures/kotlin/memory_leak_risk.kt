package com.example.antipattern

import android.app.Activity
import android.content.Context
import android.view.View
import android.os.Handler

// BAD: Static reference to Context (memory leak)
object ContextHolder {
    lateinit var context: Context  // LEAK! Context outlives Activity
    lateinit var activity: Activity  // LEAK! Static Activity reference
}

// BAD: Companion object holding Context
class LeakyClass {
    companion object {
        var appContext: Context? = null  // LEAK if Activity context stored
        var cachedView: View? = null  // LEAK! View holds Activity reference
    }
}

// BAD: Anonymous inner class holding Activity reference
class LeakyActivity : Activity() {

    fun startLeakyOperation() {
        // BAD: Anonymous Runnable holds implicit reference to Activity
        Handler().postDelayed(object : Runnable {
            override fun run() {
                // 'this@LeakyActivity' is captured - memory leak!
                updateUI()
            }
        }, 10000)
    }

    // BAD: Non-static inner class holds reference to outer Activity
    inner class LeakyInnerClass {
        fun doSomething() {
            // Holds implicit reference to LeakyActivity
        }
    }

    private fun updateUI() { }
}

// BAD: Handler without WeakReference
class HandlerLeakActivity : Activity() {
    // BAD: Handler as instance variable without weak reference
    private val handler = Handler()

    fun postDelayed() {
        handler.postDelayed({
            // This lambda captures 'this'
            doWork()
        }, 5000)
    }

    private fun doWork() { }
}

// GOOD: Using WeakReference
class SafeActivity : Activity() {

    // GOOD: Static inner class doesn't hold reference
    class SafeRunnable(activity: Activity) : Runnable {
        private val activityRef = java.lang.ref.WeakReference(activity)

        override fun run() {
            activityRef.get()?.let { /* safe */ }
        }
    }
}

// GOOD: Using application context
class SafeContextUsage(private val appContext: Context) {
    // Application context is safe - it lives as long as the app
    fun useContext() {
        appContext.getString(R.string.app_name)
    }
}

// BAD: Singleton holding Activity
class LeakySingleton private constructor() {
    var activity: Activity? = null  // LEAK!

    companion object {
        val instance = LeakySingleton()
    }
}
