package com.example

class Sample {
    fun used(): Int = helper() + 1

    private fun helper(): Int = 42

    // Never called from anywhere in this workspace.
    private fun orphanedHelper(): String = "nobody calls me"
}

fun main() {
    println(Sample().used())
}
