package com.elumine.searchdeadcode.baseline

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class BaselineWriterTest {

    private val entry = BaselineWriter.Issue(
        file = "src/Sample.kt",
        name = "OrphanHelper",
        kind = "class",
        line = 8,
        rule = "DC001",
    )

    /**
     * Golden test: fixtures/cli-baseline.json was written by the real CLI
     * (`searchdeadcode --generate-baseline`). The writer must read it and
     * write the same shape back — same keys, epoch-string created_at,
     * explicit nulls.
     */
    @Test
    fun `round-trips a baseline the CLI generated`() {
        val cliBaseline = javaClass.getResourceAsStream("/fixtures/cli-baseline.json")!!
            .readBytes().toString(Charsets.UTF_8)

        val next = BaselineWriter.append(
            cliBaseline,
            BaselineWriter.Issue("src/Other.kt", "unusedFun", "function", 3, rule = "DC002"),
        )
        val root = Json.parseToJsonElement(next).jsonObject

        assertEquals(setOf("version", "created_at", "issues", "total_at_baseline"), root.keys)
        assertEquals(1, root["version"]!!.jsonPrimitive.content.toInt())
        // preserved from the CLI file, not restamped
        assertEquals("1786108836", root["created_at"]!!.jsonPrimitive.content)
        assertEquals(1, root["total_at_baseline"]!!.jsonPrimitive.content.toInt())

        val issues = root["issues"]!!.jsonArray.map { it.jsonObject }
        assertEquals(2, issues.size)
        // the CLI's own entry survives untouched, fqn value included
        assertEquals("sample.OrphanHelper", issues[0]["fqn"]!!.jsonPrimitive.content)
        // our appended entry emits explicit nulls, the shape serde writes
        assertEquals(setOf("file", "name", "kind", "line", "fqn", "rule"), issues[1].keys)
        assertEquals(JsonNull, issues[1]["fqn"])
        assertEquals("DC002", issues[1]["rule"]!!.jsonPrimitive.content)
    }

    @Test
    fun `creates the document when there is no baseline yet`() {
        val text = BaselineWriter.append(null, entry, now = "1700000000")
        val root = Json.parseToJsonElement(text).jsonObject
        assertEquals("1700000000", root["created_at"]!!.jsonPrimitive.content)
        assertEquals(1, root["issues"]!!.jsonArray.size)
        assertEquals(1, root["total_at_baseline"]!!.jsonPrimitive.content.toInt())
        assertTrue("the file ends with a newline", text.endsWith("\n"))
    }

    @Test
    fun `starts fresh rather than losing the action on an unreadable baseline`() {
        val text = BaselineWriter.append("{broken json", entry, now = "1700000000")
        val root = Json.parseToJsonElement(text).jsonObject
        assertEquals(1, root["issues"]!!.jsonArray.size)
    }

    @Test
    fun `deduplicates within the Rust matches() line drift tolerance`() {
        val once = BaselineWriter.append(null, entry, now = "1700000000")
        val drifted = entry.copy(line = entry.line + 10, rule = "DC013")
        val twice = BaselineWriter.append(once, drifted)
        assertEquals(
            1,
            Json.parseToJsonElement(twice).jsonObject["issues"]!!.jsonArray.size,
        )
    }

    @Test
    fun `a homonym past the drift tolerance is a second entry, not a duplicate`() {
        // matches() only suppresses within ±10 lines when fqn is absent: a
        // second dead `reset()` 200 lines away needs its own entry.
        val once = BaselineWriter.append(null, entry, now = "1700000000")
        val farHomonym = entry.copy(line = entry.line + 192)
        val twice = BaselineWriter.append(once, farHomonym)
        assertEquals(
            2,
            Json.parseToJsonElement(twice).jsonObject["issues"]!!.jsonArray.size,
        )
    }

    @Test
    fun `a different declaration is appended, counters preserved`() {
        val once = BaselineWriter.append(null, entry, now = "1700000000")
        val other = entry.copy(name = "OtherClass")
        val twice = BaselineWriter.append(once, other)
        val root = Json.parseToJsonElement(twice).jsonObject
        assertEquals(2, root["issues"]!!.jsonArray.size)
        // created_at and total_at_baseline describe the ORIGINAL baseline
        assertEquals("1700000000", root["created_at"]!!.jsonPrimitive.content)
        assertEquals(1, root["total_at_baseline"]!!.jsonPrimitive.content.toInt())
    }

    @Test
    fun `now is epoch seconds in a string, the CLI convention`() {
        val now = BaselineWriter.now()
        assertTrue(now.matches(Regex("\\d{10,}")))
    }
}
