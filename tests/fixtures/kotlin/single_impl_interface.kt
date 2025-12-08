// Test fixture: Single Implementation Interface Anti-Pattern
// Anti-pattern #7 from "8 anti-patterns in Android codebase"
package com.example.fixtures.antipatterns

// ANTI-PATTERN: Interface with only one implementation
interface UserRepository {
    fun getUsers(): List<String>
    fun getUserById(id: String): String?
    fun saveUser(user: String)
}

// Only implementation - interface is unnecessary
class UserRepositoryImpl : UserRepository {
    override fun getUsers(): List<String> = listOf("user1", "user2")
    override fun getUserById(id: String): String? = "user_$id"
    override fun saveUser(user: String) {}
}

// ANTI-PATTERN: Another unnecessary interface
interface DataSource {
    fun fetchData(): String
}

class RemoteDataSource : DataSource {
    override fun fetchData(): String = "remote data"
}
// Note: No LocalDataSource exists, so interface is unnecessary

// ANTI-PATTERN: Interface for ViewModel (very common mistake)
interface MainViewModelContract {
    fun loadData()
    fun refreshData()
}

class MainViewModel : MainViewModelContract {
    override fun loadData() {}
    override fun refreshData() {}
}

// ANTI-PATTERN: Interface for UseCase with single impl
interface GetUsersUseCase {
    suspend fun execute(): List<String>
}

class GetUsersUseCaseImpl : GetUsersUseCase {
    override suspend fun execute(): List<String> = listOf()
}

// OK: Interface with multiple implementations
interface PaymentProcessor {
    fun processPayment(amount: Double): Boolean
}

class StripePaymentProcessor : PaymentProcessor {
    override fun processPayment(amount: Double): Boolean = true
}

class PayPalPaymentProcessor : PaymentProcessor {
    override fun processPayment(amount: Double): Boolean = true
}

class CryptoPaymentProcessor : PaymentProcessor {
    override fun processPayment(amount: Double): Boolean = true
}

// OK: Interface for testing (fake implementation)
interface NetworkClient {
    fun get(url: String): String
}

class RealNetworkClient : NetworkClient {
    override fun get(url: String): String = "real response"
}

class FakeNetworkClient : NetworkClient {  // For testing
    override fun get(url: String): String = "fake response"
}

// OK: Interface for platform abstraction
interface FileSystem {
    fun readFile(path: String): String
    fun writeFile(path: String, content: String)
}

class AndroidFileSystem : FileSystem {
    override fun readFile(path: String): String = "android file"
    override fun writeFile(path: String, content: String) {}
}

class DesktopFileSystem : FileSystem {
    override fun readFile(path: String): String = "desktop file"
    override fun writeFile(path: String, content: String) {}
}

// BETTER: Just use the class directly when single impl
class SimpleUserRepository {
    fun getUsers(): List<String> = listOf("user1", "user2")
    fun getUserById(id: String): String? = "user_$id"
    fun saveUser(user: String) {}
}

// BETTER: Inject the class directly
class UserService(
    private val repository: SimpleUserRepository  // No interface needed
) {
    fun fetchUsers() = repository.getUsers()
}

fun main() {
    val repo = UserRepositoryImpl()
    println(repo.getUsers())

    val simpleRepo = SimpleUserRepository()
    println(simpleRepo.getUsers())
}
