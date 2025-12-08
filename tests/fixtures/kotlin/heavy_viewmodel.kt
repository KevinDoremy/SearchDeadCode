package com.example.antipattern

import androidx.lifecycle.ViewModel
import javax.inject.Inject

// BAD: ViewModel with too many dependencies (8+)
class HeavyUserViewModel @Inject constructor(
    private val userRepository: UserRepository,
    private val settingsRepository: SettingsRepository,
    private val analyticsManager: AnalyticsManager,
    private val notificationManager: NotificationManager,
    private val cacheManager: CacheManager,
    private val networkMonitor: NetworkMonitor,
    private val featureFlags: FeatureFlags,
    private val logger: Logger,
    private val errorHandler: ErrorHandler  // 9 dependencies!
) : ViewModel() {

    // Too many responsibilities in one ViewModel
    fun loadUser() { }
    fun updateSettings() { }
    fun trackEvent() { }
    fun showNotification() { }
    fun clearCache() { }
    fun checkNetwork() { }
    fun getFeature() { }
    fun log() { }
    fun handleError() { }
}

// GOOD: ViewModel with reasonable dependencies (3)
class SimpleUserViewModel @Inject constructor(
    private val userRepository: UserRepository,
    private val analyticsManager: AnalyticsManager,
    private val errorHandler: ErrorHandler
) : ViewModel() {

    fun loadUser() { }
    fun trackEvent() { }
}

// BAD: ViewModel with direct data source access
class DirectAccessViewModel @Inject constructor(
    private val database: AppDatabase,  // Direct database access!
    private val retrofit: Retrofit,      // Direct network access!
    private val sharedPrefs: SharedPreferences  // Direct prefs access!
) : ViewModel() {

    fun loadData() {
        // Directly accessing data layer - should use repository
    }
}

// GOOD: ViewModel using abstracted repository
class CleanViewModel @Inject constructor(
    private val repository: DataRepository
) : ViewModel() {

    fun loadData() {
        // Uses repository abstraction
    }
}
