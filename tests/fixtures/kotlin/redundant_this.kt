// Test fixture: Redundant this/self reference patterns
package com.example.fixtures.redundantthis

// REDUNDANT: this. when not needed for disambiguation
class RedundantThis {
    private var name: String = ""
    private var count: Int = 0

    fun setName(value: String) {
        this.name = value  // REDUNDANT: no shadowing, this. unnecessary
    }

    fun increment() {
        this.count++  // REDUNDANT: no shadowing
    }

    fun getName(): String {
        return this.name  // REDUNDANT: no shadowing
    }

    fun process() {
        this.increment()  // REDUNDANT: calling own method
        println(this.getName())  // REDUNDANT: calling own method
    }
}

// NOT REDUNDANT: this. needed for disambiguation
class RequiredThis {
    private var name: String = ""
    private var value: Int = 0

    fun setName(name: String) {  // Parameter shadows field
        this.name = name  // REQUIRED: disambiguates from parameter
    }

    fun setValue(value: Int) {  // Parameter shadows field
        this.value = value  // REQUIRED: disambiguates from parameter
    }

    fun getName(): String = name
    fun getValue(): Int = value
}

// NOT REDUNDANT: this for returning self
class BuilderPattern {
    private var name: String = ""
    private var age: Int = 0

    fun withName(name: String): BuilderPattern {
        this.name = name
        return this  // NOT redundant: returning self
    }

    fun withAge(age: Int): BuilderPattern {
        this.age = age
        return this  // NOT redundant: returning self
    }

    fun build(): String = "$name, $age"
}

// NOT REDUNDANT: this in extension function context
class ExtensionContext {
    fun String.process(): String {
        return this.uppercase()  // Refers to String receiver, might be intentional
    }

    fun test() {
        println("hello".process())
    }
}

// REDUNDANT: this in simple getters/setters
class SimpleAccessors {
    private var _value: Int = 0

    var value: Int
        get() = this._value  // REDUNDANT
        set(v) { this._value = v }  // Could be redundant depending on context
}

// NOT REDUNDANT: this@ in nested contexts
class NestedContext {
    private val name = "outer"

    fun process() {
        val inner = object {
            val name = "inner"

            fun printNames() {
                println(this.name)  // inner's name
                println(this@NestedContext.name)  // outer's name - NOT redundant
            }
        }
        inner.printNames()
    }
}

// REDUNDANT: this in chain calls
class ChainCalls {
    private var items = mutableListOf<String>()

    fun addItem(item: String) {
        this.items.add(item)  // REDUNDANT: no shadowing
    }

    fun clearItems() {
        this.items.clear()  // REDUNDANT: no shadowing
    }

    fun getItems(): List<String> = items
}

fun main() {
    val r = RedundantThis()
    r.setName("test")
    r.process()

    val req = RequiredThis()
    req.setName("name")
    req.setValue(42)
    println("${req.getName()} ${req.getValue()}")

    val builder = BuilderPattern()
        .withName("John")
        .withAge(30)
    println(builder.build())

    val ext = ExtensionContext()
    ext.test()

    val nested = NestedContext()
    nested.process()

    val chain = ChainCalls()
    chain.addItem("item1")
    println(chain.getItems())
}
