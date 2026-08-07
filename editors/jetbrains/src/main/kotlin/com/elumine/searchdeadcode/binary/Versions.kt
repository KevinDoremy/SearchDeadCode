package com.elumine.searchdeadcode.binary

/** `1.2.3` → (1, 2, 3); missing parts read as 0. Pure, unit tested. */
object Versions {
    fun parse(text: String): Triple<Int, Int, Int> {
        val m = Regex("""(\d+)\.(\d+)(?:\.(\d+))?""").find(text) ?: return Triple(0, 0, 0)
        return Triple(
            m.groupValues[1].toInt(),
            m.groupValues[2].toInt(),
            m.groupValues[3].ifEmpty { "0" }.toInt(),
        )
    }

    fun isAtLeast(version: String, minimum: String): Boolean {
        val a = parse(version)
        val b = parse(minimum)
        return a.toList().zip(b.toList()).firstOrNull { it.first != it.second }
            ?.let { it.first > it.second } != false
    }

    private fun Triple<Int, Int, Int>.toList() = listOf(first, second, third)
}
