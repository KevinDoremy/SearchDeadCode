package com.elumine.searchdeadcode.fixes

import org.junit.Assert.assertEquals
import org.junit.Test

class IgnoreCommentTest {

    @Test
    fun `keeps the declaration's indentation`() {
        assertEquals(
            "    // deadcode:ignore(kept for reflection)\n",
            IgnoreInlineFix.buildIgnoreComment("    fun helper() {}", "kept for reflection"),
        )
    }

    @Test
    fun `strips parentheses the CLI regex would choke on`() {
        // the CLI parses the reason with \(([^)]*)\)
        assertEquals(
            "// deadcode:ignore(used by JNI native side)\n",
            IgnoreInlineFix.buildIgnoreComment("val x = 1", "used by JNI (native side)"),
        )
    }

    @Test
    fun `flattens newlines and collapses whitespace`() {
        assertEquals(
            "// deadcode:ignore(line one line two)\n",
            IgnoreInlineFix.buildIgnoreComment("val x = 1", "line one\n\n  line two  "),
        )
    }
}
