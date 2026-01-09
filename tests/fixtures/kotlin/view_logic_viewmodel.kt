// Test fixture for ViewLogicInViewModelDetector (AP017)
// Detects View/Context references in ViewModel

package com.example.antipattern

import android.content.Context
import android.view.View
import android.widget.TextView
import android.widget.Button
import android.app.Activity
import android.app.Fragment
import androidx.lifecycle.ViewModel
import androidx.lifecycle.AndroidViewModel
import android.app.Application

// BAD: ViewModel holding View reference (memory leak!)
class BadViewModelWithView : ViewModel() {

    // BAD: Direct View reference
    private var textView: TextView? = null

    // BAD: View reference as property
    lateinit var button: Button

    fun setView(view: TextView) {
        this.textView = view  // Memory leak when Activity is destroyed
    }

    fun updateText(text: String) {
        textView?.text = text  // May crash or leak
    }
}

// BAD: ViewModel holding Context reference
class BadViewModelWithContext : ViewModel() {

    // BAD: Context reference (usually Activity context = leak)
    private var context: Context? = null

    // BAD: Activity reference
    private var activity: Activity? = null

    fun init(context: Context) {
        this.context = context  // Memory leak!
    }

    fun showToast(message: String) {
        // Using context in ViewModel - bad practice
    }
}

// BAD: ViewModel with Fragment reference
class BadViewModelWithFragment : ViewModel() {

    // BAD: Fragment reference
    private var fragment: Fragment? = null

    fun setFragment(fragment: Fragment) {
        this.fragment = fragment
    }
}

// GOOD: Use AndroidViewModel for Application context only
class GoodAndroidViewModel(application: Application) : AndroidViewModel(application) {

    // OK: Application context doesn't leak
    fun getAppContext(): Context = getApplication()

    fun loadData() {
        val context = getApplication<Application>()
        // Use application context for resources, etc.
    }
}

// GOOD: ViewModel without View/Context references
class GoodViewModel : ViewModel() {

    private val _userData = MutableLiveData<User>()
    val userData: LiveData<User> = _userData

    // No View, Context, Activity, or Fragment references
    fun loadUser(userId: String) {
        // Business logic only
        _userData.value = User(userId, "John")
    }
}

// GOOD: Pass data, not views
class GoodViewModelWithData : ViewModel() {

    fun processInput(text: String) {
        // Receive data, not View
    }

    fun handleClick(itemId: Int) {
        // Receive data, not Button
    }
}

// OK: Repository/UseCase dependencies are fine
class ViewModelWithProperDeps(
    private val userRepository: UserRepository,
    private val analyticsTracker: AnalyticsTracker
) : ViewModel() {

    fun loadUser(id: String) {
        // Use repository, not View
    }
}

// Supporting types
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData

data class User(val id: String, val name: String)
interface UserRepository
interface AnalyticsTracker
