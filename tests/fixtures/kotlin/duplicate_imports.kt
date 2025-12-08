// Test fixture: Duplicate import patterns
package com.example.fixtures.duplicateimports

// DUPLICATE: Same import twice
import kotlin.collections.List
import kotlin.collections.List

// DUPLICATE: Same import multiple times
import kotlin.collections.Map
import kotlin.collections.Map
import kotlin.collections.Map

// NOT DUPLICATE: Different imports
import kotlin.collections.Set
import kotlin.collections.MutableSet

// DUPLICATE: Same class from same package
import android.view.View
import android.view.View

// NOT DUPLICATE: Same class name from different packages
import android.widget.TextView
import androidx.appcompat.widget.AppCompatTextView

// DUPLICATE: Wildcard imports
import java.util.*
import java.util.*

// NOT DUPLICATE: Different wildcard imports
import java.io.*
import java.net.*

class DuplicateImportsTest {
    fun useImports() {
        val list: List<String> = listOf()
        val map: Map<String, Int> = mapOf()
        val set: Set<Int> = setOf()
    }
}

fun main() {
    val test = DuplicateImportsTest()
    test.useImports()
}
