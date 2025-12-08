// Test fixture: Global Mutable State Anti-Pattern
// Anti-pattern #3 from "8 anti-patterns in Android codebase"
package com.example.fixtures.antipatterns

// ANTI-PATTERN: Object with public mutable vars (global state)
object GlobalState {
    var currentUser: String? = null      // BAD: global mutable state
    var isLoggedIn: Boolean = false      // BAD: global mutable state
    var sessionToken: String = ""        // BAD: global mutable state
    var lastSyncTime: Long = 0           // BAD: global mutable state

    fun login(user: String, token: String) {
        currentUser = user
        sessionToken = token
        isLoggedIn = true
    }

    fun logout() {
        currentUser = null
        sessionToken = ""
        isLoggedIn = false
    }
}

// ANTI-PATTERN: Another global state singleton
object AppConfig {
    var debugMode: Boolean = false       // BAD: can be changed anywhere
    var apiBaseUrl: String = ""          // BAD: can be changed anywhere
    var featureFlags: MutableMap<String, Boolean> = mutableMapOf()  // BAD
}

// OK: Object with only val (immutable)
object Constants {
    val MAX_RETRIES = 3                  // OK: immutable
    val TIMEOUT_MS = 30000L              // OK: immutable
    val API_VERSION = "v1"               // OK: immutable
}

// OK: Object with only functions (utility)
object StringUtils {
    fun capitalize(s: String): String = s.replaceFirstChar { it.uppercase() }
    fun isEmpty(s: String?): Boolean = s.isNullOrEmpty()
}

// OK: Object with private mutable state (encapsulated)
object Counter {
    private var count = 0                // OK: private

    fun increment(): Int {
        count++
        return count
    }

    fun getCount(): Int = count
}

// ANTI-PATTERN: Companion object with global state
class UserRepository {
    companion object {
        var instance: UserRepository? = null  // BAD: mutable singleton
        var cachedUsers: List<String> = emptyList()  // BAD: global cache

        fun getInstance(): UserRepository {
            if (instance == null) {
                instance = UserRepository()
            }
            return instance!!
        }
    }

    fun getUsers(): List<String> = cachedUsers
}

// OK: Using proper DI pattern
class UserService(
    private val repository: UserRepository  // Good: injected dependency
) {
    fun fetchUsers(): List<String> = repository.getUsers()
}

fun main() {
    // Demonstrating the problem with global state
    GlobalState.login("user1", "token123")
    println("User: ${GlobalState.currentUser}")

    // This can be called from anywhere, making debugging hard
    GlobalState.currentUser = "hacked!"  // BAD: direct mutation

    AppConfig.debugMode = true
    AppConfig.featureFlags["new_feature"] = true
}
