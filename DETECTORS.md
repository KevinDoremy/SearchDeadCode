# SearchDeadCode Detectors

This document provides a comprehensive reference for all detectors available in SearchDeadCode.

## Overview

SearchDeadCode includes **50 detectors** organized into two categories:

- **Dead Code Detectors (DC001-DC016)**: Find unused, unreachable, or redundant code
- **Anti-Pattern Detectors (AP001-AP034)**: Find code smells and architectural issues

## Quick Start

```bash
# Run all anti-pattern detectors
searchdeadcode --anti-patterns /path/to/project

# Run specific detector groups
searchdeadcode --kotlin-patterns /path/to/project
searchdeadcode --android-patterns /path/to/project
searchdeadcode --compose-patterns /path/to/project
searchdeadcode --performance-patterns /path/to/project
searchdeadcode --architecture-patterns /path/to/project

# Combine with dead code detection
searchdeadcode --deep --anti-patterns /path/to/project
```

---

## Dead Code Detectors (DC001-DC016)

### DC001: Unreferenced Declaration
**Severity**: Warning | **Confidence**: Medium

Finds classes, functions, and properties that are never referenced anywhere in the codebase.

```kotlin
// BAD: Never used anywhere
class UnusedHelper {
    fun helperMethod() { }
}
```

**CLI**: Enabled by default with `--deep` mode

---

### DC002: Assign-Only Variable
**Severity**: Warning | **Confidence**: Medium

Finds variables that are assigned but never read.

```kotlin
// BAD: counter is written but never read
var counter = 0
counter++
counter = 10
// counter value is never used
```

**CLI**: `--write-only`

---

### DC003: Unused Parameter
**Severity**: Info | **Confidence**: Medium

Finds function parameters that are declared but never used in the function body.

```kotlin
// BAD: 'context' parameter is never used
fun processData(data: String, context: Context): String {
    return data.uppercase()
}
```

**CLI**: `--unused-params`

---

### DC004: Unused Import
**Severity**: Info | **Confidence**: High

Finds import statements that don't correspond to any used symbols.

```kotlin
import java.util.ArrayList  // BAD: ArrayList is never used
import java.util.List

fun getData(): List<String> = listOf()
```

**CLI**: Enabled by default

---

### DC005: Unused Enum Case
**Severity**: Warning | **Confidence**: Medium

Finds enum values that are never referenced.

```kotlin
enum class Status {
    ACTIVE,
    INACTIVE,
    PENDING,  // BAD: Never used in when expressions or comparisons
}
```

**CLI**: Enabled by default

---

### DC006: Redundant Public
**Severity**: Info | **Confidence**: Low

Finds public declarations that are only used within the same module.

```kotlin
// BAD: Could be internal or private
public class InternalHelper {
    public fun help() { }  // Only called from same module
}
```

**CLI**: Enabled by default

---

### DC007: Dead Branch
**Severity**: Warning | **Confidence**: High

Finds code branches that can never be executed.

```kotlin
fun process(value: Int) {
    if (value > 10 && value < 5) {  // BAD: Impossible condition
        // This code can never run
    }
}
```

**CLI**: Enabled by default

---

### DC008: Unused Sealed Variant
**Severity**: Warning | **Confidence**: High

Finds sealed class/interface variants that are never instantiated.

```kotlin
sealed class Result {
    data class Success(val data: String) : Result()
    data class Error(val message: String) : Result()
    object Cancelled : Result()  // BAD: Never instantiated anywhere
}
```

**CLI**: `--sealed-variants`

---

### DC009: Redundant Override
**Severity**: Info | **Confidence**: Medium

Finds method overrides that only call the super implementation without adding behavior.

```kotlin
// BAD: Just delegates to super
override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
}
```

**CLI**: `--redundant-overrides`

---

### DC010: Write-Only Preference
**Severity**: Warning | **Confidence**: High

Finds SharedPreferences keys that are written but never read.

```kotlin
// BAD: "last_sync" is written but never read
prefs.edit().putLong("last_sync", System.currentTimeMillis()).apply()
// No getString("last_sync", ...) or getLong("last_sync", ...) anywhere
```

**CLI**: `--write-only-prefs`

---

### DC011: Write-Only DAO
**Severity**: Warning | **Confidence**: High

Finds Room DAOs that have @Insert/@Update/@Delete but no @Query methods.

```kotlin
// BAD: Data is inserted but never queried
@Dao
interface LogDao {
    @Insert
    suspend fun insert(log: LogEntry)
    // No @Query methods - data is write-only
}
```

**CLI**: `--write-only-dao`

---

### DC012: Duplicate Import
**Severity**: Warning | **Confidence**: High

Finds the same import statement appearing multiple times.

```kotlin
import com.example.Utils
import com.example.Utils  // BAD: Duplicate
```

**CLI**: Enabled by default

---

### DC013: Redundant Null Init
**Severity**: Info | **Confidence**: High

Finds nullable variables explicitly initialized to null (the default).

```kotlin
// BAD: Explicit null is redundant for nullable types
var name: String? = null

// GOOD: Let Kotlin use default
var name: String? = null  // or just don't initialize
```

**CLI**: Enabled by default

---

### DC014: Redundant This
**Severity**: Info | **Confidence**: Medium

Finds unnecessary `this.` references where not needed for disambiguation.

```kotlin
class User(val name: String) {
    fun greet() {
        println(this.name)  // BAD: 'this.' is unnecessary
    }
}
```

**CLI**: Enabled by default

---

### DC015: Redundant Parentheses
**Severity**: Info | **Confidence**: High

Finds unnecessary parentheses around expressions.

```kotlin
val x = (1 + 2)  // BAD: Parentheses not needed
val y = ((a))    // BAD: Double parentheses
```

**CLI**: Enabled by default

---

### DC016: Prefer isEmpty
**Severity**: Info | **Confidence**: High

Finds `size == 0` or `length == 0` checks that should use `isEmpty()`.

```kotlin
// BAD
if (list.size == 0) { }
if (str.length == 0) { }

// GOOD
if (list.isEmpty()) { }
if (str.isEmpty()) { }
```

**CLI**: Enabled by default

---

## Anti-Pattern Detectors (AP001-AP034)

### Architecture Patterns (AP001-AP006)

Enable with: `--architecture-patterns` or `--anti-patterns`

---

#### AP001: Global Mutable State
**Severity**: Warning | **Confidence**: Medium

Finds Kotlin objects with mutable public properties.

```kotlin
// BAD: Global mutable state
object AppState {
    var currentUser: User? = null  // Mutable global state
    var isLoggedIn = false
}
```

**Better**: Use dependency injection or state management patterns.

---

#### AP002: Deep Inheritance
**Severity**: Warning | **Confidence**: Medium

Finds classes with inheritance depth > 3 levels (excluding framework classes).

```kotlin
// BAD: Too deep
class BaseA
class BaseB : BaseA()
class BaseC : BaseB()
class MyClass : BaseC()  // 4 levels deep
```

**Better**: Prefer composition over inheritance.

---

#### AP003: Single Implementation Interface
**Severity**: Info | **Confidence**: Low

Finds interfaces with only one implementation (unnecessary abstraction).

```kotlin
// BAD: Only one implementation exists
interface UserRepository { }
class UserRepositoryImpl : UserRepository { }  // The only impl
```

**Better**: Remove interface until you need multiple implementations.

---

#### AP004: EventBus Pattern
**Severity**: Warning | **Confidence**: High

Finds usage of EventBus or similar global event patterns.

```kotlin
// BAD: Global event bus
@Subscribe
fun onUserLoggedIn(event: LoginEvent) { }
```

**Better**: Use structured communication (callbacks, Flow, LiveData).

---

### Kotlin Patterns (AP007-AP010, AP021-AP025)

Enable with: `--kotlin-patterns` or `--anti-patterns`

---

#### AP007: Heavy ViewModel
**Severity**: Warning | **Confidence**: Medium

Finds ViewModels with >6 dependencies or direct data layer access.

```kotlin
// BAD: Too many dependencies
class UserViewModel(
    private val userRepo: UserRepository,
    private val orderRepo: OrderRepository,
    private val paymentRepo: PaymentRepository,
    private val notificationRepo: NotificationRepository,
    private val analyticsRepo: AnalyticsRepository,
    private val configRepo: ConfigRepository,
    private val featureFlagRepo: FeatureFlagRepository,  // 7+ deps
) : ViewModel()
```

**Better**: Split into smaller ViewModels or use UseCases.

---

#### AP008: GlobalScope Usage
**Severity**: Warning | **Confidence**: High

Finds `GlobalScope.launch` or `GlobalScope.async` usage.

```kotlin
// BAD: Memory leak risk
GlobalScope.launch {
    fetchData()
}
```

**Better**: Use `viewModelScope`, `lifecycleScope`, or custom scopes.

---

#### AP009: Lateinit Abuse
**Severity**: Info | **Confidence**: Low

Finds classes with >5 lateinit properties without @Inject.

```kotlin
// BAD: Too many lateinit properties
class MyFragment : Fragment() {
    lateinit var adapter: MyAdapter
    lateinit var viewModel: MyViewModel
    lateinit var binding: FragmentBinding
    lateinit var analytics: Analytics
    lateinit var navigator: Navigator
    lateinit var logger: Logger  // 6+ lateinit
}
```

**Better**: Use constructor injection or `lazy`.

---

#### AP010: Scope Function Chaining
**Severity**: Info | **Confidence**: Low

Finds excessive chaining of scope functions (let, apply, also, run, with).

```kotlin
// BAD: Hard to read
user?.let { it.name }
    ?.also { log(it) }
    ?.run { uppercase() }
    ?.let { validate(it) }
```

**Better**: Use intermediate variables for clarity.

---

#### AP021: Nullability Overload
**Severity**: Warning | **Confidence**: Low

Finds excessive `!!` force unwrap or redundant null checks.

```kotlin
// BAD: Excessive force unwrap
val name = user!!.profile!!.name!!
```

**Better**: Use safe calls, elvis operator, or restructure code.

---

#### AP022: Reflection Overuse
**Severity**: Info | **Confidence**: Low

Finds excessive Kotlin reflection in non-test code.

```kotlin
// BAD: Reflection in hot paths
for (prop in User::class.memberProperties) {
    println(prop.get(user))
}
```

**Better**: Use direct property access or compile-time alternatives.

---

#### AP023: Long Parameter List
**Severity**: Warning | **Confidence**: Medium

Finds functions with >6 parameters.

```kotlin
// BAD: Too many parameters
fun createUser(
    name: String,
    email: String,
    phone: String,
    address: String,
    city: String,
    country: String,
    zipCode: String  // 7+ params
)
```

**Better**: Use data class or builder pattern.

---

#### AP024: Complex Condition
**Severity**: Info | **Confidence**: Low

Finds conditions with >4 boolean operators.

```kotlin
// BAD: Hard to understand
if (a && b || c && !d || e && f) { }
```

**Better**: Extract to named booleans.

---

#### AP025: String Literal Duplication
**Severity**: Info | **Confidence**: Low

Finds repeated string literals that should be constants.

```kotlin
// BAD: Magic strings
intent.putExtra("user_id", userId)
// ... elsewhere ...
val userId = intent.getStringExtra("user_id")  // Duplicated
```

**Better**: Extract to companion object constant.

---

### Performance Patterns (AP011-AP015)

Enable with: `--performance-patterns` or `--anti-patterns`

---

#### AP011: Memory Leak Risk
**Severity**: Warning | **Confidence**: Medium

Finds static references to Context, Activity, View, or Fragment.

```kotlin
// BAD: Memory leak
object Cache {
    var activity: Activity? = null  // Leaks Activity
}

companion object {
    var context: Context? = null  // Leaks Context
}
```

**Better**: Use WeakReference or application context.

---

#### AP012: Long Method
**Severity**: Warning | **Confidence**: Medium

Finds methods exceeding 50 lines.

**Better**: Extract into smaller, focused methods.

---

#### AP013: Large Class
**Severity**: Warning | **Confidence**: Medium

Finds classes exceeding 500 lines or 30 methods.

**Better**: Split responsibilities into separate classes.

---

#### AP014: Collection Without Sequence
**Severity**: Info | **Confidence**: Low

Finds chained collection operations without `asSequence()`.

```kotlin
// BAD: Creates intermediate collections
list.filter { }.map { }.filter { }

// GOOD: Lazy evaluation
list.asSequence().filter { }.map { }.filter { }.toList()
```

---

#### AP015: Object Allocation in Loop
**Severity**: Warning | **Confidence**: Medium

Finds object creation inside loops or `onDraw()`.

```kotlin
// BAD: Allocates every iteration
for (item in items) {
    val formatter = SimpleDateFormat("yyyy-MM-dd")  // Allocates each loop
}
```

**Better**: Move allocation outside loop.

---

### Android Patterns (AP016-AP020, AP026-AP030)

Enable with: `--android-patterns` or `--anti-patterns`

---

#### AP016: Mutable State Exposed
**Severity**: Warning | **Confidence**: Medium

Finds public `MutableLiveData` or `MutableStateFlow`.

```kotlin
// BAD: Mutable state exposed
class MyViewModel : ViewModel() {
    val users = MutableLiveData<List<User>>()  // Public mutable!
}

// GOOD: Private backing field
class MyViewModel : ViewModel() {
    private val _users = MutableLiveData<List<User>>()
    val users: LiveData<List<User>> = _users
}
```

---

#### AP017: View Logic in ViewModel
**Severity**: Warning | **Confidence**: High

Finds View, Context, or Activity references in ViewModel.

```kotlin
// BAD: ViewModel holds View reference
class MyViewModel(
    private val context: Context  // Memory leak!
) : ViewModel()
```

**Better**: Use application context or remove View dependencies.

---

#### AP018: Missing UseCase
**Severity**: Info | **Confidence**: Low

Finds ViewModels using Repository directly without UseCase layer.

```kotlin
// Consider adding domain layer
class MyViewModel(
    private val userRepository: UserRepository,
    private val orderRepository: OrderRepository,
    private val paymentRepository: PaymentRepository  // Multiple repos = add UseCases
) : ViewModel()
```

---

#### AP019: Nested Callback
**Severity**: Warning | **Confidence**: Low

Finds deeply nested callbacks (callback hell).

```kotlin
// BAD: Callback hell
api.fetchUser { user ->
    api.fetchOrders(user.id) { orders ->
        api.fetchPayments(orders) { payments ->
            // Deep nesting
        }
    }
}
```

**Better**: Use coroutines or RxJava.

---

#### AP020: Hardcoded Dispatcher
**Severity**: Info | **Confidence**: Low

Finds hardcoded `Dispatchers.IO/Main/Default`.

```kotlin
// BAD: Hardcoded dispatcher
withContext(Dispatchers.IO) {
    fetchData()
}

// GOOD: Inject dispatcher
class MyRepository(private val ioDispatcher: CoroutineDispatcher) {
    suspend fun fetchData() = withContext(ioDispatcher) { }
}
```

---

#### AP026: Unclosed Resource
**Severity**: Warning | **Confidence**: Low

Finds Cursor, Stream, or other resources that may not be closed.

```kotlin
// BAD: Resource may leak
val stream = FileInputStream(file)
val data = stream.read()
// stream.close() missing!

// GOOD: Use .use {}
FileInputStream(file).use { stream ->
    stream.read()
}
```

---

#### AP027: Main Thread Database
**Severity**: Warning | **Confidence**: Medium

Finds database operations that may block the main thread.

```kotlin
// BAD: Blocking DAO method
@Dao
interface UserDao {
    @Query("SELECT * FROM users")
    fun getAllUsers(): List<User>  // Not suspend = blocks!
}

// GOOD: Suspend function
@Query("SELECT * FROM users")
suspend fun getAllUsers(): List<User>
```

---

#### AP028: WakeLock Abuse
**Severity**: Warning | **Confidence**: Low

Finds WakeLock that may not be properly released.

```kotlin
// BAD: May not release
wakeLock.acquire()
doLongOperation()
// release() missing or not in finally

// GOOD: Use timeout and finally
wakeLock.acquire(10 * 60 * 1000L)  // 10 min timeout
try {
    doLongOperation()
} finally {
    wakeLock.release()
}
```

---

#### AP029: AsyncTask Usage
**Severity**: Warning | **Confidence**: High

Finds deprecated AsyncTask usage.

```kotlin
// BAD: Deprecated in API 30
class LoadTask : AsyncTask<String, Int, List<User>>() {
    override fun doInBackground(vararg params: String) = fetchUsers()
}
```

**Better**: Use coroutines, Executor, or WorkManager.

---

#### AP030: Init in onDraw
**Severity**: Warning | **Confidence**: Medium

Finds object allocation in `onDraw()` methods.

```kotlin
// BAD: Allocates every frame
override fun onDraw(canvas: Canvas) {
    val paint = Paint()  // Called 60+ times/sec!
    canvas.drawRect(rect, paint)
}

// GOOD: Pre-allocate
private val paint = Paint()
override fun onDraw(canvas: Canvas) {
    canvas.drawRect(rect, paint)
}
```

---

### Compose Patterns (AP031-AP034)

Enable with: `--compose-patterns` or `--anti-patterns`

---

#### AP031: State Without Remember
**Severity**: Warning | **Confidence**: Low

Finds mutableStateOf without remember wrapper.

```kotlin
// BAD: State resets on recomposition
@Composable
fun Counter() {
    var count = mutableStateOf(0)  // No remember!
}

// GOOD: Remembered state
@Composable
fun Counter() {
    var count by remember { mutableStateOf(0) }
}
```

---

#### AP032: LaunchedEffect Without Key
**Severity**: Warning | **Confidence**: Medium

Finds LaunchedEffect with Unit key that should use parameters.

```kotlin
// BAD: Effect won't re-run when userId changes
@Composable
fun UserProfile(userId: String) {
    LaunchedEffect(Unit) {  // Should use userId as key
        user = fetchUser(userId)
    }
}

// GOOD: Effect re-runs when userId changes
@Composable
fun UserProfile(userId: String) {
    LaunchedEffect(userId) {
        user = fetchUser(userId)
    }
}
```

---

#### AP033: Business Logic in Composable
**Severity**: Warning | **Confidence**: Low

Finds data fetching or business logic in @Composable functions.

```kotlin
// BAD: Network call in composable
@Composable
fun UserScreen(userId: String) {
    LaunchedEffect(userId) {
        val response = retrofit.userService.getUser(userId)  // Bad!
    }
}

// GOOD: Logic in ViewModel
class UserViewModel : ViewModel() {
    fun loadUser(userId: String) {
        viewModelScope.launch {
            _user.value = userRepository.getUser(userId)
        }
    }
}
```

---

#### AP034: NavController Passing
**Severity**: Info | **Confidence**: Medium

Finds NavController passed to child composables.

```kotlin
// BAD: Tight coupling
@Composable
fun HomeScreen(navController: NavController) {
    ItemList(navController = navController)  // Passed down
}

// GOOD: Navigation callbacks
@Composable
fun HomeScreen(onNavigateToDetails: (String) -> Unit) {
    ItemList(onItemClick = onNavigateToDetails)
}
```

---

## Configuration

### YAML Configuration

Create `.deadcode.yml` in your project root:

```yaml
detection:
  unused_class: true
  unused_method: true
  unused_property: true
  anti_patterns:
    enabled: true
    architecture: true
    kotlin: true
    performance: true
    android: true
    compose: true

exclude:
  - "**/build/**"
  - "**/generated/**"
  - "**/*Test.kt"

retain_patterns:
  - "*Activity"
  - "*Fragment"
  - "*ViewModel"
```

### CLI Flags

| Flag | Description |
|------|-------------|
| `--anti-patterns` | Enable all anti-pattern detectors (AP001-AP034) |
| `--architecture-patterns` | Enable architecture patterns (AP001-AP006) |
| `--kotlin-patterns` | Enable Kotlin patterns (AP007-AP010, AP021-AP025) |
| `--performance-patterns` | Enable performance patterns (AP011-AP015) |
| `--android-patterns` | Enable Android patterns (AP016-AP020, AP026-AP030) |
| `--compose-patterns` | Enable Compose patterns (AP031-AP034) |
| `--unused-params` | Enable unused parameter detection |
| `--write-only` | Enable write-only variable detection |
| `--sealed-variants` | Enable unused sealed variant detection |
| `--redundant-overrides` | Enable redundant override detection |
| `--unused-resources` | Enable unused resource detection |
| `--unused-extras` | Enable unused Intent extra detection |
| `--write-only-prefs` | Enable write-only SharedPreferences detection |
| `--write-only-dao` | Enable write-only Room DAO detection |

---

## Detector Counts

| Category | Count | Codes |
|----------|-------|-------|
| Dead Code | 16 | DC001-DC016 |
| Architecture | 4 | AP001-AP004 |
| Kotlin (Phase 1) | 4 | AP007-AP010 |
| Performance | 5 | AP011-AP015 |
| Architecture/Design | 5 | AP016-AP020 |
| Kotlin (Phase 4) | 5 | AP021-AP025 |
| Android (Phase 5) | 5 | AP026-AP030 |
| Compose (Phase 6) | 4 | AP031-AP034 |
| **Total** | **50** | |
