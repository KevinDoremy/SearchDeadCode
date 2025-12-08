package com.example.antipattern

import kotlinx.coroutines.*
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope

// BAD: GlobalScope usage - coroutine outlives component lifecycle
class GlobalScopeAbuse {

    // BAD: Using GlobalScope.launch
    fun badLaunch() {
        GlobalScope.launch {
            // This coroutine will outlive the component!
            loadData()
        }
    }

    // BAD: Using GlobalScope.async
    fun badAsync(): Deferred<Data> {
        return GlobalScope.async {
            // Memory leak risk!
            fetchData()
        }
    }

    // BAD: GlobalScope in a service
    fun startBackgroundWork() {
        GlobalScope.launch(Dispatchers.IO) {
            // Will keep running even if service is destroyed
            while (true) {
                doWork()
                delay(1000)
            }
        }
    }
}

// GOOD: ViewModel with proper scope
class ProperViewModel : ViewModel() {

    // GOOD: Using viewModelScope - cancelled when ViewModel cleared
    fun loadData() {
        viewModelScope.launch {
            fetchData()
        }
    }

    // GOOD: Using viewModelScope.async
    fun loadDataAsync(): Deferred<Data> {
        return viewModelScope.async {
            fetchData()
        }
    }
}

// GOOD: Activity/Fragment with lifecycleScope
class ProperActivity {

    // GOOD: Using lifecycleScope
    fun onResume() {
        lifecycleScope.launch {
            refreshData()
        }
    }

    // GOOD: Using lifecycleScope.launchWhenStarted
    fun bindUi() {
        lifecycleScope.launchWhenStarted {
            observeUiState()
        }
    }
}

// BAD: Using runBlocking in inappropriate places
class RunBlockingAbuse {

    // BAD: runBlocking on main thread
    fun badRunBlocking() {
        runBlocking {
            // Blocks the calling thread!
            delay(1000)
        }
    }

    // OK: runBlocking in main function or tests
    // fun main() = runBlocking { ... }  // This is acceptable
}
