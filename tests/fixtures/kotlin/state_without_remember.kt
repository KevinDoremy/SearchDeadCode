// Test fixture for StateWithoutRememberDetector (AP031)
// Detects state variables without proper remember {} wrapper

package com.example.compose

import androidx.compose.runtime.*
import androidx.compose.material.*
import androidx.compose.foundation.layout.*

// BAD: State without remember - will reset on every recomposition
@Composable
fun BadCounter() {
    var count = mutableStateOf(0)  // BAD: No remember!

    Button(onClick = { count.value++ }) {
        Text("Count: ${count.value}")
    }
}

// BAD: Multiple state variables without remember
@Composable
fun BadForm() {
    var name = mutableStateOf("")      // BAD
    var email = mutableStateOf("")     // BAD
    var isValid = mutableStateOf(false) // BAD

    Column {
        TextField(value = name.value, onValueChange = { name.value = it })
        TextField(value = email.value, onValueChange = { email.value = it })
    }
}

// BAD: State created in local variable
@Composable
fun BadToggle() {
    val enabled = mutableStateOf(true)  // BAD: Will reset!

    Switch(
        checked = enabled.value,
        onCheckedChange = { enabled.value = it }
    )
}

// GOOD: Proper state with remember
@Composable
fun GoodCounter() {
    var count by remember { mutableStateOf(0) }

    Button(onClick = { count++ }) {
        Text("Count: $count")
    }
}

// GOOD: Using rememberSaveable for configuration changes
@Composable
fun GoodSaveableCounter() {
    var count by rememberSaveable { mutableStateOf(0) }

    Button(onClick = { count++ }) {
        Text("Count: $count")
    }
}

// GOOD: State hoisted from ViewModel
@Composable
fun GoodHoistedState(
    count: Int,
    onCountChange: (Int) -> Unit
) {
    Button(onClick = { onCountChange(count + 1) }) {
        Text("Count: $count")
    }
}

// GOOD: derivedStateOf with remember
@Composable
fun GoodDerivedState(items: List<String>) {
    val sortedItems by remember(items) {
        derivedStateOf { items.sorted() }
    }

    LazyColumn {
        items(sortedItems) { item ->
            Text(item)
        }
    }
}

// GOOD: State in ViewModel, not in composable
class CounterViewModel : ViewModel() {
    private val _count = MutableStateFlow(0)
    val count: StateFlow<Int> = _count.asStateFlow()

    fun increment() {
        _count.value++
    }
}

@Composable
fun GoodViewModelState(viewModel: CounterViewModel = viewModel()) {
    val count by viewModel.count.collectAsState()

    Button(onClick = { viewModel.increment() }) {
        Text("Count: $count")
    }
}
