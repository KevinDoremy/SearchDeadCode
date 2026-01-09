// Test fixture for WakeLockAbuseDetector (AP028)
// Detects WakeLock not properly released

package com.example.antipattern

import android.content.Context
import android.os.PowerManager

// BAD: WakeLock abuse patterns
class WakeLockAbuse {

    private var wakeLock: PowerManager.WakeLock? = null

    // BAD: WakeLock acquired but never released
    fun startLongOperation(context: Context) {
        val powerManager = context.getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = powerManager.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            "MyApp::LongOperation"
        )
        wakeLock?.acquire()  // Never released!

        doLongOperation()
    }

    // BAD: WakeLock without timeout
    fun acquireIndefinitely(context: Context) {
        val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
        val wl = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "MyApp::Forever")
        wl.acquire()  // No timeout - could drain battery completely!
    }

    // BAD: Release not in finally block
    fun riskyOperation(context: Context) {
        val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
        val wl = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "MyApp::Risky")
        wl.acquire()

        doRiskyOperation()  // Could throw exception!

        wl.release()  // Never reached if exception
    }

    // BAD: Acquiring in onCreate without release in onDestroy
    fun onCreate(context: Context) {
        val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "MyApp::Lifecycle")
        wakeLock?.acquire()
    }

    // Missing onDestroy with wakeLock?.release()
}

// GOOD: Proper WakeLock handling
class ProperWakeLockUsage {

    private var wakeLock: PowerManager.WakeLock? = null

    // GOOD: WakeLock with timeout
    fun startTimedOperation(context: Context) {
        val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
        val wl = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "MyApp::Timed")
        wl.acquire(10 * 60 * 1000L)  // 10 minutes max

        try {
            doLongOperation()
        } finally {
            if (wl.isHeld) {
                wl.release()
            }
        }
    }

    // GOOD: Release in finally block
    fun safeOperation(context: Context) {
        val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
        val wl = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "MyApp::Safe")
        wl.acquire(5 * 60 * 1000L)

        try {
            doRiskyOperation()
        } finally {
            wl.release()
        }
    }

    // GOOD: Lifecycle-aware WakeLock
    fun onCreate(context: Context) {
        val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "MyApp::Lifecycle")
    }

    fun onResume() {
        wakeLock?.acquire(30 * 60 * 1000L)  // With timeout
    }

    fun onPause() {
        if (wakeLock?.isHeld == true) {
            wakeLock?.release()
        }
    }

    fun onDestroy() {
        wakeLock = null
    }
}

// GOOD: Using WorkManager instead
class ModernApproach {

    // GOOD: WorkManager handles WakeLock internally
    fun scheduleWork(context: Context) {
        val request = OneTimeWorkRequestBuilder<MyWorker>()
            .build()
        WorkManager.getInstance(context).enqueue(request)
    }
}

// OK: WakeLock in foreground service (proper pattern)
class ForegroundServiceWithWakeLock {

    private var wakeLock: PowerManager.WakeLock? = null

    fun onStartCommand(context: Context) {
        val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "MyApp::ForegroundService")
        wakeLock?.acquire()  // OK in foreground service

        startForeground()
    }

    fun onDestroy() {
        wakeLock?.release()  // Always released in onDestroy
        wakeLock = null
    }
}

// Supporting functions/classes
fun doLongOperation() {}
fun doRiskyOperation() {}
fun startForeground() {}

class OneTimeWorkRequestBuilder<T> {
    fun build(): WorkRequest = WorkRequest()
}
class WorkRequest
class WorkManager {
    companion object {
        fun getInstance(context: Context): WorkManager = WorkManager()
    }
    fun enqueue(request: WorkRequest) {}
}
class MyWorker
