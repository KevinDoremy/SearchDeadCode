// Test fixture for MutableStateExposedDetector (AP016)
// Detects public MutableLiveData/MutableStateFlow properties

package com.example.antipattern

import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.LiveData
import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

// BAD: Exposing mutable state directly
class BadViewModel : ViewModel() {

    // BAD: Public MutableLiveData - can be modified from outside
    val userData = MutableLiveData<User>()

    // BAD: Public MutableStateFlow - can be modified from outside
    val uiState = MutableStateFlow(UiState.Loading)

    // BAD: Protected is also problematic for subclasses
    protected val settings = MutableLiveData<Settings>()

    fun loadUser(id: String) {
        // Load user...
        userData.value = User(id, "John")
    }
}

// BAD: Fragment/Activity can now do: viewModel.userData.value = hackedUser
class BadFragment {
    private lateinit var viewModel: BadViewModel

    fun hackTheViewModel() {
        // This should not be possible!
        viewModel.userData.value = User("hacked", "Hacker")
        viewModel.uiState.value = UiState.Error("Injected error")
    }
}

// GOOD: Proper encapsulation with backing properties
class GoodViewModel : ViewModel() {

    // Private mutable backing property
    private val _userData = MutableLiveData<User>()
    // Public read-only exposure
    val userData: LiveData<User> = _userData

    // Private mutable backing property
    private val _uiState = MutableStateFlow(UiState.Loading)
    // Public read-only exposure
    val uiState: StateFlow<UiState> = _uiState.asStateFlow()

    // Private mutable backing property
    private val _settings = MutableLiveData<Settings>()
    // Public read-only exposure
    val settings: LiveData<Settings> = _settings

    fun loadUser(id: String) {
        // Only ViewModel can modify
        _userData.value = User(id, "John")
    }

    fun updateState(state: UiState) {
        // Only ViewModel can modify
        _uiState.value = state
    }
}

// GOOD: Fragment can only read, not write
class GoodFragment {
    private lateinit var viewModel: GoodViewModel

    fun observeData() {
        // Can only observe, cannot modify
        viewModel.userData.observe(this) { user ->
            // Update UI
        }
    }
}

// OK: Private mutable state is fine
class PrivateMutableState : ViewModel() {
    private val _data = MutableLiveData<String>()
    private val _flow = MutableStateFlow("")
}

// Supporting classes
data class User(val id: String, val name: String)
data class Settings(val darkMode: Boolean)
sealed class UiState {
    object Loading : UiState()
    data class Success(val data: String) : UiState()
    data class Error(val message: String) : UiState()
}
