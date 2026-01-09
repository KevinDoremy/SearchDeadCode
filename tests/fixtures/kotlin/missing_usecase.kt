// Test fixture for MissingUseCaseDetector (AP018)
// Detects Repository called directly from ViewModel (bypassing domain layer)

package com.example.antipattern

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.launch

// BAD: ViewModel directly depends on Repository (no domain layer)
class BadViewModel(
    private val userRepository: UserRepository,
    private val orderRepository: OrderRepository,
    private val productRepository: ProductRepository
) : ViewModel() {

    // Direct repository access - no business logic abstraction
    fun loadUser(id: String) {
        viewModelScope.launch {
            val user = userRepository.getUser(id)
            // Business logic mixed with presentation
            if (user.isActive && user.orders.isNotEmpty()) {
                // Complex logic that should be in UseCase
            }
        }
    }

    // Multiple repositories = sign of missing UseCase
    fun loadDashboard() {
        viewModelScope.launch {
            val user = userRepository.getCurrentUser()
            val orders = orderRepository.getRecentOrders()
            val products = productRepository.getRecommended()
            // Orchestration logic that belongs in UseCase
        }
    }
}

// BAD: ViewModel with too many repository dependencies
class OverloadedViewModel(
    private val userRepository: UserRepository,
    private val orderRepository: OrderRepository,
    private val productRepository: ProductRepository,
    private val paymentRepository: PaymentRepository,
    private val shippingRepository: ShippingRepository
) : ViewModel() {
    // Too many repositories = definitely need UseCases
}

// GOOD: ViewModel depends on UseCase (domain layer)
class GoodViewModel(
    private val getUserUseCase: GetUserUseCase,
    private val loadDashboardUseCase: LoadDashboardUseCase
) : ViewModel() {

    fun loadUser(id: String) {
        viewModelScope.launch {
            // UseCase handles business logic
            val result = getUserUseCase(id)
            // ViewModel only handles presentation
        }
    }

    fun loadDashboard() {
        viewModelScope.launch {
            // Single UseCase orchestrates multiple repositories
            val dashboard = loadDashboardUseCase()
            // Clean separation
        }
    }
}

// GOOD: Using Interactor pattern (same as UseCase)
class AnotherGoodViewModel(
    private val userInteractor: UserInteractor,
    private val checkoutInteractor: CheckoutInteractor
) : ViewModel() {

    fun processCheckout() {
        viewModelScope.launch {
            checkoutInteractor.execute()
        }
    }
}

// OK: Single repository for simple screens
class SimpleViewModel(
    private val settingsRepository: SettingsRepository
) : ViewModel() {
    // Single repository is acceptable for simple CRUD screens
    fun loadSettings() {
        // Simple data loading
    }
}

// Domain layer classes
interface UserRepository {
    suspend fun getUser(id: String): User
    suspend fun getCurrentUser(): User
}

interface OrderRepository {
    suspend fun getRecentOrders(): List<Order>
}

interface ProductRepository {
    suspend fun getRecommended(): List<Product>
}

interface PaymentRepository
interface ShippingRepository
interface SettingsRepository

// UseCases (domain layer)
class GetUserUseCase(private val userRepository: UserRepository) {
    suspend operator fun invoke(id: String): User {
        val user = userRepository.getUser(id)
        // Business logic here
        return user
    }
}

class LoadDashboardUseCase(
    private val userRepository: UserRepository,
    private val orderRepository: OrderRepository,
    private val productRepository: ProductRepository
) {
    suspend operator fun invoke(): Dashboard {
        // Orchestrate multiple repositories
        val user = userRepository.getCurrentUser()
        val orders = orderRepository.getRecentOrders()
        val products = productRepository.getRecommended()
        return Dashboard(user, orders, products)
    }
}

interface UserInteractor {
    suspend fun getUser(id: String): User
}

interface CheckoutInteractor {
    suspend fun execute()
}

// Data classes
data class User(val id: String, val name: String, val isActive: Boolean, val orders: List<Order>)
data class Order(val id: String)
data class Product(val id: String)
data class Dashboard(val user: User, val orders: List<Order>, val products: List<Product>)
