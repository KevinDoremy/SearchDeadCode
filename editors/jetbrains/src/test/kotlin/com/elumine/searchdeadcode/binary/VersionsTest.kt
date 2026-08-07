package com.elumine.searchdeadcode.binary

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class VersionsTest {

    @Test
    fun `parses the CLI version banner`() {
        assertEquals(Triple(0, 19, 1), Versions.parse("searchdeadcode 0.19.1"))
        assertEquals(Triple(1, 2, 0), Versions.parse("1.2"))
        assertEquals(Triple(0, 0, 0), Versions.parse("no digits here"))
    }

    @Test
    fun `isAtLeast compares component-wise`() {
        assertTrue(Versions.isAtLeast("0.19.1", "0.10.0"))
        assertTrue(Versions.isAtLeast("0.10.0", "0.10.0"))
        assertTrue(Versions.isAtLeast("1.0.0", "0.99.99"))
        assertFalse(Versions.isAtLeast("0.9.9", "0.10.0"))
        assertFalse(Versions.isAtLeast("0.10.0", "0.10.1"))
    }
}
