// Test fixture: Prefer isEmpty() over size/length comparisons
package com.example.fixtures.preferisempty

// SHOULD USE isEmpty()
class ShouldUseIsEmpty {
    fun checkListSize(list: List<String>): Boolean {
        return list.size == 0  // PREFER: list.isEmpty()
    }

    fun checkListNotEmpty(list: List<String>): Boolean {
        return list.size != 0  // PREFER: list.isNotEmpty()
    }

    fun checkListGreaterZero(list: List<String>): Boolean {
        return list.size > 0  // PREFER: list.isNotEmpty()
    }

    fun checkMapSize(map: Map<String, Int>): Boolean {
        return map.size == 0  // PREFER: map.isEmpty()
    }

    fun checkSetSize(set: Set<Int>): Boolean {
        return set.size == 0  // PREFER: set.isEmpty()
    }

    fun checkStringLength(s: String): Boolean {
        return s.length == 0  // PREFER: s.isEmpty()
    }

    fun checkStringNotEmpty(s: String): Boolean {
        return s.length != 0  // PREFER: s.isNotEmpty()
    }

    fun checkArraySize(arr: Array<Int>): Boolean {
        return arr.size == 0  // PREFER: arr.isEmpty()
    }
}

// SHOULD USE isNotEmpty() (opposite condition)
class ShouldUseIsNotEmpty {
    fun checkHasItems(list: List<String>): Boolean {
        return list.size >= 1  // PREFER: list.isNotEmpty()
    }

    fun checkNotZero(list: List<String>): Boolean {
        return 0 != list.size  // PREFER: list.isNotEmpty()
    }

    fun checkPositive(list: List<String>): Boolean {
        return 0 < list.size  // PREFER: list.isNotEmpty()
    }
}

// ALREADY CORRECT - using isEmpty/isNotEmpty
class AlreadyCorrect {
    fun correctEmpty(list: List<String>): Boolean {
        return list.isEmpty()  // CORRECT
    }

    fun correctNotEmpty(list: List<String>): Boolean {
        return list.isNotEmpty()  // CORRECT
    }

    fun correctStringEmpty(s: String): Boolean {
        return s.isEmpty()  // CORRECT
    }

    fun correctStringNotEmpty(s: String): Boolean {
        return s.isNotEmpty()  // CORRECT
    }
}

// NOT APPLICABLE - comparison with non-zero values
class NotApplicable {
    fun checkExactSize(list: List<String>): Boolean {
        return list.size == 5  // NOT applicable: checking specific size
    }

    fun checkMinSize(list: List<String>): Boolean {
        return list.size >= 3  // NOT applicable: checking minimum size
    }

    fun checkMaxSize(list: List<String>): Boolean {
        return list.size <= 10  // NOT applicable: checking maximum size
    }

    fun compareTwo(list1: List<String>, list2: List<String>): Boolean {
        return list1.size == list2.size  // NOT applicable: comparing two sizes
    }
}

// EDGE CASES
class EdgeCases {
    fun nullableList(list: List<String>?): Boolean {
        return list?.size == 0  // Could use list?.isEmpty() == true
    }

    fun inCondition(list: List<String>) {
        if (list.size == 0) {  // PREFER: if (list.isEmpty())
            println("empty")
        }
    }

    fun inWhen(list: List<String>): String {
        return when {
            list.size == 0 -> "empty"  // PREFER: list.isEmpty()
            list.size == 1 -> "single"
            else -> "multiple"
        }
    }

    fun withNegation(list: List<String>): Boolean {
        return !(list.size == 0)  // PREFER: list.isNotEmpty()
    }
}

// CHAINED OPERATIONS
class ChainedOperations {
    fun filterThenCheck(list: List<Int>): Boolean {
        return list.filter { it > 0 }.size == 0  // PREFER: .isEmpty()
    }

    fun mapThenCheck(list: List<Int>): Boolean {
        return list.map { it * 2 }.size > 0  // PREFER: .isNotEmpty()
    }
}

fun main() {
    val s = ShouldUseIsEmpty()
    println(s.checkListSize(emptyList()))
    println(s.checkStringLength(""))

    val n = ShouldUseIsNotEmpty()
    println(n.checkHasItems(listOf("a")))

    val c = AlreadyCorrect()
    println(c.correctEmpty(emptyList()))

    val na = NotApplicable()
    println(na.checkExactSize(listOf("1","2","3","4","5")))

    val e = EdgeCases()
    e.inCondition(emptyList())
    println(e.inWhen(listOf("a")))

    val ch = ChainedOperations()
    println(ch.filterThenCheck(listOf(1, 2, 3)))
}
