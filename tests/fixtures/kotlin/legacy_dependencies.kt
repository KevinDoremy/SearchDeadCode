// Test fixture: Legacy Dependencies Anti-Pattern
// Anti-pattern #5 from "8 anti-patterns in Android codebase"
package com.example.fixtures.antipatterns

// Note: This file demonstrates code that uses legacy/deprecated libraries
// The actual detection would be done by checking build.gradle dependencies

// LEGACY: ButterKnife (deprecated, use ViewBinding)
// import butterknife.BindView
// import butterknife.ButterKnife

class ButterKnifeActivity {
    // @BindView(R.id.textView)
    // lateinit var textView: TextView

    // LEGACY PATTERN: ButterKnife binding
    fun onCreate() {
        // ButterKnife.bind(this)  // DEPRECATED
    }
}

// LEGACY: Kotlin Android Extensions / Synthetic (deprecated)
// import kotlinx.android.synthetic.main.activity_main.*

class SyntheticActivity {
    // LEGACY PATTERN: synthetic imports
    fun setupViews() {
        // textView.text = "Hello"  // DEPRECATED: use ViewBinding instead
        // button.setOnClickListener { }  // DEPRECATED
    }
}

// LEGACY: RxJava 1.x (use RxJava 3 or Coroutines)
// import rx.Observable
// import rx.Subscriber

class RxJava1Usage {
    // LEGACY PATTERN: RxJava 1.x
    // fun fetchData(): Observable<String> {
    //     return Observable.just("data")
    // }
}

// LEGACY: Deprecated support libraries (use AndroidX)
// import android.support.v4.app.Fragment
// import android.support.v7.app.AppCompatActivity
// import android.support.v7.widget.RecyclerView

class SupportLibraryActivity {
    // LEGACY: using android.support.* instead of androidx.*
}

// LEGACY: AsyncTask (deprecated in API 30)
class AsyncTaskUsage {
    // LEGACY PATTERN: AsyncTask
    // inner class LoadDataTask : AsyncTask<Void, Void, String>() {
    //     override fun doInBackground(vararg params: Void?): String {
    //         return "data"
    //     }
    // }
}

// LEGACY: Loader/LoaderManager (deprecated)
class LoaderUsage {
    // LEGACY PATTERN: Loaders
    // fun onCreateLoader(id: Int, args: Bundle?): Loader<Cursor> {
    //     return CursorLoader(context, uri, null, null, null, null)
    // }
}

// LEGACY: IntentService (deprecated in API 30)
class LegacyIntentService {
    // LEGACY: Use WorkManager instead
    // class MyIntentService : IntentService("MyIntentService") {
    //     override fun onHandleIntent(intent: Intent?) {
    //         // Background work
    //     }
    // }
}

// LEGACY: LocalBroadcastManager (deprecated)
class LocalBroadcastUsage {
    // LEGACY PATTERN: LocalBroadcastManager
    // fun sendBroadcast() {
    //     LocalBroadcastManager.getInstance(context).sendBroadcast(intent)
    // }
}

// MODERN: ViewBinding (correct approach)
class ViewBindingActivity {
    // private lateinit var binding: ActivityMainBinding
    //
    // fun onCreate() {
    //     binding = ActivityMainBinding.inflate(layoutInflater)
    //     setContentView(binding.root)
    //     binding.textView.text = "Hello"
    // }
}

// MODERN: Coroutines (correct approach)
class CoroutinesUsage {
    suspend fun fetchData(): String {
        // Use coroutines instead of RxJava or AsyncTask
        return "data"
    }
}

// MODERN: WorkManager (correct approach)
class WorkManagerUsage {
    // class MyWorker(context: Context, params: WorkerParameters)
    //     : CoroutineWorker(context, params) {
    //
    //     override suspend fun doWork(): Result {
    //         // Background work
    //         return Result.success()
    //     }
    // }
}

// List of legacy dependencies to check in build.gradle:
// - com.jakewharton:butterknife (deprecated)
// - org.jetbrains.kotlin:kotlin-android-extensions (deprecated)
// - io.reactivex:rxjava:1.x (outdated)
// - com.android.support:* (use androidx.*)
// - com.google.android:support-v4 (deprecated)
// - com.squareup.retrofit:retrofit:1.x (outdated)
// - com.squareup.okhttp:okhttp:2.x (outdated)
// - org.greenrobot:eventbus (anti-pattern)

fun main() {
    println("Legacy dependencies should be migrated!")
}
