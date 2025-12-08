// Test fixture: Redundant parentheses patterns
package com.example.fixtures.redundantparens

// REDUNDANT: Double parentheses
class RedundantParens {
    fun doubleParens(x: Int): Boolean {
        return ((x > 0))  // REDUNDANT: double parens
    }

    fun tripleParens(x: Int): Boolean {
        return (((x > 0)))  // REDUNDANT: triple parens
    }

    fun ifCondition(x: Int) {
        if ((x > 0)) {  // REDUNDANT: parens around already-parenthesized condition
            println("positive")
        }
    }

    fun whenSubject(x: Int) {
        when ((x)) {  // REDUNDANT: parens around simple variable
            1 -> println("one")
            else -> println("other")
        }
    }
}

// REDUNDANT: Unnecessary parens around simple expressions
class SimpleExpressions {
    fun returnValue(): Int {
        return (42)  // REDUNDANT: parens around literal
    }

    fun returnVariable(x: Int): Int {
        return (x)  // REDUNDANT: parens around variable
    }

    fun assignment() {
        val x = (10)  // REDUNDANT: parens around literal
        val y = (x)   // REDUNDANT: parens around variable
        println("$x $y")
    }
}

// NOT REDUNDANT: Parens for operator precedence clarity
class OperatorPrecedence {
    fun complexMath(a: Int, b: Int, c: Int): Int {
        return (a + b) * c  // NOT redundant: changes precedence
    }

    fun bitwiseOps(x: Int, y: Int): Int {
        return (x and y) or (x xor y)  // NOT redundant: clarity
    }

    fun booleanOps(a: Boolean, b: Boolean, c: Boolean): Boolean {
        return (a && b) || c  // NOT redundant: precedence
    }
}

// NOT REDUNDANT: Parens for method chaining on expressions
class MethodChaining {
    fun chainOnCast(obj: Any): String {
        return (obj as String).uppercase()  // NOT redundant: cast needs parens
    }

    fun chainOnNullable(s: String?): Int {
        return (s ?: "default").length  // NOT redundant: elvis needs parens
    }

    fun chainOnArithmetic(x: Int): String {
        return (x + 1).toString()  // NOT redundant: arithmetic needs parens
    }
}

// REDUNDANT: Parens around already-atomic expressions
class AtomicExpressions {
    fun stringLiteral(): String {
        return ("hello")  // REDUNDANT
    }

    fun functionCall(): Int {
        return (listOf(1,2,3).size)  // REDUNDANT around whole expression
    }

    fun propertyAccess(s: String): Int {
        return (s.length)  // REDUNDANT
    }
}

// NOT REDUNDANT: Parens in lambdas
class LambdaParens {
    fun withLambda() {
        listOf(1, 2, 3).map { (it * 2) }  // Could be argued either way
    }

    fun destructuring() {
        mapOf("a" to 1).forEach { (key, value) ->  // NOT redundant: destructuring
            println("$key: $value")
        }
    }
}

// REDUNDANT: Parens around single when branch
class WhenBranches {
    fun whenReturn(x: Int): String {
        return when (x) {
            1 -> ("one")    // REDUNDANT
            2 -> ("two")    // REDUNDANT
            else -> ("other")  // REDUNDANT
        }
    }
}

// NOT REDUNDANT: Parens for negative numbers
class NegativeNumbers {
    fun negativeInRange() {
        val range = (-10)..10  // Parens around negative might be needed
        for (i in range) {
            println(i)
        }
    }
}

fun main() {
    val r = RedundantParens()
    println(r.doubleParens(5))
    r.ifCondition(3)
    r.whenSubject(1)

    val s = SimpleExpressions()
    println(s.returnValue())
    s.assignment()

    val o = OperatorPrecedence()
    println(o.complexMath(1, 2, 3))

    val m = MethodChaining()
    println(m.chainOnCast("test"))
    println(m.chainOnNullable(null))

    val a = AtomicExpressions()
    println(a.stringLiteral())

    val w = WhenBranches()
    println(w.whenReturn(1))
}
