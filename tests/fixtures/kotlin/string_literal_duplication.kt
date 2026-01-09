// Test fixture for StringLiteralDuplicationDetector (AP025)
// Detects repeated string literals (magic strings)

package com.example.antipattern

// BAD: Repeated string literals (magic strings)
class MagicStrings {

    // BAD: Same key used multiple times
    fun saveSettings(prefs: SharedPreferences) {
        prefs.edit()
            .putString("user_name", userName)
            .putString("user_email", userEmail)
            .apply()
    }

    fun loadSettings(prefs: SharedPreferences) {
        userName = prefs.getString("user_name", "")  // Duplicate!
        userEmail = prefs.getString("user_email", "")  // Duplicate!
    }

    fun clearSettings(prefs: SharedPreferences) {
        prefs.edit()
            .remove("user_name")  // Duplicate!
            .remove("user_email")  // Duplicate!
            .apply()
    }

    // BAD: Intent extra keys duplicated
    fun startActivity(context: Context) {
        val intent = Intent(context, DetailActivity::class.java)
        intent.putExtra("item_id", itemId)
        intent.putExtra("item_name", itemName)
        intent.putExtra("item_price", itemPrice)
        context.startActivity(intent)
    }

    fun extractData(intent: Intent) {
        val id = intent.getStringExtra("item_id")  // Duplicate!
        val name = intent.getStringExtra("item_name")  // Duplicate!
        val price = intent.getDoubleExtra("item_price", 0.0)  // Duplicate!
    }

    // BAD: API endpoints duplicated
    fun fetchUsers() {
        api.get("/api/v1/users")
    }

    fun updateUser(id: String) {
        api.put("/api/v1/users/$id")
    }

    fun deleteUser(id: String) {
        api.delete("/api/v1/users/$id")  // "/api/v1/users" repeated
    }

    // BAD: Error messages duplicated
    fun validate(input: String) {
        if (input.isEmpty()) {
            throw ValidationException("Input cannot be empty")
        }
    }

    fun validateName(name: String) {
        if (name.isEmpty()) {
            throw ValidationException("Input cannot be empty")  // Duplicate!
        }
    }
}

// GOOD: Using constants
class WithConstants {

    companion object {
        private const val KEY_USER_NAME = "user_name"
        private const val KEY_USER_EMAIL = "user_email"
        private const val EXTRA_ITEM_ID = "item_id"
        private const val EXTRA_ITEM_NAME = "item_name"
        private const val EXTRA_ITEM_PRICE = "item_price"
        private const val API_BASE = "/api/v1"
        private const val ERROR_EMPTY_INPUT = "Input cannot be empty"
    }

    fun saveSettings(prefs: SharedPreferences) {
        prefs.edit()
            .putString(KEY_USER_NAME, userName)
            .putString(KEY_USER_EMAIL, userEmail)
            .apply()
    }

    fun loadSettings(prefs: SharedPreferences) {
        userName = prefs.getString(KEY_USER_NAME, "")
        userEmail = prefs.getString(KEY_USER_EMAIL, "")
    }

    fun startActivity(context: Context) {
        val intent = Intent(context, DetailActivity::class.java)
        intent.putExtra(EXTRA_ITEM_ID, itemId)
        intent.putExtra(EXTRA_ITEM_NAME, itemName)
        intent.putExtra(EXTRA_ITEM_PRICE, itemPrice)
        context.startActivity(intent)
    }

    fun extractData(intent: Intent) {
        val id = intent.getStringExtra(EXTRA_ITEM_ID)
        val name = intent.getStringExtra(EXTRA_ITEM_NAME)
        val price = intent.getDoubleExtra(EXTRA_ITEM_PRICE, 0.0)
    }

    fun fetchUsers() {
        api.get("$API_BASE/users")
    }

    fun validate(input: String) {
        if (input.isEmpty()) {
            throw ValidationException(ERROR_EMPTY_INPUT)
        }
    }
}

// GOOD: Using sealed class/enum for keys
object PrefsKeys {
    const val USER_NAME = "user_name"
    const val USER_EMAIL = "user_email"
}

object IntentExtras {
    const val ITEM_ID = "item_id"
    const val ITEM_NAME = "item_name"
    const val ITEM_PRICE = "item_price"
}

// OK: Common single-use strings
class AcceptableStrings {

    fun log(message: String) {
        println("[INFO] $message")  // "[INFO]" only used here
    }

    fun formatDate(date: Date): String {
        return SimpleDateFormat("yyyy-MM-dd").format(date)  // Format pattern
    }

    // OK: Empty string, common separators
    fun join(items: List<String>): String {
        return items.joinToString(", ")
    }

    fun getDefault(): String {
        return ""  // Empty string is fine
    }
}

// Supporting types
interface SharedPreferences {
    fun edit(): Editor
    fun getString(key: String, default: String?): String?

    interface Editor {
        fun putString(key: String, value: String?): Editor
        fun remove(key: String): Editor
        fun apply()
    }
}

class Intent(context: Context, cls: Class<*>) {
    fun putExtra(key: String, value: String?): Intent = this
    fun putExtra(key: String, value: Double): Intent = this
    fun getStringExtra(key: String): String? = null
    fun getDoubleExtra(key: String, default: Double): Double = default
}

interface Context {
    fun startActivity(intent: Intent)
}

class DetailActivity
class ValidationException(message: String) : Exception(message)
class Date
class SimpleDateFormat(pattern: String) {
    fun format(date: Date): String = ""
}

interface Api {
    fun get(url: String)
    fun put(url: String)
    fun delete(url: String)
}

private var userName: String = ""
private var userEmail: String = ""
private var itemId: String = ""
private var itemName: String = ""
private var itemPrice: Double = 0.0
private val api: Api = object : Api {
    override fun get(url: String) {}
    override fun put(url: String) {}
    override fun delete(url: String) {}
}
