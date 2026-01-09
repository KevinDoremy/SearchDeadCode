// Test fixture for NullabilityOverloadDetector (AP021)
// Detects overly defensive null handling

package com.example.antipattern

// BAD: Excessive force unwrap (!!)
class ForceUnwrapAbuse {

    // BAD: Multiple !! in one expression
    fun processUser(user: User?) {
        val name = user!!.profile!!.name!!.uppercase()
        val email = user!!.email!!.trim()
    }

    // BAD: !! when safe alternatives exist
    fun getLength(text: String?): Int {
        return text!!.length  // Could crash!
    }

    // BAD: !! in loop
    fun processAll(items: List<Item?>) {
        for (item in items) {
            println(item!!.name)  // Dangerous
        }
    }
}

// BAD: Redundant null checks
class RedundantNullChecks {

    // BAD: if-null check followed by !!
    fun redundantCheck(value: String?) {
        if (value != null) {
            println(value!!.length)  // !! is redundant after null check
        }
    }

    // BAD: ?.let { it } pattern (redundant let)
    fun redundantLet(name: String?): String {
        return name?.let { it } ?: ""  // Just use name ?: ""
    }

    // BAD: Checking then force unwrapping
    fun checkThenForce(data: Data?) {
        if (data == null) return
        process(data!!)  // !! is unnecessary, smart cast should work
    }
}

// BAD: Overly defensive patterns
class OverlyDefensive {

    // BAD: Double null check
    fun doubleCheck(value: String?) {
        if (value != null && value != null) {  // Redundant
            println(value)
        }
    }

    // BAD: Unnecessary Elvis with same value
    fun unnecessaryElvis(name: String?) {
        val result = name ?: name  // Pointless
    }

    // BAD: Safe call on non-null after check
    fun safeAfterCheck(user: User?) {
        if (user != null) {
            user?.doSomething()  // ?. unnecessary after null check
        }
    }
}

// GOOD: Proper null handling
class ProperNullHandling {

    // GOOD: Safe call with Elvis
    fun getNameOrDefault(user: User?): String {
        return user?.name ?: "Unknown"
    }

    // GOOD: Using let for scoping
    fun processIfPresent(data: Data?) {
        data?.let { validData ->
            // Process valid data
            println(validData.value)
        }
    }

    // GOOD: Early return pattern
    fun processUser(user: User?) {
        val validUser = user ?: return
        println(validUser.name)
    }

    // GOOD: require/checkNotNull for preconditions
    fun mustHaveUser(user: User?) {
        requireNotNull(user) { "User must not be null" }
        println(user.name)  // Smart cast works
    }
}

// OK: Legitimate !! usage
class LegitimateForceUnwrap {

    private lateinit var user: User

    // OK: After lateinit initialization check
    fun afterInit() {
        if (::user.isInitialized) {
            println(user.name)
        }
    }

    // OK: In tests with known state
    fun testCase() {
        val result = apiCall()
        // In test, we know this won't be null
        assertEquals("expected", result!!.value)
    }
}

// Supporting types
data class User(
    val name: String,
    val email: String?,
    val profile: Profile?
) {
    fun doSomething() {}
}

data class Profile(val name: String?)
data class Data(val value: String)
data class Item(val name: String)
