// Test fixture: Excessive Feature Toggles Anti-Pattern
// Anti-pattern #4 from "8 anti-patterns in Android codebase"
package com.example.fixtures.antipatterns

// ANTI-PATTERN: Too many feature toggles
object FeatureFlags {
    var enableNewUI: Boolean = false
    var enableDarkMode: Boolean = false
    var enableNewCheckout: Boolean = false
    var enableNewSearch: Boolean = false
    var enableNewProfile: Boolean = false
    var enableNewSettings: Boolean = false
    var enableNewHome: Boolean = false
    var enableNewCart: Boolean = false
    var enableNewPayment: Boolean = false
    var enableNewNotifications: Boolean = false
    var enableABTestA: Boolean = false
    var enableABTestB: Boolean = false
    var enableABTestC: Boolean = false
    // More than 10 feature toggles - CODE SMELL!
}

// ANTI-PATTERN: Nested feature toggles (very hard to test)
class NestedToggleActivity {
    fun showContent() {
        if (FeatureFlags.enableNewUI) {
            if (FeatureFlags.enableDarkMode) {
                if (FeatureFlags.enableNewHome) {
                    // 3 levels of nesting - nightmare to test!
                    showNewDarkHome()
                } else {
                    showNewDarkOldHome()
                }
            } else {
                if (FeatureFlags.enableNewHome) {
                    showNewLightHome()
                } else {
                    showNewLightOldHome()
                }
            }
        } else {
            if (FeatureFlags.enableDarkMode) {
                showOldDarkHome()
            } else {
                showOldLightHome()
            }
        }
        // This creates 2^3 = 8 different paths to test!
    }

    private fun showNewDarkHome() {}
    private fun showNewDarkOldHome() {}
    private fun showNewLightHome() {}
    private fun showNewLightOldHome() {}
    private fun showOldDarkHome() {}
    private fun showOldLightHome() {}
}

// ANTI-PATTERN: Feature toggle in every method
class ToggleEverywhere {
    fun loadData() {
        if (FeatureFlags.enableNewSearch) {
            loadNewSearchData()
        } else {
            loadOldSearchData()
        }
    }

    fun displayResults() {
        if (FeatureFlags.enableNewSearch) {
            displayNewResults()
        } else {
            displayOldResults()
        }
    }

    fun handleClick() {
        if (FeatureFlags.enableNewSearch) {
            handleNewClick()
        } else {
            handleOldClick()
        }
    }

    fun trackAnalytics() {
        if (FeatureFlags.enableNewSearch) {
            trackNewAnalytics()
        } else {
            trackOldAnalytics()
        }
    }

    // Duplicated logic everywhere!
    private fun loadNewSearchData() {}
    private fun loadOldSearchData() {}
    private fun displayNewResults() {}
    private fun displayOldResults() {}
    private fun handleNewClick() {}
    private fun handleOldClick() {}
    private fun trackNewAnalytics() {}
    private fun trackOldAnalytics() {}
}

// ANTI-PATTERN: Copy-paste for feature toggles
class OldCheckoutFragment {
    // Old implementation (should be removed when toggle is removed)
    fun processPayment() {}
}

class NewCheckoutFragment {
    // New implementation
    fun processPayment() {}
}
// Both exist in codebase - which one is used? Check feature toggle!

// BETTER: Use strategy pattern
interface SearchStrategy {
    fun loadData()
    fun displayResults()
    fun handleClick()
}

class OldSearchStrategy : SearchStrategy {
    override fun loadData() { /* old impl */ }
    override fun displayResults() { /* old impl */ }
    override fun handleClick() { /* old impl */ }
}

class NewSearchStrategy : SearchStrategy {
    override fun loadData() { /* new impl */ }
    override fun displayResults() { /* new impl */ }
    override fun handleClick() { /* new impl */ }
}

class SearchController(
    private val strategy: SearchStrategy  // Injected based on feature flag
) {
    fun loadData() = strategy.loadData()
    fun displayResults() = strategy.displayResults()
    fun handleClick() = strategy.handleClick()
}

// BETTER: Factory based on feature flag (single place)
object SearchFactory {
    fun createStrategy(): SearchStrategy {
        return if (FeatureFlags.enableNewSearch) {
            NewSearchStrategy()
        } else {
            OldSearchStrategy()
        }
    }
}

// BETTER: Keep feature toggles minimal
object MinimalFeatureFlags {
    // Only essential, time-limited toggles
    var enableBetaFeature: Boolean = false  // Has removal date!
    var enableMaintenanceMode: Boolean = false  // Operational toggle
}

fun main() {
    val activity = NestedToggleActivity()
    activity.showContent()

    val strategy = SearchFactory.createStrategy()
    val controller = SearchController(strategy)
    controller.loadData()
}
