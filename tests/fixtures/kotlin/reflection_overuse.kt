// Test fixture for ReflectionOveruseDetector (AP022)
// Detects excessive Kotlin reflection usage

package com.example.antipattern

import kotlin.reflect.KClass
import kotlin.reflect.KProperty
import kotlin.reflect.full.memberProperties
import kotlin.reflect.full.primaryConstructor

// BAD: Excessive reflection in hot paths
class ReflectionHeavyMapper {

    // BAD: Reflection in frequently called method
    fun <T : Any> mapToJson(obj: T): String {
        val kClass = obj::class
        val properties = kClass.memberProperties

        return buildString {
            append("{")
            properties.forEachIndexed { index, prop ->
                if (index > 0) append(",")
                append("\"${prop.name}\":")
                append("\"${prop.getter.call(obj)}\"")
            }
            append("}")
        }
    }

    // BAD: Creating instances via reflection
    fun <T : Any> createInstance(kClass: KClass<T>): T {
        val constructor = kClass.primaryConstructor
            ?: throw IllegalArgumentException("No primary constructor")
        return constructor.call()
    }

    // BAD: Property access via reflection in loop
    fun processAll(items: List<Any>) {
        items.forEach { item ->
            item::class.memberProperties.forEach { prop ->
                println("${prop.name} = ${prop.getter.call(item)}")
            }
        }
    }
}

// BAD: Reflection for simple operations
class UnnecessaryReflection {

    // BAD: Using reflection to get class name
    fun getClassName(obj: Any): String {
        return obj::class.simpleName ?: "Unknown"
        // Could just use: obj.javaClass.simpleName
    }

    // BAD: Checking type via reflection
    fun isString(obj: Any): Boolean {
        return obj::class == String::class
        // Could just use: obj is String
    }

    // BAD: Copying via reflection
    fun <T : Any> copyObject(obj: T): T {
        val kClass = obj::class
        val props = kClass.memberProperties.associate {
            it.name to it.getter.call(obj)
        }
        // Complex reflection copy...
        @Suppress("UNCHECKED_CAST")
        return obj as T  // Simplified for example
    }
}

// GOOD: Appropriate reflection usage
class AppropriateReflection {

    // GOOD: One-time initialization
    private val typeAdapters: Map<KClass<*>, TypeAdapter<*>> by lazy {
        mapOf(
            String::class to StringAdapter(),
            Int::class to IntAdapter()
        )
    }

    // GOOD: Framework/library code
    fun registerAdapter(kClass: KClass<*>, adapter: TypeAdapter<*>) {
        // Registration typically happens once
    }

    // GOOD: Serialization framework (called infrequently)
    fun serialize(obj: Any): ByteArray {
        // Frameworks like Gson/Moshi use reflection appropriately
        return ByteArray(0)
    }
}

// GOOD: Avoiding reflection
class NoReflectionAlternatives {

    // GOOD: Direct property access
    fun getUserName(user: User): String {
        return user.name  // Direct access, no reflection
    }

    // GOOD: Type check with 'is'
    fun processIfString(obj: Any) {
        if (obj is String) {
            println(obj.uppercase())
        }
    }

    // GOOD: Using data class copy
    fun copyUser(user: User): User {
        return user.copy()  // No reflection needed
    }

    // GOOD: Factory pattern instead of reflection
    fun createUser(name: String): User {
        return User(name)  // Direct constructor call
    }
}

// OK: Test code using reflection
class TestHelper {

    // OK: Reflection in tests is acceptable
    fun setPrivateField(obj: Any, fieldName: String, value: Any) {
        val field = obj::class.java.getDeclaredField(fieldName)
        field.isAccessible = true
        field.set(obj, value)
    }
}

// Supporting types
data class User(val name: String)
interface TypeAdapter<T>
class StringAdapter : TypeAdapter<String>
class IntAdapter : TypeAdapter<Int>
