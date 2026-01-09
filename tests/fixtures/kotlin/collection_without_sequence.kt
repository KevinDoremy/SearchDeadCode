// Test fixture for CollectionWithoutSequenceDetector (AP014)
// Detects chained collection operations without asSequence()

package com.example.antipattern

// BAD: Chained operations without sequence (inefficient for large collections)
class IneffectiveChaining {

    // Creates intermediate list for each operation
    fun processLargeList(items: List<Item>): List<String> {
        return items
            .filter { it.isActive }
            .map { it.name }
            .filter { it.isNotEmpty() }
            .map { it.uppercase() }
    }

    // Multiple intermediate collections
    fun transformData(data: List<Data>): List<Result> {
        return data
            .filter { it.isValid() }
            .map { transform(it) }
            .sortedBy { it.priority }
            .take(10)
    }

    // Especially bad with large datasets
    fun heavyProcessing(records: List<Record>): Int {
        return records
            .filter { it.year > 2020 }
            .map { it.value }
            .filter { it > 0 }
            .sum()
    }
}

// GOOD: Using asSequence() for lazy evaluation
class EffectiveChaining {

    // Lazy evaluation - single pass through data
    fun processLargeListEfficiently(items: List<Item>): List<String> {
        return items.asSequence()
            .filter { it.isActive }
            .map { it.name }
            .filter { it.isNotEmpty() }
            .map { it.uppercase() }
            .toList()
    }

    // Efficient with sequences
    fun transformDataEfficiently(data: List<Data>): List<Result> {
        return data.asSequence()
            .filter { it.isValid() }
            .map { transform(it) }
            .sortedBy { it.priority }
            .take(10)
            .toList()
    }
}

// OK: Single operation doesn't need sequence
class SingleOperations {

    // Single map is fine
    fun simpleMap(items: List<Item>): List<String> {
        return items.map { it.name }
    }

    // Single filter is fine
    fun simpleFilter(items: List<Item>): List<Item> {
        return items.filter { it.isActive }
    }
}

// OK: Already using sequence
class AlreadySequence {

    fun withSequence(items: List<Item>): List<String> {
        return items.asSequence()
            .filter { it.isActive }
            .map { it.name }
            .toList()
    }

    fun fromSequence(items: Sequence<Item>): List<String> {
        return items
            .filter { it.isActive }
            .map { it.name }
            .toList()
    }
}

// Supporting classes
data class Item(val name: String, val isActive: Boolean)
data class Data(val id: Int) {
    fun isValid() = id > 0
}
data class Result(val priority: Int)
data class Record(val year: Int, val value: Int)
