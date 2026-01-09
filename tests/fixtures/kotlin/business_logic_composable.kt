// Test fixture for BusinessLogicInComposableDetector (AP033)
// Detects non-UI logic in @Composable functions

package com.example.compose

import androidx.compose.runtime.*
import androidx.compose.material.*
import kotlinx.coroutines.flow.*

// BAD: Business logic directly in composable
@Composable
fun BadUserProfile(userId: String) {
    var user by remember { mutableStateOf<User?>(null) }

    LaunchedEffect(userId) {
        // BAD: Network call in composable
        val response = retrofit.userService.getUser(userId)
        if (response.isSuccessful) {
            user = response.body()
        }
    }

    user?.let { UserCard(it) }
}

// BAD: Database operations in composable
@Composable
fun BadDatabaseAccess() {
    var items by remember { mutableStateOf<List<Item>>(emptyList()) }

    LaunchedEffect(Unit) {
        // BAD: Direct DAO access
        items = database.itemDao().getAllItems()
    }

    ItemsList(items)
}

// BAD: Complex calculations in composable
@Composable
fun BadCalculation(orders: List<Order>) {
    // BAD: Heavy computation during composition
    val totalRevenue = orders
        .filter { it.status == OrderStatus.COMPLETED }
        .sumOf { it.items.sumOf { item -> item.price * item.quantity } }

    val averageOrderValue = if (orders.isNotEmpty()) {
        totalRevenue / orders.size
    } else 0.0

    Text("Total: $$totalRevenue, Avg: $$averageOrderValue")
}

// BAD: Validation logic in composable
@Composable
fun BadFormValidation() {
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }

    // BAD: Validation logic mixed with UI
    val isEmailValid = email.contains("@") &&
                       email.contains(".") &&
                       email.length >= 5

    val isPasswordValid = password.length >= 8 &&
                          password.any { it.isDigit() } &&
                          password.any { it.isUpperCase() }

    Column {
        TextField(value = email, onValueChange = { email = it })
        if (!isEmailValid && email.isNotEmpty()) {
            Text("Invalid email", color = Color.Red)
        }

        TextField(value = password, onValueChange = { password = it })
        if (!isPasswordValid && password.isNotEmpty()) {
            Text("Password too weak", color = Color.Red)
        }
    }
}

// BAD: SharedPreferences access in composable
@Composable
fun BadPrefsAccess(context: Context) {
    var theme by remember { mutableStateOf("light") }

    LaunchedEffect(Unit) {
        // BAD: Direct prefs access
        val prefs = context.getSharedPreferences("settings", Context.MODE_PRIVATE)
        theme = prefs.getString("theme", "light") ?: "light"
    }

    ThemeSelector(theme)
}

// GOOD: Business logic in ViewModel
class UserProfileViewModel(
    private val userRepository: UserRepository
) : ViewModel() {
    private val _user = MutableStateFlow<User?>(null)
    val user: StateFlow<User?> = _user.asStateFlow()

    fun loadUser(userId: String) {
        viewModelScope.launch {
            _user.value = userRepository.getUser(userId)
        }
    }
}

@Composable
fun GoodUserProfile(
    userId: String,
    viewModel: UserProfileViewModel = viewModel()
) {
    val user by viewModel.user.collectAsState()

    LaunchedEffect(userId) {
        viewModel.loadUser(userId)
    }

    user?.let { UserCard(it) }
}

// GOOD: Derived state for simple UI transformations
@Composable
fun GoodDerivedState(items: List<Item>) {
    val sortedItems by remember(items) {
        derivedStateOf { items.sortedBy { it.name } }
    }

    ItemsList(sortedItems)
}

// GOOD: Validation in ViewModel/UseCase
class FormViewModel : ViewModel() {
    private val _formState = MutableStateFlow(FormState())
    val formState: StateFlow<FormState> = _formState.asStateFlow()

    fun updateEmail(email: String) {
        _formState.update { it.copy(
            email = email,
            isEmailValid = validateEmail(email)
        )}
    }

    private fun validateEmail(email: String): Boolean {
        return android.util.Patterns.EMAIL_ADDRESS.matcher(email).matches()
    }
}

@Composable
fun GoodFormValidation(viewModel: FormViewModel = viewModel()) {
    val state by viewModel.formState.collectAsState()

    Column {
        TextField(
            value = state.email,
            onValueChange = { viewModel.updateEmail(it) }
        )
        if (!state.isEmailValid && state.email.isNotEmpty()) {
            Text("Invalid email", color = Color.Red)
        }
    }
}

// GOOD: Using remember for simple memoization
@Composable
fun GoodMemoization(items: List<String>) {
    val itemCount = remember(items) { items.size }

    Text("$itemCount items")
}
