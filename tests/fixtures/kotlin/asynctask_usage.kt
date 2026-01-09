// Test fixture for AsyncTaskUsageDetector (AP029)
// Detects deprecated AsyncTask usage

package com.example.antipattern

import android.os.AsyncTask

// BAD: Using deprecated AsyncTask
class DeprecatedAsyncTaskUsage {

    // BAD: Extending AsyncTask
    inner class LoadDataTask : AsyncTask<String, Int, List<User>>() {

        override fun onPreExecute() {
            showLoading()
        }

        override fun doInBackground(vararg params: String): List<User> {
            val url = params[0]
            return fetchUsers(url)
        }

        override fun onProgressUpdate(vararg values: Int) {
            updateProgress(values[0])
        }

        override fun onPostExecute(result: List<User>) {
            hideLoading()
            displayUsers(result)
        }
    }

    // BAD: Executing AsyncTask
    fun loadData() {
        LoadDataTask().execute("https://api.example.com/users")
    }

    // BAD: AsyncTask with executeOnExecutor
    fun loadDataParallel() {
        LoadDataTask().executeOnExecutor(AsyncTask.THREAD_POOL_EXECUTOR, "url")
    }

    // BAD: Anonymous AsyncTask
    fun quickTask() {
        object : AsyncTask<Void, Void, String>() {
            override fun doInBackground(vararg params: Void?): String {
                return "result"
            }

            override fun onPostExecute(result: String) {
                handleResult(result)
            }
        }.execute()
    }
}

// GOOD: Using Kotlin Coroutines
class CoroutinesApproach {

    private val scope = CoroutineScope(Dispatchers.Main + Job())

    fun loadData() {
        scope.launch {
            showLoading()
            try {
                val users = withContext(Dispatchers.IO) {
                    fetchUsers("https://api.example.com/users")
                }
                displayUsers(users)
            } finally {
                hideLoading()
            }
        }
    }

    fun loadDataWithProgress() {
        scope.launch {
            showLoading()
            val users = withContext(Dispatchers.IO) {
                fetchUsersWithProgress { progress ->
                    withContext(Dispatchers.Main) {
                        updateProgress(progress)
                    }
                }
            }
            displayUsers(users)
            hideLoading()
        }
    }

    fun onDestroy() {
        scope.cancel()  // Proper cancellation!
    }
}

// GOOD: Using ViewModel with coroutines
class ModernViewModel : ViewModel() {

    private val _users = MutableLiveData<List<User>>()
    val users: LiveData<List<User>> = _users

    private val _loading = MutableLiveData<Boolean>()
    val loading: LiveData<Boolean> = _loading

    fun loadData() {
        viewModelScope.launch {
            _loading.value = true
            try {
                val result = withContext(Dispatchers.IO) {
                    fetchUsers("https://api.example.com/users")
                }
                _users.value = result
            } finally {
                _loading.value = false
            }
        }
    }
}

// GOOD: Using RxJava
class RxJavaApproach {

    private val disposables = CompositeDisposable()

    fun loadData() {
        disposables.add(
            Single.fromCallable { fetchUsers("url") }
                .subscribeOn(Schedulers.io())
                .observeOn(AndroidSchedulers.mainThread())
                .doOnSubscribe { showLoading() }
                .doFinally { hideLoading() }
                .subscribe(
                    { users -> displayUsers(users) },
                    { error -> showError(error) }
                )
        )
    }

    fun onDestroy() {
        disposables.clear()
    }
}

// GOOD: Using Executor with Handler
class ExecutorApproach {

    private val executor = Executors.newSingleThreadExecutor()
    private val handler = Handler(Looper.getMainLooper())

    fun loadData() {
        handler.post { showLoading() }

        executor.execute {
            val users = fetchUsers("url")
            handler.post {
                hideLoading()
                displayUsers(users)
            }
        }
    }

    fun onDestroy() {
        executor.shutdown()
    }
}

// Supporting types and functions
data class User(val name: String)
fun fetchUsers(url: String): List<User> = emptyList()
fun fetchUsersWithProgress(onProgress: (Int) -> Unit): List<User> = emptyList()
fun showLoading() {}
fun hideLoading() {}
fun updateProgress(progress: Int) {}
fun displayUsers(users: List<User>) {}
fun handleResult(result: String) {}
fun showError(error: Throwable) {}

// Mock types
class CoroutineScope(context: CoroutineContext)
interface CoroutineContext
class Job : CoroutineContext
object Dispatchers {
    val Main: CoroutineContext = Job()
    val IO: CoroutineContext = Job()
}
fun CoroutineScope.launch(block: suspend () -> Unit) {}
fun CoroutineScope.cancel() {}
suspend fun <T> withContext(context: CoroutineContext, block: suspend () -> T): T = TODO()

open class ViewModel {
    val viewModelScope: CoroutineScope = CoroutineScope(Job())
}
class MutableLiveData<T> {
    var value: T? = null
}
class LiveData<T>

class CompositeDisposable {
    fun add(disposable: Any) {}
    fun clear() {}
}
class Single<T> {
    companion object {
        fun <T> fromCallable(block: () -> T): Single<T> = Single()
    }
    fun subscribeOn(scheduler: Any): Single<T> = this
    fun observeOn(scheduler: Any): Single<T> = this
    fun doOnSubscribe(action: () -> Unit): Single<T> = this
    fun doFinally(action: () -> Unit): Single<T> = this
    fun subscribe(onSuccess: (T) -> Unit, onError: (Throwable) -> Unit): Any = Any()
}
object Schedulers {
    fun io(): Any = Any()
}
object AndroidSchedulers {
    fun mainThread(): Any = Any()
}

class Handler(looper: Looper) {
    fun post(action: () -> Unit) {}
}
class Looper {
    companion object {
        fun getMainLooper(): Looper = Looper()
    }
}
object Executors {
    fun newSingleThreadExecutor(): ExecutorService = object : ExecutorService {
        override fun execute(command: Runnable) {}
        override fun shutdown() {}
    }
}
interface ExecutorService {
    fun execute(command: Runnable)
    fun shutdown()
}
