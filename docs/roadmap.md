# Roadmap

Forty advanced dead code patterns prioritized by detectability, frequency, and impact, plus the implementation phase plan.

## Patterns prioritized for future detectors

### Tier 1 — High impact, high detectability ⭐⭐⭐

Common patterns, easy to detect, significant waste.

| # | Pattern | Detectability | Frequency | Description |
|---|---|---|---|---|
| 1 | Write-only variables | High | 58+ occurrences | Variables assigned but never read |
| 2 | Unused sealed class variants | High | 73 sealed classes | Sealed cases never instantiated |
| 3 | Override methods that only call super | High | 284 overrides | `override fun onCreate() { super.onCreate() }` |
| 4 | Ignored return values | High | Common | `list.map { transform(it) }` without using the result |
| 5 | Empty catch blocks | High | Common | `catch (e: Exception) { }` |
| 6 | Unused intent extras | High | 90 putExtra calls | `putExtra("key", value)` where `"key"` is never read |
| 7 | Write-only SharedPreferences | High | Medium | `prefs.edit().putString("x", y)` where `"x"` is never read |
| 8 | Write-only database tables | High | 16 DAOs | `@Insert` without corresponding `@Query` usage |
| 9 | Redundant null checks | High | Common | `user?.let { if (it != null) }` |
| 10 | Dead feature flags | Medium | 388 isEnabled | Flag always true / false |

### Tier 2 — Medium impact, high detectability ⭐⭐

| # | Pattern | Detectability | Frequency | Description |
|---|---|---|---|---|
| 11 | Unobserved LiveData / StateFlow | Medium | 64 collectors | `_state.value = x` where `_state` never observed |
| 12 | Unused constructor parameters | High | Medium | Parameters passed but never used |
| 13 | Middle-man classes | Medium | Low | Classes that only delegate, no logic |
| 14 | Lazy classes | Medium | Low | Classes with minimal logic |
| 15 | Invariants always true / false | High | Common | `if (list.size >= 0)` |
| 16 | Cache write without read | Medium | Medium | `cache.save(data)` but always fetching from network |
| 17 | Analytics events never analyzed | Low | 253 log calls | Events tracked but no dashboard configured |
| 18 | Unused type parameters | High | Low | `class Foo<T>` where T is never used |
| 19 | Dead migrations | Medium | Low | Database migrations for versions no user has |
| 20 | Listeners never triggered | Medium | Medium | `view.setOnClickListener { }` on views that cannot be clicked |

### Tier 3 — High impact, lower detectability ⭐

High-value patterns that need more sophisticated analysis.

| # | Pattern | Detectability | Frequency | Description |
|---|---|---|---|---|
| 21 | Dormant code reactivated (Knight Capital Bug) | Low | Rare | Old code accidentally enabled by feature flags |
| 22 | Defensive copies never modified | Medium | Low | `val copy = list.toMutableList()` but copy never mutated |
| 23 | Calculations overwritten immediately | Medium | Low | `var x = expensiveCalc(); x = otherValue` |
| 24 | Partially dead code | Medium | Medium | Code only used on some branches but computed on all |
| 25 | Recalculation of available values | Medium | Low | `val h1 = data.hash(); ... val h2 = data.hash()` |
| 26 | Audit logs never queried | Low | Low | `auditDao.insert(log)` with no read methods |
| 27 | Breadcrumbs without consumer | Low | Low | Navigation history saved but never displayed |
| 28 | Event bus without subscribers | Medium | Low | `eventBus.post(event)` with no `@Subscribe` |
| 29 | Coroutines launched then cancelled | Low | Medium | Jobs cancelled before completing meaningful work |
| 30 | Workers that produce unused output | Low | Low | WorkManager jobs whose results are never consumed |

### Tier 4 — Specialized patterns ⭐

| # | Pattern | Detectability | Frequency | Description |
|---|---|---|---|---|
| 31 | Annotations without effect | Medium | Low | `@Keep` when ProGuard isn't configured to use it |
| 32 | Validation after the fact | Medium | Low | `db.insert(x); require(x.isValid)` |
| 33 | Unused debug logging | High | 253 Timber calls | Logs in production that output to nowhere |
| 34 | Semi-dead classes | Medium | Low | Classes used as types but never instantiated |
| 35 | Test-only code in production | High | Medium | Code only referenced by tests |
| 36 | Timestamps never used | Medium | Low | `updatedAt` field maintained but never queried |
| 37 | Serializable without serialization | Medium | Low | `@Serializable` on classes never serialized |
| 38 | Crashlytics keys never filtered | Low | Low | Custom keys set but never used in dashboard |
| 39 | Threads spawned without work | Low | Rare | Executor pools with empty task queues |
| 40 | Configuration values never read | Medium | Medium | Properties defined but never accessed |

## Implementation phases

### Phase 9 — Write-only detection ✅ (mostly complete)

Priority: ⭐⭐⭐⭐⭐ · Patterns: 1, 7, 8, 26 · Estimated dead code found: 15-25% increase.

- [x] Write-only variables (`--write-only`)
- [x] Write-only SharedPreferences (`--write-only-prefs`)
- [x] Write-only database tables (`--write-only-dao`)
- [ ] Write-only cache

### Phase 10 — Sealed class & override analysis ✅

Priority: ⭐⭐⭐⭐ · Patterns: 2, 3 · Estimated dead code found: 10-15% increase.

- [x] Unused sealed variants (`--sealed-variants`)
- [x] Redundant overrides (`--redundant-overrides`)

### Phase 11 — Intent & data flow ✅ (partial)

Priority: ⭐⭐⭐ · Patterns: 4, 6, 9 · Estimated dead code found: 5-10% increase.

- [ ] Ignored return values
- [x] Unused intent extras (`--unused-extras`)
- [ ] Redundant null checks

### Phase 12 — Observable state analysis

Priority: ⭐⭐ · Patterns: 10, 11, 16 · Estimated dead code found: 5-8% increase.

- [ ] Dead feature flags
- [ ] Unobserved StateFlow / LiveData
- [ ] Write-only cache

### Phase 13 — Advanced flow analysis

Priority: ⭐ · Patterns: 21-30 · Estimated dead code found: 2-5% increase.

- [ ] Partially dead code
- [ ] Recalculation detection
- [ ] Event bus orphans

### Phase 8 — Multi-platform (parallel track)

- [ ] iOS / Swift support
- [ ] React Native (native + JavaScript layers)
- [ ] Flutter / Dart
- [ ] KMP shared code analysis

### Memory + performance backlog

- [ ] Parallel graph construction (parallelize reference resolution phase)
- [ ] Memory optimization for very large codebases (100k+ files)
- [ ] LSP server (real-time dead code highlighting in editors)
- [ ] IntelliJ / Android Studio plugin

## Pattern detection examples

### Write-only variable (#1)

```kotlin
class Analytics {
    private var lastEventTime: Long = 0  // DEAD: never read

    fun track(event: Event) {
        lastEventTime = System.currentTimeMillis()  // write-only
        send(event)
    }
}
```

### Unused sealed variant (#2)

```kotlin
sealed class UiState {
    object Loading : UiState()                       // Used
    data class Success(val data: Data) : UiState()   // Used
    data class Error(val msg: String) : UiState()    // Used
    object Empty : UiState()                         // DEAD: never emitted
}
```

### Override only calling super (#3)

```kotlin
override fun onCreateView(...): View {
    return super.onCreateView(inflater, container, savedInstanceState)
    // DEAD: if this is all it does, the override is unnecessary
}
```

### Write-only database (#8)

```kotlin
@Dao
interface ReadHistoryDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun saveReadArticle(history: ReadHistory)  // Called

    @Query("SELECT * FROM read_history ORDER BY timestamp DESC")
    fun getReadHistory(): Flow<List<ReadHistory>>  // DEAD: never called
}
```

### Ignored return value (#4)

```kotlin
// DEAD: the sorted list is never used
articles.sortedByDescending { it.date }
adapter.submitList(articles)  // still the original unsorted list
```

### Dead feature flag (#10)

```kotlin
// The flag has been true for 2 years
if (RemoteConfig.isNewPlayerEnabled()) {  // always true
    playWithExoPlayer()
} else {
    playWithMediaPlayer()  // DEAD: never executed
}
```

## Real-world codebase analysis

From analysis of an Android project with 1 806 files:

| Pattern category | Occurrences | Potential dead code |
|---|---|---|
| Timber / Log calls | 253 | ~50% may be production-silent |
| Override methods | 284 | ~10-20% may only call super |
| Intent extras (putExtra) | 90 | ~30% may be unread |
| Sealed classes | 73 | ~5-10% may have unused variants |
| Feature flags | 388 | ~20% may be dead branches |
| Flow collectors | 64 | ~10% may be unobserved |
| Map operations | 72 | ~5% may have ignored results |
| Private vars | 58 | ~20% may be write-only |
| DAO @Insert methods | 16 | ~10% may be write-only tables |
| DAO @Query methods | 49 | (need cross-reference analysis) |

**Estimated additional dead code**: these advanced detectors could identify **30-50% more dead code** beyond current detection.

## Contributing to future development

Good first issues:

1. **Add new annotation support** — easy: add annotation names to `entry_points.rs`.
2. **Improve XML parsing** — medium: add support for more XML attributes.
3. **Write tests** — medium: add test cases for edge cases.
4. **Performance profiling** — advanced: identify and fix bottlenecks.
5. **LSP implementation** — advanced: implement language server protocol.

See [`CONTRIBUTING.md`](../CONTRIBUTING.md) for development setup and guidelines.
