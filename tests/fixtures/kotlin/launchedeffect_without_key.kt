// Test fixture for LaunchedEffectWithoutKeyDetector (AP032)
// Detects LaunchedEffect/DisposableEffect without proper keys

package com.example.compose

import androidx.compose.runtime.*
import androidx.compose.material.*

// BAD: LaunchedEffect with Unit key - runs only once, ignores parameter changes
@Composable
fun BadLaunchedEffect(userId: String) {
    var user by remember { mutableStateOf<User?>(null) }

    LaunchedEffect(Unit) {  // BAD: Should use userId as key
        user = fetchUser(userId)
    }

    user?.let { Text(it.name) }
}

// BAD: LaunchedEffect with true/false constant
@Composable
fun BadConstantKey(query: String) {
    var results by remember { mutableStateOf<List<Result>>(emptyList()) }

    LaunchedEffect(true) {  // BAD: Constant key
        results = search(query)
    }

    ResultsList(results)
}

// BAD: DisposableEffect without proper key
@Composable
fun BadDisposableEffect(lifecycleOwner: LifecycleOwner) {
    DisposableEffect(Unit) {  // BAD: Should use lifecycleOwner
        val observer = LifecycleEventObserver { _, event ->
            // Handle lifecycle
        }
        lifecycleOwner.lifecycle.addObserver(observer)

        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
        }
    }
}

// BAD: SideEffect that should be LaunchedEffect
@Composable
fun BadSideEffect(analytics: Analytics, screenName: String) {
    SideEffect {
        analytics.logScreen(screenName)  // BAD: Runs on every recomposition
    }
}

// GOOD: LaunchedEffect with proper key
@Composable
fun GoodLaunchedEffect(userId: String) {
    var user by remember { mutableStateOf<User?>(null) }

    LaunchedEffect(userId) {  // GOOD: Re-runs when userId changes
        user = fetchUser(userId)
    }

    user?.let { Text(it.name) }
}

// GOOD: LaunchedEffect with multiple keys
@Composable
fun GoodMultipleKeys(userId: String, refresh: Boolean) {
    var data by remember { mutableStateOf<Data?>(null) }

    LaunchedEffect(userId, refresh) {  // GOOD: Multiple keys
        data = fetchData(userId)
    }

    data?.let { DisplayData(it) }
}

// GOOD: DisposableEffect with proper key
@Composable
fun GoodDisposableEffect(lifecycleOwner: LifecycleOwner) {
    DisposableEffect(lifecycleOwner) {  // GOOD: Proper key
        val observer = LifecycleEventObserver { _, event ->
            // Handle lifecycle
        }
        lifecycleOwner.lifecycle.addObserver(observer)

        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
        }
    }
}

// GOOD: produceState with proper keys
@Composable
fun GoodProduceState(url: String): State<Image?> {
    return produceState<Image?>(initialValue = null, url) {
        value = loadImage(url)
    }
}

// GOOD: rememberCoroutineScope for user-triggered actions
@Composable
fun GoodCoroutineScope() {
    val scope = rememberCoroutineScope()
    var isLoading by remember { mutableStateOf(false) }

    Button(onClick = {
        scope.launch {
            isLoading = true
            doWork()
            isLoading = false
        }
    }) {
        Text(if (isLoading) "Loading..." else "Click me")
    }
}
