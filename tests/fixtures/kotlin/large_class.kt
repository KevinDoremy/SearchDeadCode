package com.example.antipattern

// BAD: God class with too many responsibilities
class GodClass {
    // Too many properties (>20)
    var prop1: String = ""
    var prop2: String = ""
    var prop3: String = ""
    var prop4: String = ""
    var prop5: String = ""
    var prop6: String = ""
    var prop7: String = ""
    var prop8: String = ""
    var prop9: String = ""
    var prop10: String = ""
    var prop11: String = ""
    var prop12: String = ""
    var prop13: String = ""
    var prop14: String = ""
    var prop15: String = ""
    var prop16: String = ""
    var prop17: String = ""
    var prop18: String = ""
    var prop19: String = ""
    var prop20: String = ""
    var prop21: String = ""  // Exceeds threshold!

    // Too many methods (>30)
    fun method1() {}
    fun method2() {}
    fun method3() {}
    fun method4() {}
    fun method5() {}
    fun method6() {}
    fun method7() {}
    fun method8() {}
    fun method9() {}
    fun method10() {}
    fun method11() {}
    fun method12() {}
    fun method13() {}
    fun method14() {}
    fun method15() {}
    fun method16() {}
    fun method17() {}
    fun method18() {}
    fun method19() {}
    fun method20() {}
    fun method21() {}
    fun method22() {}
    fun method23() {}
    fun method24() {}
    fun method25() {}
    fun method26() {}
    fun method27() {}
    fun method28() {}
    fun method29() {}
    fun method30() {}
    fun method31() {}  // Exceeds threshold!

    // Mixed responsibilities - should be separate classes
    fun loadUser() {}
    fun saveUser() {}
    fun validateUser() {}
    fun formatUser() {}
    fun sendEmail() {}
    fun generateReport() {}
    fun calculateTax() {}
    fun processPayment() {}
    fun updateInventory() {}
    fun logActivity() {}
}

// GOOD: Small, focused class
class UserRepository {
    fun load(id: String): User? = null
    fun save(user: User) {}
    fun delete(id: String) {}
}

// GOOD: Single responsibility
class EmailService {
    fun send(to: String, subject: String, body: String) {}
    fun sendBulk(recipients: List<String>, subject: String, body: String) {}
}

// GOOD: Single responsibility
class TaxCalculator {
    fun calculate(amount: Double, rate: Double): Double = amount * rate
    fun calculateWithDeductions(amount: Double, rate: Double, deductions: Double): Double {
        return (amount - deductions) * rate
    }
}

data class User(val id: String, val name: String)
