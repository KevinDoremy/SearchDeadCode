package com.elumine.searchdeadcode.sarif

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/** Ported case for case from editors/vscode/test/SarifParser.test.ts. */
class SarifParserTest {

    private fun sample(): String =
        javaClass.getResourceAsStream("/fixtures/sample.sarif")!!
            .readBytes().toString(Charsets.UTF_8)

    @Test
    fun `reads the three well-formed results and drops the malformed one`() {
        val found = SarifParser.parse(sample())
        assertEquals(listOf("DC001", "DC004", "AP023"), found.map { it.ruleId })
    }

    @Test
    fun `converts SARIF 1-based positions to 0-based`() {
        val dc001 = SarifParser.parse(sample()).first()
        assertEquals(14, dc001.line)
        assertEquals(0, dc001.column)
    }

    @Test
    fun `defaults a missing startColumn to the start of the line`() {
        val dc004 = SarifParser.parse(sample()).first { it.ruleId == "DC004" }
        assertEquals(0, dc004.column)
    }

    @Test
    fun `falls back to the rule default level when the result omits it`() {
        val dc004 = SarifParser.parse(sample()).first { it.ruleId == "DC004" }
        assertEquals(SdcLevel.NOTE, dc004.level)
    }

    @Test
    fun `carries helpUri and fingerprint when present, null otherwise`() {
        val found = SarifParser.parse(sample())
        val dc001 = found[0]
        val dc004 = found[1]
        assertTrue(dc001.helpUri!!.contains("#dc001"))
        assertEquals("a1b2c3", dc001.fingerprint)
        assertNull(dc004.helpUri)
        assertNull(dc004.fingerprint)
    }

    @Test
    fun `reads a whole-line deletion fix as a 0-based inclusive range`() {
        val dc001 = SarifParser.parse(sample()).first()
        assertEquals(SdcFinding.DeletedRegion(14, 41), dc001.fix)
    }

    @Test
    fun `ignores a fix that inserts text instead of deleting`() {
        val doc = sample().replace("\"text\": \"\"", "\"text\": \"replacement\"")
        assertNull(SarifParser.parse(doc).first().fix)
    }

    @Test
    fun `applies the rule allowlist, empty keeps everything`() {
        val filtered = SarifParser.parse(sample(), listOf("DC001", "DC004"))
        assertEquals(listOf("DC001", "DC004"), filtered.map { it.ruleId })
        assertEquals(3, SarifParser.parse(sample(), emptyList()).size)
    }

    @Test
    fun `survives 010-era output with no rules, fingerprints or fixes`() {
        val legacy = """
            {"version":"2.1.0","runs":[{
              "tool":{"driver":{"name":"searchdeadcode","version":"0.10.0"}},
              "results":[{"ruleId":"DC001","level":"warning",
                "message":{"text":"class 'OrphanHelper' is never used"},
                "locations":[{"physicalLocation":{
                  "artifactLocation":{"uri":"p/src/Orphan.kt"},
                  "region":{"startLine":3,"startColumn":1}}}]}]}]}
        """.trimIndent()
        val finding = SarifParser.parse(legacy).single()
        assertEquals("DC001", finding.ruleId)
        assertEquals(2, finding.line)
        assertEquals(0, finding.column)
        assertEquals(SdcLevel.WARNING, finding.level)
        assertNull(finding.fix)
        assertNull(finding.helpUri)
    }

    @Test
    fun `rejects non-JSON, a non-21 version and a missing runs`() {
        assertThrows(SarifParser.SarifParseException::class.java) {
            SarifParser.parse("not json")
        }
        val badVersion = assertThrows(SarifParser.SarifParseException::class.java) {
            SarifParser.parse("""{"version":"3.0.0","runs":[]}""")
        }
        assertTrue(badVersion.message!!.contains("unsupported SARIF version"))
        val noRuns = assertThrows(SarifParser.SarifParseException::class.java) {
            SarifParser.parse("""{"version":"2.1.0"}""")
        }
        assertTrue(noRuns.message!!.contains("missing runs"))
        assertEquals(emptyList<SdcFinding>(), SarifParser.parse("""{"version":"2.1.0","runs":[]}"""))
    }

    @Test
    fun `never throws on structurally odd results`() {
        val odd = """
            {"version":"2.1.0","runs":[{"results":[null,42,{"ruleId":5},{"ruleId":"DC001","locations":[]}]}]}
        """.trimIndent()
        assertEquals(emptyList<SdcFinding>(), SarifParser.parse(odd))
    }

    // resolvePath — the triple fallback

    private fun existing(vararg paths: String): (String) -> Boolean = { it in paths }

    @Test
    fun `joins a plain relative path to the root`() {
        assertEquals(
            "/ws/proj/src/Main.kt",
            SarifParser.resolvePath("/ws/proj", "src/Main.kt", existing("/ws/proj/src/Main.kt")),
        )
    }

    @Test
    fun `strips the root basename when the CLI reports paths from the parent`() {
        // the real 0.10 behaviour: scanning /ws/proj reports "proj/src/Main.kt"
        assertEquals(
            "/ws/proj/src/Main.kt",
            SarifParser.resolvePath("/ws/proj", "proj/src/Main.kt", existing("/ws/proj/src/Main.kt")),
        )
    }

    @Test
    fun `prefers the direct join when both candidates exist`() {
        assertEquals(
            "/ws/proj/proj/src/Main.kt",
            SarifParser.resolvePath(
                "/ws/proj",
                "proj/src/Main.kt",
                existing("/ws/proj/proj/src/Main.kt", "/ws/proj/src/Main.kt"),
            ),
        )
    }

    @Test
    fun `returns absolute paths untouched`() {
        assertEquals(
            "/elsewhere/A.kt",
            SarifParser.resolvePath("/ws/proj", "/elsewhere/A.kt") { false },
        )
    }

    @Test
    fun `falls back to the direct join rather than dropping the finding`() {
        assertEquals(
            "/ws/proj/src/Gone.kt",
            SarifParser.resolvePath("/ws/proj", "src/Gone.kt") { false },
        )
    }

    @Test
    fun `tolerates a trailing slash on the root`() {
        assertEquals(
            "/ws/proj/src/Main.kt",
            SarifParser.resolvePath("/ws/proj/", "src/Main.kt", existing("/ws/proj/src/Main.kt")),
        )
    }

    @Test
    fun `normalizes Windows separators`() {
        assertEquals(
            "C:/ws/proj/src/Main.kt",
            SarifParser.resolvePath("C:\\ws\\proj", "src\\Main.kt", existing("C:/ws/proj/src/Main.kt")),
        )
    }

    // describe — never invent an entry

    @Test
    fun `describe recovers kind and name from the CLI message shape`() {
        val finding = SarifParser.parse(sample()).first()
        assertEquals("class" to "LegacyEncoder", SarifParser.describe(finding))
    }

    @Test
    fun `describe returns null when the message does not match`() {
        val finding = SdcFinding(
            ruleId = "DC001", message = "something unrecognizable",
            level = SdcLevel.WARNING, uri = "a.kt", line = 0, column = 0,
        )
        assertNull(SarifParser.describe(finding))
    }
}
