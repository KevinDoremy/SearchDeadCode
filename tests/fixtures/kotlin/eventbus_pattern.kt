// Test fixture: EventBus Pattern Anti-Pattern
// Anti-pattern #8 from "8 anti-patterns in Android codebase"
package com.example.fixtures.antipatterns

import org.greenrobot.eventbus.EventBus
import org.greenrobot.eventbus.Subscribe
import org.greenrobot.eventbus.ThreadMode

// ANTI-PATTERN: Using EventBus library
class UserUpdatedEvent(val userId: String)
class DataRefreshEvent
class LoginEvent(val username: String)
class LogoutEvent

// ANTI-PATTERN: Activity using EventBus
class EventBusActivity {
    fun onStart() {
        EventBus.getDefault().register(this)  // BAD: EventBus usage
    }

    fun onStop() {
        EventBus.getDefault().unregister(this)  // BAD: EventBus usage
    }

    @Subscribe(threadMode = ThreadMode.MAIN)
    fun onUserUpdated(event: UserUpdatedEvent) {  // BAD: Event handler
        // Handle event
    }

    @Subscribe
    fun onDataRefresh(event: DataRefreshEvent) {  // BAD: Event handler
        // Refresh UI
    }

    fun updateUser(userId: String) {
        // After updating, post event - can be received anywhere!
        EventBus.getDefault().post(UserUpdatedEvent(userId))  // BAD: posting event
    }
}

// ANTI-PATTERN: Custom EventBus-like pattern
object CustomEventBus {
    private val listeners = mutableMapOf<Class<*>, MutableList<(Any) -> Unit>>()

    fun <T : Any> register(eventType: Class<T>, listener: (T) -> Unit) {
        @Suppress("UNCHECKED_CAST")
        listeners.getOrPut(eventType) { mutableListOf() }.add(listener as (Any) -> Unit)
    }

    fun post(event: Any) {
        listeners[event::class.java]?.forEach { it(event) }
    }
}

// ANTI-PATTERN: Using custom event bus
class CustomEventBusConsumer {
    init {
        CustomEventBus.register(UserUpdatedEvent::class.java) { event ->
            handleUserUpdate(event)
        }
    }

    private fun handleUserUpdate(event: UserUpdatedEvent) {
        println("User ${event.userId} updated")
    }
}

// ANTI-PATTERN: LiveData misused as EventBus
class SingleLiveEvent<T> {  // Anti-pattern: LiveData as event bus
    private var value: T? = null
    private val observers = mutableListOf<(T) -> Unit>()

    fun postValue(value: T) {
        this.value = value
        observers.forEach { it(value) }
    }

    fun observe(observer: (T) -> Unit) {
        observers.add(observer)
    }
}

class EventLiveDataViewModel {
    val navigationEvent = SingleLiveEvent<String>()  // BAD: event-style LiveData
    val showToastEvent = SingleLiveEvent<String>()   // BAD: event-style LiveData

    fun onButtonClick() {
        navigationEvent.postValue("details")  // BAD: posting events
    }
}

// BETTER: Use structured communication
// 1. Direct method calls
class DirectCommunication {
    private var listener: UserUpdateListener? = null

    interface UserUpdateListener {
        fun onUserUpdated(userId: String)
    }

    fun setListener(listener: UserUpdateListener) {
        this.listener = listener
    }

    fun updateUser(userId: String) {
        // Direct callback - traceable and debuggable
        listener?.onUserUpdated(userId)
    }
}

// 2. Use StateFlow for UI state
class StateFlowViewModel {
    // StateFlow for state (not events)
    private val _uiState = mutableListOf<String>()
    val uiState: List<String> get() = _uiState

    fun updateState(newData: String) {
        _uiState.add(newData)
    }
}

// 3. Use Navigation component for navigation
class NavigationViewModel {
    fun navigateToDetails(id: String) {
        // Use Navigation component directly
        // navController.navigate("details/$id")
    }
}

// 4. Use SharedFlow for one-time events (if needed)
class SharedFlowViewModel {
    // SharedFlow for events that need to be consumed once
    // private val _events = MutableSharedFlow<UiEvent>()
    // val events: SharedFlow<UiEvent> = _events.asSharedFlow()
}

fun main() {
    val activity = EventBusActivity()
    activity.onStart()
    activity.updateUser("user123")
    activity.onStop()
}
