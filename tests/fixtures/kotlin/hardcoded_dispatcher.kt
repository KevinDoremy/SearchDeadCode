// Test fixture for HardcodedDispatcherDetector (AP020)
// Detects hardcoded Dispatchers.IO/Main/Default

package com.example.antipattern

import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*

// BAD: Hardcoded dispatchers make testing difficult
class BadRepository {

    // BAD: Hardcoded Dispatchers.IO
    suspend fun fetchData(): Data = withContext(Dispatchers.IO) {
        // Network/DB call
        networkClient.fetch()
    }

    // BAD: Hardcoded Dispatchers.Default
    suspend fun processData(data: Data): Result = withContext(Dispatchers.Default) {
        // CPU-intensive work
        heavyComputation(data)
    }

    // BAD: Hardcoded Dispatchers.Main
    suspend fun updateUI(result: Result) = withContext(Dispatchers.Main) {
        // UI update
        view.showResult(result)
    }
}

// BAD: Hardcoded in Flow
class BadFlowRepository {

    // BAD: flowOn with hardcoded dispatcher
    fun observeData(): Flow<Data> = flow {
        emit(fetchFromNetwork())
    }.flowOn(Dispatchers.IO)

    // BAD: Multiple hardcoded dispatchers
    fun processStream(): Flow<Result> = flow {
        emit(fetchFromNetwork())
    }
        .map { heavyComputation(it) }
        .flowOn(Dispatchers.Default)
        .onEach { updateCache(it) }
        .flowOn(Dispatchers.IO)
}

// BAD: Hardcoded in launch/async
class BadViewModel : ViewModel() {

    fun loadData() {
        // BAD: Hardcoded dispatcher in launch
        viewModelScope.launch(Dispatchers.IO) {
            val data = repository.fetchData()
            withContext(Dispatchers.Main) {
                updateUI(data)
            }
        }
    }

    fun processAsync() {
        viewModelScope.launch {
            // BAD: Hardcoded dispatcher in async
            val result = async(Dispatchers.Default) {
                heavyComputation()
            }.await()
        }
    }
}

// GOOD: Inject dispatchers for testability
class GoodRepository(
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
    private val defaultDispatcher: CoroutineDispatcher = Dispatchers.Default
) {

    suspend fun fetchData(): Data = withContext(ioDispatcher) {
        networkClient.fetch()
    }

    suspend fun processData(data: Data): Result = withContext(defaultDispatcher) {
        heavyComputation(data)
    }
}

// GOOD: Using DispatcherProvider pattern
interface DispatcherProvider {
    val main: CoroutineDispatcher
    val io: CoroutineDispatcher
    val default: CoroutineDispatcher
}

class DefaultDispatcherProvider : DispatcherProvider {
    override val main = Dispatchers.Main
    override val io = Dispatchers.IO
    override val default = Dispatchers.Default
}

class TestDispatcherProvider(
    testDispatcher: TestCoroutineDispatcher
) : DispatcherProvider {
    override val main = testDispatcher
    override val io = testDispatcher
    override val default = testDispatcher
}

class GoodViewModelWithProvider(
    private val dispatchers: DispatcherProvider
) : ViewModel() {

    fun loadData() {
        viewModelScope.launch(dispatchers.io) {
            val data = repository.fetchData()
            withContext(dispatchers.main) {
                updateUI(data)
            }
        }
    }
}

// GOOD: Flow with injected dispatcher
class GoodFlowRepository(
    private val ioDispatcher: CoroutineDispatcher
) {

    fun observeData(): Flow<Data> = flow {
        emit(fetchFromNetwork())
    }.flowOn(ioDispatcher)
}

// OK: Dispatchers in test files are expected
class RepositoryTest {

    private val testDispatcher = TestCoroutineDispatcher()

    @Test
    fun testFetch() = runBlockingTest {
        // OK: Test code can use Dispatchers directly
        val result = withContext(Dispatchers.Unconfined) {
            repository.fetchData()
        }
    }
}

// Supporting types
interface NetworkClient {
    suspend fun fetch(): Data
}

data class Data(val content: String)
data class Result(val value: Int)

interface View {
    fun showResult(result: Result)
}

class ViewModel {
    val viewModelScope = CoroutineScope(SupervisorJob())
}

class TestCoroutineDispatcher : CoroutineDispatcher() {
    override fun dispatch(context: CoroutineContext, block: Runnable) {
        block.run()
    }
}
