// Test fixture: SharedPreferences write-only patterns
package com.example.fixtures.prefs

import android.content.Context
import android.content.SharedPreferences

// Case 1: Write-only SharedPreferences - key written but never read
class WriteOnlyPrefs(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("app", Context.MODE_PRIVATE)

    // DEAD: "last_sync_time" is written but never read
    fun saveLastSyncTime(time: Long) {
        prefs.edit().putLong("last_sync_time", time).apply()
    }

    // DEAD: "user_token" is written but never read
    fun saveUserToken(token: String) {
        prefs.edit().putString("user_token", token).apply()
    }
}

// Case 2: NOT write-only - key is both written and read
class ReadWritePrefs(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("app", Context.MODE_PRIVATE)

    fun saveUsername(name: String) {
        prefs.edit().putString("username", name).apply()
    }

    fun getUsername(): String {
        return prefs.getString("username", "") ?: ""  // READ here
    }
}

// Case 3: Mixed - some keys write-only, some read
class MixedPrefs(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("app", Context.MODE_PRIVATE)

    // DEAD: "debug_flag" is written but never read
    fun setDebugFlag(enabled: Boolean) {
        prefs.edit().putBoolean("debug_flag", enabled).apply()
    }

    // NOT DEAD: "theme" is both written and read
    fun saveTheme(theme: String) {
        prefs.edit().putString("theme", theme).apply()
    }

    fun getTheme(): String {
        return prefs.getString("theme", "light") ?: "light"
    }
}

// Case 4: SharedPreferences with commit() instead of apply()
class CommitPrefs(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("settings", Context.MODE_PRIVATE)

    // DEAD: "analytics_enabled" is written but never read
    fun disableAnalytics() {
        prefs.edit().putBoolean("analytics_enabled", false).commit()
    }
}

// Case 5: SharedPreferences delegate pattern (should NOT flag)
class DelegatePrefs(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("delegate", Context.MODE_PRIVATE)

    // NOT DEAD: delegate pattern reads via property
    var isFirstLaunch: Boolean
        get() = prefs.getBoolean("first_launch", true)
        set(value) {
            prefs.edit().putBoolean("first_launch", value).apply()
        }
}

// Case 6: Chained edit operations
class ChainedPrefs(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("chained", Context.MODE_PRIVATE)

    // DEAD: Multiple keys written but never read
    fun saveAllSettings(name: String, age: Int, premium: Boolean) {
        prefs.edit()
            .putString("settings_name", name)
            .putInt("settings_age", age)
            .putBoolean("settings_premium", premium)
            .apply()
    }
}

// Case 7: Extension function pattern
fun SharedPreferences.saveValue(key: String, value: String) {
    edit().putString(key, value).apply()
}

// Case 8: Key constants (common pattern)
class PrefsWithConstants(context: Context) {
    companion object {
        const val KEY_SESSION_ID = "session_id"
        const val KEY_LAST_LOGIN = "last_login"
    }

    private val prefs: SharedPreferences = context.getSharedPreferences("app", Context.MODE_PRIVATE)

    // DEAD: KEY_SESSION_ID written but never read
    fun saveSessionId(id: String) {
        prefs.edit().putString(KEY_SESSION_ID, id).apply()
    }

    // NOT DEAD: KEY_LAST_LOGIN is both written and read
    fun saveLastLogin(time: Long) {
        prefs.edit().putLong(KEY_LAST_LOGIN, time).apply()
    }

    fun getLastLogin(): Long {
        return prefs.getLong(KEY_LAST_LOGIN, 0)
    }
}
