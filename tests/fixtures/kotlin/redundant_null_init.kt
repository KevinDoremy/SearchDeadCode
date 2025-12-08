// Test fixture: Redundant null initialization patterns
package com.example.fixtures.redundantnull

// REDUNDANT: Explicit null init on nullable var
class RedundantNullInit {
    private var name: String? = null  // REDUNDANT: = null is default for nullable
    private var age: Int? = null      // REDUNDANT: = null is default for nullable

    fun setName(n: String) {
        name = n
    }

    fun setAge(a: Int) {
        age = a
    }
}

// NOT REDUNDANT: Non-null with actual value
class NonRedundantInit {
    private var name: String = ""       // NOT redundant: non-null needs init
    private var count: Int = 0          // NOT redundant: non-null needs init
    private var enabled: Boolean = true // NOT redundant: actual value

    fun process() {
        println("$name $count $enabled")
    }
}

// NOT REDUNDANT: Nullable with non-null initial value
class NullableWithValue {
    private var cache: String? = "default"  // NOT redundant: has actual value
    private var lastError: Exception? = RuntimeException()  // NOT redundant

    fun clear() {
        cache = null
        lastError = null
    }

    fun getCache(): String? = cache
}

// REDUNDANT: Local variable null init
class LocalVarNull {
    fun process() {
        var result: String? = null  // REDUNDANT in local scope too
        result = computeValue()
        println(result)
    }

    private fun computeValue(): String = "computed"
}

// NOT REDUNDANT: lateinit (no init allowed)
class LateinitExample {
    private lateinit var adapter: String  // lateinit - no init

    fun setup() {
        adapter = "initialized"
    }

    fun use(): String = adapter
}

// REDUNDANT: Nullable generic types
class GenericNull {
    private var items: List<String>? = null  // REDUNDANT
    private var callback: (() -> Unit)? = null  // REDUNDANT

    fun setItems(list: List<String>) {
        items = list
    }

    fun setCallback(cb: () -> Unit) {
        callback = cb
    }
}

fun main() {
    val r = RedundantNullInit()
    r.setName("test")

    val n = NonRedundantInit()
    n.process()

    val v = NullableWithValue()
    v.clear()
    println(v.getCache())

    val l = LocalVarNull()
    l.process()

    val e = LateinitExample()
    e.setup()
    println(e.use())

    val g = GenericNull()
    g.setItems(listOf())
    g.setCallback { }
}
