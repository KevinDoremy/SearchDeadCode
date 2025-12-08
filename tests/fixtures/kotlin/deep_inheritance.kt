// Test fixture: Deep Inheritance Anti-Pattern
// Anti-pattern #6 from "8 anti-patterns in Android codebase"
package com.example.fixtures.antipatterns

// ANTI-PATTERN: Deep inheritance chain for Activities
// Level 0: Framework class (not in codebase)
// open class AppCompatActivity

// Level 1: Base activity
open class BaseActivity {
    open fun showLoading() {}
    open fun hideLoading() {}
    open fun showError(message: String) {}
}

// Level 2: Another base with more features
open class BaseViewModelActivity : BaseActivity() {
    open fun initViewModel() {}
    open fun observeData() {}
}

// Level 3: Even more specific base
open class BaseToolbarActivity : BaseViewModelActivity() {
    open fun setupToolbar() {}
    open fun setTitle(title: String) {}
}

// Level 4: Feature-specific base - TOO DEEP!
open class BaseListActivity : BaseToolbarActivity() {
    open fun setupRecyclerView() {}
    open fun loadItems() {}
}

// Level 5: Actual feature activity - WAY TOO DEEP!
class UserListActivity : BaseListActivity() {
    override fun loadItems() {
        // Load users
    }
}

// ANTI-PATTERN: Deep inheritance for Fragments
open class BaseFragment {
    open fun showProgress() {}
}

open class BaseViewModelFragment : BaseFragment() {
    open fun createViewModel() {}
}

open class BaseDialogFragment : BaseViewModelFragment() {
    open fun showDialog() {}
}

class ConfirmationDialogFragment : BaseDialogFragment() {
    fun confirm() {}
}

// ANTI-PATTERN: Deep inheritance for ViewModels
open class BaseViewModel {
    open fun handleError(error: Throwable) {}
}

open class BaseLoadingViewModel : BaseViewModel() {
    var isLoading: Boolean = false
}

open class BaseNavigationViewModel : BaseLoadingViewModel() {
    fun navigate(destination: String) {}
}

open class BaseFormViewModel : BaseNavigationViewModel() {
    fun validate(): Boolean = true
}

class UserFormViewModel : BaseFormViewModel() {
    fun submitUser() {}
}

// BETTER: Composition over inheritance
interface LoadingHandler {
    fun showLoading()
    fun hideLoading()
}

interface ErrorHandler {
    fun showError(message: String)
}

interface ToolbarHandler {
    fun setupToolbar()
    fun setTitle(title: String)
}

// Simple activity using composition
class SimpleActivity(
    private val loadingHandler: LoadingHandler,
    private val errorHandler: ErrorHandler
) {
    fun doWork() {
        loadingHandler.showLoading()
        try {
            // work
        } catch (e: Exception) {
            errorHandler.showError(e.message ?: "Error")
        } finally {
            loadingHandler.hideLoading()
        }
    }
}

// OK: Single level of inheritance
open class BaseFeatureActivity {
    fun commonSetup() {}
}

class FeatureActivity : BaseFeatureActivity() {
    fun featureSpecificSetup() {}
}

fun main() {
    val activity = UserListActivity()
    activity.setupToolbar()
    activity.loadItems()
}
