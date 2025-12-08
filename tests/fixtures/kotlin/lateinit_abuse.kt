package com.example.antipattern

import android.os.Bundle
import android.view.View
import javax.inject.Inject

// BAD: lateinit abuse examples
class LateinitAbuse {

    // BAD: lateinit on a type that could be nullable
    lateinit var optionalUser: User  // Should be User? with null init

    // BAD: lateinit on primitives (won't compile, but pattern shows intent)
    // lateinit var count: Int  // Can't use lateinit on primitives

    // BAD: lateinit when lazy would be better
    lateinit var heavyObject: HeavyObject  // Could use lazy { }

    // BAD: lateinit for views that might not exist
    lateinit var optionalButton: Button  // View might not be in all layouts

    // GOOD: Using lazy for heavy initialization
    val lazyHeavyObject: HeavyObject by lazy {
        HeavyObject()
    }

    // GOOD: Nullable for truly optional
    var nullableUser: User? = null

    // OK: lateinit for dependency injection (common pattern)
    @Inject
    lateinit var repository: UserRepository

    // OK: lateinit in Activity for views bound in onCreate
    lateinit var binding: ActivityBinding

    fun onCreate() {
        binding = ActivityBinding.inflate(layoutInflater)
    }
}

// BAD: Class with many lateinit properties
class TooManyLateinit {
    lateinit var a: String
    lateinit var b: String
    lateinit var c: String
    lateinit var d: String
    lateinit var e: String  // 5+ lateinit is a smell

    fun init() {
        a = "a"
        b = "b"
        c = "c"
        d = "d"
        e = "e"
    }
}

// GOOD: Using constructor injection instead
class ProperInjection(
    val a: String,
    val b: String,
    val c: String,
    val d: String,
    val e: String
)
