# Detectors reference

The 9 detection types implemented in SearchDeadCode, with code examples and the flag that enables each.

## 1. Unused classes / interfaces

Classes or interfaces that are never instantiated, extended, or referenced.

```kotlin
// DEAD: Never used anywhere
class OrphanHelper {
    fun doSomething() {}
}
```

## 2. Unused methods / functions

Methods that are never called, including extension functions.

```kotlin
class UserService {
    fun getUser(id: String) = // used

    // DEAD: Never called
    fun legacyGetUser(id: Int) = // ...
}

// Extension functions are also detected
fun String.deadExtension(): String = this  // DEAD: Never called
```

## 3. Unused properties / fields

Properties declared but never read.

```kotlin
class Config {
    val apiUrl = "https://api.example.com"  // used
    val debugMode = true                     // DEAD: never read
}
```

## 4. Assign-only properties

Properties that are written to but never read.

```kotlin
class Analytics {
    var lastEventTime: Long = 0  // DEAD: assigned but never read

    fun track(event: Event) {
        lastEventTime = System.currentTimeMillis()  // write-only
        send(event)
    }
}
```

Enable with `--write-only` and `--write-only-prefs` (SharedPreferences variant), `--write-only-dao` (DAO `@Insert` without `@Query`).

## 5. Unused parameters

Function parameters never used in the body. Enable with `--unused-params`.

```kotlin
// DEAD: 'context' parameter never used
fun formatDate(date: Date, context: Context): String {
    return SimpleDateFormat("yyyy-MM-dd").format(date)
}
```

Conservative by design: skips underscore-prefixed (`_unused`), override methods, abstract / interface methods, `@Composable` functions, constructors, and callback patterns (`onXxx`, `*Listener`, `*Callback`).

## 6. Unused imports

Import statements with no corresponding usage.

```kotlin
import com.example.utils.StringUtils  // DEAD: never used
import com.example.models.User        // used

class UserProfile {
    fun display(user: User) {}
}
```

## 7. Unused enum cases

Individual enum values that are never referenced.

```kotlin
enum class Status {
    ACTIVE,     // used
    INACTIVE,   // used
    LEGACY,     // DEAD: never referenced
    DEPRECATED  // DEAD: never referenced
}
```

Sealed class variant detection: enable with `--sealed-variants`.

## 8. Redundant public modifiers

Public declarations only used within the same module.

```kotlin
// Could be internal/private: only used within this module
public class InternalHelper {
    public fun process() {}
}
```

## 9. Dead branches

Code paths that can never execute.

```kotlin
fun process(value: Int) {
    if (value > 0) {
        // reachable
    } else if (value <= 0) {
        // reachable
    } else {
        // DEAD: impossible to reach
        handleImpossible()
    }
}
```

## Unused Android resources

Strings, colors, dimens, styles, attrs declared in `res/values/*.xml` but never referenced. Enable with `--unused-resources`.

```bash
$ searchdeadcode ./my-app --unused-resources

📦 Unused Android Resources:
  ○ app/src/main/res/values/strings.xml:21 - string 'unused_feature_text'
  ○ app/src/main/res/values/colors.xml:12 - color 'deprecated_accent'
  ○ app/src/main/res/values/styles.xml:15 - style 'LegacyButton'

Found 53 unused resources (672 total defined, 1142 referenced)
```

Common false positives to filter via `exclude` patterns: `com_braze_*`, `google_*` (read via reflection), theme attributes referenced by parent themes, build-variant resources.

## Zombie code (cycle detection)

Mutually dependent dead code: A uses B, B uses A, neither used elsewhere. Enable with `--detect-cycles`.

```
🧟 Zombie Code Detected:
  2 dead cycles found (15 declarations)
  Largest cycle: 8 mutually dependent declarations
  3 zombie pairs (A↔B mutual references)
```

## Override methods that only call super

Detected with `--redundant-overrides`.

```kotlin
override fun onCreateView(...): View {
    return super.onCreateView(inflater, container, savedInstanceState)
    // DEAD: If this is all it does, the override is unnecessary
}
```

## Unused intent extras

`putExtra("key", value)` where `"key"` is never read with `getExtra`. Enable with `--unused-extras`.

## Confidence levels

Each finding gets a confidence level:

| Level | Indicator | Description |
|---|---|---|
| Confirmed | ● green | Runtime coverage confirms code is never executed |
| High | ◉ bright green | Private / internal code with no static references |
| Medium | ○ yellow | Default for static-only analysis |
| Low | ◌ red | May be a false positive (reflection, dynamic dispatch) |

Filter with `--min-confidence` (`low`, `medium`, `high`, `confirmed`).

## Auto-retained Android entry points

The tool automatically retains (never reports as dead):

| Category | Patterns / Annotations |
|---|---|
| Lifecycle | `*Activity`, `*Fragment`, `*Service`, `*BroadcastReceiver`, `*ContentProvider`, `*Application` |
| Compose | `@Composable`, `@Preview` |
| ViewModels | `*ViewModel`, `@HiltViewModel` |
| Dependency Injection | `@Inject`, `@Provides`, `@Binds`, `@Module`, `@Component`, `@HiltAndroidApp`, `@AndroidEntryPoint`, `@AssistedInject` |
| Serialization | `@Serializable`, `@Parcelize`, `@JsonClass`, `@Entity`, `@SerializedName` |
| Data Binding | `@BindingAdapter`, `@InverseBindingAdapter`, `@BindingMethod` |
| Room | `@Dao`, `@Database`, `@Query`, `@Insert`, `@Update`, `@Delete`, `@RawQuery`, `@TypeConverter` |
| Retrofit | `@GET`, `@POST`, `@PUT`, `@DELETE`, `@PATCH`, `@HEAD`, `@OPTIONS`, `@HTTP`, `@Path`, `@Body` |
| Testing | `@Test`, `@Before`, `@After`, `@RunWith`, `@ParameterizedTest` |
| Reflection | `@JvmStatic`, `@JvmOverloads`, `@JvmField`, `@JvmName`, `@Keep` |
| WorkManager | `@HiltWorker` |
| Koin DI | `@Factory`, `@Single`, `@KoinViewModel` |
| Event Bus | `@Subscribe` |
| Coroutines | `suspend` functions in reachable classes, `@FlowPreview`, `@ExperimentalCoroutinesApi` |
| Entry functions | `main()` |

## XML parsing

The tool parses Android XML files for additional entry points:

**AndroidManifest.xml**
- `<activity android:name=".MainActivity">`
- `<service android:name=".MyService">`
- `<receiver>`, `<provider>`, `<application>` components

**Layout XMLs** (`res/layout/*.xml`)
- Custom views: `<com.example.CustomView>`
- Context references: `tools:context=".MyActivity"`
- Data binding: `app:viewModel="@{viewModel}"`

## Test code handling

Code that is **only** used in tests is reported as dead. Rationale: test-only utilities should live in test directories; production code should not exist solely for testing.

To exclude test files:
```yaml
exclude:
  - "**/test/**"
  - "**/androidTest/**"
  - "**/*Test.kt"
  - "**/*Spec.kt"
```
