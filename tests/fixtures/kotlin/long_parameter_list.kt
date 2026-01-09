// Test fixture for LongParameterListDetector (AP023)
// Detects functions with too many parameters

package com.example.antipattern

// BAD: Functions with too many parameters
class TooManyParameters {

    // BAD: 8 parameters - hard to call correctly
    fun createUser(
        firstName: String,
        lastName: String,
        email: String,
        phone: String,
        address: String,
        city: String,
        country: String,
        postalCode: String
    ): User {
        return User(firstName, lastName, email, phone, address, city, country, postalCode)
    }

    // BAD: 10 parameters - very confusing
    fun configureServer(
        host: String,
        port: Int,
        timeout: Long,
        maxConnections: Int,
        useSsl: Boolean,
        certPath: String,
        keyPath: String,
        logLevel: String,
        retryCount: Int,
        bufferSize: Int
    ) {
        // Configure...
    }

    // BAD: Boolean parameters are especially confusing
    fun processData(
        data: String,
        validate: Boolean,
        transform: Boolean,
        cache: Boolean,
        async: Boolean,
        retry: Boolean,
        log: Boolean
    ) {
        // What does processData(data, true, false, true, false, true, false) mean?
    }
}

// GOOD: Using data classes/builder pattern
class BetterApproach {

    // GOOD: Use data class for related parameters
    data class UserDetails(
        val firstName: String,
        val lastName: String,
        val email: String,
        val phone: String,
        val address: Address
    )

    data class Address(
        val street: String,
        val city: String,
        val country: String,
        val postalCode: String
    )

    fun createUser(details: UserDetails): User {
        return User(
            details.firstName,
            details.lastName,
            details.email,
            details.phone,
            details.address.street,
            details.address.city,
            details.address.country,
            details.address.postalCode
        )
    }

    // GOOD: Builder pattern for complex configuration
    data class ServerConfig(
        val host: String,
        val port: Int = 8080,
        val timeout: Long = 30000,
        val maxConnections: Int = 100,
        val useSsl: Boolean = false,
        val certPath: String? = null,
        val keyPath: String? = null
    )

    fun configureServer(config: ServerConfig) {
        // Configure...
    }

    // GOOD: Options object for boolean flags
    data class ProcessOptions(
        val validate: Boolean = true,
        val transform: Boolean = false,
        val cache: Boolean = true,
        val async: Boolean = false
    )

    fun processData(data: String, options: ProcessOptions = ProcessOptions()) {
        // Clear what each option means
    }
}

// OK: Constructor with many parameters but using @Inject
class InjectedDependencies @javax.inject.Inject constructor(
    private val userRepository: UserRepository,
    private val orderRepository: OrderRepository,
    private val paymentService: PaymentService,
    private val notificationService: NotificationService,
    private val analyticsTracker: AnalyticsTracker,
    private val logger: Logger
) {
    // DI frameworks handle this, parameters are clear interfaces
}

// OK: Reasonable number of parameters (3-4)
class ReasonableParameters {

    fun formatName(firstName: String, lastName: String, title: String?): String {
        return "${title ?: ""} $firstName $lastName".trim()
    }

    fun calculatePrice(basePrice: Double, quantity: Int, discount: Double): Double {
        return basePrice * quantity * (1 - discount)
    }
}

// OK: Extension functions with context
fun String.formatWith(prefix: String, suffix: String, separator: String): String {
    return "$prefix$separator$this$separator$suffix"
}

// Supporting types
data class User(
    val firstName: String,
    val lastName: String,
    val email: String,
    val phone: String,
    val street: String,
    val city: String,
    val country: String,
    val postalCode: String
)

interface UserRepository
interface OrderRepository
interface PaymentService
interface NotificationService
interface AnalyticsTracker
interface Logger
