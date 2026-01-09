// Test fixture for ComplexConditionDetector (AP024)
// Detects conditions with too many boolean operators

package com.example.antipattern

// BAD: Overly complex conditions
class ComplexConditions {

    // BAD: Too many && operators
    fun isValidUser(user: User): Boolean {
        return user.name.isNotEmpty() &&
                user.email.contains("@") &&
                user.age >= 18 &&
                user.age <= 120 &&
                user.country.isNotEmpty() &&
                user.isVerified &&
                !user.isBanned
    }

    // BAD: Mixed && and || without clear grouping
    fun shouldProcess(item: Item): Boolean {
        return item.isActive && item.quantity > 0 || item.isPriority && !item.isExpired ||
                item.category == "urgent" && item.price > 0 && item.inStock
    }

    // BAD: Nested conditions in when
    fun getDiscount(user: User, order: Order): Double {
        return when {
            user.isPremium && order.total > 100 && order.items.size > 5 && !order.hasDiscount -> 0.2
            user.isVip && order.total > 50 || user.hasSubscription && order.isFirstOrder -> 0.15
            order.total > 200 && order.items.size > 10 && user.orderCount > 5 && !order.isGift -> 0.1
            else -> 0.0
        }
    }

    // BAD: Long condition in if statement
    fun processOrder(order: Order) {
        if (order.isValid &&
            order.isPaid &&
            order.items.isNotEmpty() &&
            order.shippingAddress != null &&
            order.billingAddress != null &&
            !order.isCancelled &&
            order.total > 0 &&
            order.customer != null
        ) {
            // Process...
        }
    }
}

// GOOD: Extract to named booleans
class ClearConditions {

    // GOOD: Named boolean for clarity
    fun isValidUser(user: User): Boolean {
        val hasValidName = user.name.isNotEmpty()
        val hasValidEmail = user.email.contains("@")
        val hasValidAge = user.age in 18..120
        val hasValidLocation = user.country.isNotEmpty()
        val isAllowed = user.isVerified && !user.isBanned

        return hasValidName && hasValidEmail && hasValidAge && hasValidLocation && isAllowed
    }

    // GOOD: Extract to methods
    fun shouldProcess(item: Item): Boolean {
        return isActiveAndAvailable(item) || isPriorityItem(item) || isUrgentItem(item)
    }

    private fun isActiveAndAvailable(item: Item): Boolean {
        return item.isActive && item.quantity > 0
    }

    private fun isPriorityItem(item: Item): Boolean {
        return item.isPriority && !item.isExpired
    }

    private fun isUrgentItem(item: Item): Boolean {
        return item.category == "urgent" && item.price > 0 && item.inStock
    }

    // GOOD: Using extension functions
    fun processOrder(order: Order) {
        if (order.isReadyToProcess()) {
            // Process...
        }
    }
}

// GOOD: Extension function for complex check
fun Order.isReadyToProcess(): Boolean {
    val hasRequiredData = items.isNotEmpty() &&
            shippingAddress != null &&
            billingAddress != null &&
            customer != null

    val hasValidPayment = isPaid && total > 0

    val isNotCancelled = !isCancelled

    return isValid && hasRequiredData && hasValidPayment && isNotCancelled
}

// OK: Simple conditions (2-3 operators)
class SimpleConditions {

    fun isAdult(age: Int): Boolean {
        return age >= 18 && age <= 120
    }

    fun hasAccess(user: User): Boolean {
        return user.isVerified || user.isAdmin
    }

    fun canPurchase(user: User, item: Item): Boolean {
        return user.isVerified && item.inStock && item.price <= user.balance
    }
}

// Supporting types
data class User(
    val name: String,
    val email: String,
    val age: Int,
    val country: String,
    val isVerified: Boolean,
    val isBanned: Boolean,
    val isPremium: Boolean,
    val isVip: Boolean,
    val hasSubscription: Boolean,
    val orderCount: Int,
    val isAdmin: Boolean,
    val balance: Double
)

data class Item(
    val isActive: Boolean,
    val quantity: Int,
    val isPriority: Boolean,
    val isExpired: Boolean,
    val category: String,
    val price: Double,
    val inStock: Boolean
)

data class Order(
    val isValid: Boolean,
    val isPaid: Boolean,
    val items: List<Item>,
    val shippingAddress: String?,
    val billingAddress: String?,
    val isCancelled: Boolean,
    val total: Double,
    val customer: User?,
    val hasDiscount: Boolean,
    val isFirstOrder: Boolean,
    val isGift: Boolean
)
