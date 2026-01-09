// Test fixture for NestedCallbackDetector (AP019)
// Detects callback hell / deeply nested callbacks

package com.example.antipattern

// BAD: Callback hell - deeply nested callbacks
class CallbackHell {

    // BAD: Pyramid of doom
    fun loadUserData(userId: String) {
        userService.getUser(userId) { user ->
            if (user != null) {
                orderService.getOrders(user.id) { orders ->
                    if (orders.isNotEmpty()) {
                        paymentService.getPaymentMethods(user.id) { payments ->
                            if (payments.isNotEmpty()) {
                                shippingService.getAddresses(user.id) { addresses ->
                                    // Finally we can do something!
                                    updateUI(user, orders, payments, addresses)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // BAD: Nested error handling
    fun fetchWithRetry(url: String) {
        networkClient.fetch(url) { result1 ->
            if (result1.isFailure) {
                networkClient.fetch(url) { result2 ->
                    if (result2.isFailure) {
                        networkClient.fetch(url) { result3 ->
                            if (result3.isFailure) {
                                showError("Failed after 3 retries")
                            } else {
                                handleSuccess(result3)
                            }
                        }
                    } else {
                        handleSuccess(result2)
                    }
                }
            } else {
                handleSuccess(result1)
            }
        }
    }

    // BAD: Nested async operations
    fun processSequentially(items: List<Item>) {
        processItem(items[0]) { result0 ->
            processItem(items[1]) { result1 ->
                processItem(items[2]) { result2 ->
                    processItem(items[3]) { result3 ->
                        combineResults(result0, result1, result2, result3)
                    }
                }
            }
        }
    }
}

// GOOD: Using coroutines instead of callbacks
class CoroutineApproach {

    suspend fun loadUserData(userId: String) {
        val user = userService.getUserAsync(userId)
        val orders = orderService.getOrdersAsync(user.id)
        val payments = paymentService.getPaymentMethodsAsync(user.id)
        val addresses = shippingService.getAddressesAsync(user.id)
        updateUI(user, orders, payments, addresses)
    }

    suspend fun fetchWithRetry(url: String): Result<Data> {
        repeat(3) { attempt ->
            val result = networkClient.fetchAsync(url)
            if (result.isSuccess) return result
        }
        return Result.failure(Exception("Failed after 3 retries"))
    }

    suspend fun processSequentially(items: List<Item>): List<ProcessedItem> {
        return items.map { item ->
            processItemAsync(item)
        }
    }
}

// GOOD: Using RxJava/Flow operators
class ReactiveApproach {

    fun loadUserData(userId: String): Observable<DashboardData> {
        return userService.getUserRx(userId)
            .flatMap { user ->
                Observable.zip(
                    orderService.getOrdersRx(user.id),
                    paymentService.getPaymentMethodsRx(user.id),
                    shippingService.getAddressesRx(user.id)
                ) { orders, payments, addresses ->
                    DashboardData(user, orders, payments, addresses)
                }
            }
    }

    fun fetchWithRetry(url: String): Observable<Data> {
        return networkClient.fetchRx(url)
            .retry(3)
            .onErrorReturn { Data.empty() }
    }
}

// GOOD: Breaking down into smaller functions
class RefactoredCallbacks {

    fun loadUserData(userId: String) {
        loadUser(userId)
    }

    private fun loadUser(userId: String) {
        userService.getUser(userId) { user ->
            if (user != null) {
                loadOrders(user)
            }
        }
    }

    private fun loadOrders(user: User) {
        orderService.getOrders(user.id) { orders ->
            loadPayments(user, orders)
        }
    }

    private fun loadPayments(user: User, orders: List<Order>) {
        paymentService.getPaymentMethods(user.id) { payments ->
            updateUI(user, orders, payments)
        }
    }
}

// OK: Single level callback is fine
class SingleCallback {

    fun loadUser(userId: String) {
        userService.getUser(userId) { user ->
            updateUI(user)
        }
    }
}

// Supporting types
interface UserService {
    fun getUser(id: String, callback: (User?) -> Unit)
    suspend fun getUserAsync(id: String): User
    fun getUserRx(id: String): Observable<User>
}

interface OrderService {
    fun getOrders(userId: String, callback: (List<Order>) -> Unit)
    suspend fun getOrdersAsync(userId: String): List<Order>
    fun getOrdersRx(userId: String): Observable<List<Order>>
}

interface PaymentService {
    fun getPaymentMethods(userId: String, callback: (List<Payment>) -> Unit)
    suspend fun getPaymentMethodsAsync(userId: String): List<Payment>
    fun getPaymentMethodsRx(userId: String): Observable<List<Payment>>
}

interface ShippingService {
    fun getAddresses(userId: String, callback: (List<Address>) -> Unit)
    suspend fun getAddressesAsync(userId: String): List<Address>
    fun getAddressesRx(userId: String): Observable<List<Address>>
}

interface NetworkClient {
    fun fetch(url: String, callback: (Result<Data>) -> Unit)
    suspend fun fetchAsync(url: String): Result<Data>
    fun fetchRx(url: String): Observable<Data>
}

data class User(val id: String)
data class Order(val id: String)
data class Payment(val id: String)
data class Address(val id: String)
data class Data(val content: String) {
    companion object {
        fun empty() = Data("")
    }
}
data class Item(val id: String)
data class ProcessedItem(val id: String)
data class DashboardData(
    val user: User,
    val orders: List<Order>,
    val payments: List<Payment>,
    val addresses: List<Address>
)

class Observable<T>
