package com.elumine.searchdeadcode.findings

import com.elumine.searchdeadcode.sarif.SdcFinding
import com.elumine.searchdeadcode.sarif.SdcLevel
import com.intellij.testFramework.fixtures.BasePlatformTestCase

class SdcFindingsStoreTest : BasePlatformTestCase() {

    private fun finding(line: Int = 0) = SdcFinding(
        ruleId = "DC001",
        message = "class 'Orphan' is never used",
        level = SdcLevel.WARNING,
        uri = "src/A.kt",
        line = line,
        column = 0,
        fingerprint = "abc",
    )

    fun `test an edit drops the file's findings immediately`() {
        val psiFile = myFixture.configureByText("A.kt", "class Orphan {}\n")
        val path = psiFile.virtualFile.path
        val store = SdcFindingsStore.getInstance(project)
        store.replaceAll(mapOf(path to listOf(finding())), scannedAt = 1L)
        assertEquals(1, store.findingsFor(path).size)

        myFixture.type("x")

        assertEquals("a stale line number is worse than no marker", 0, store.findingsFor(path).size)
        assertEquals(1, store.summary().invalidatedFiles)
    }

    fun `test an edit elsewhere keeps other files' findings`() {
        val edited = myFixture.configureByText("Edited.kt", "class E {}\n")
        val untouchedPath = "/never/opened/B.kt"
        val store = SdcFindingsStore.getInstance(project)
        store.replaceAll(
            mapOf(
                edited.virtualFile.path to listOf(finding()),
                untouchedPath to listOf(finding(line = 3)),
            ),
            scannedAt = 1L,
        )

        myFixture.type("x")

        assertEquals(0, store.findingsFor(edited.virtualFile.path).size)
        assertEquals(1, store.findingsFor(untouchedPath).size)
    }

    fun `test contains revalidates a specific finding and remove drops just one`() {
        val psiFile = myFixture.configureByText("C.kt", "class C {}\n")
        val path = psiFile.virtualFile.path
        val store = SdcFindingsStore.getInstance(project)
        val first = finding(line = 0)
        val second = finding(line = 5)
        store.replaceAll(mapOf(path to listOf(first, second)), scannedAt = 1L)

        assertTrue(store.contains(path, first))
        store.remove(path, first)
        assertFalse(store.contains(path, first))
        assertTrue(store.contains(path, second))
        assertEquals(1, store.summary().totalFindings)
    }

    fun `test the topic fires on every mutation`() {
        var events = 0
        project.messageBus.connect(testRootDisposable).subscribe(
            SdcFindingsListener.TOPIC,
            object : SdcFindingsListener {
                override fun findingsChanged() {
                    events++
                }
            },
        )
        val store = SdcFindingsStore.getInstance(project)
        store.replaceAll(mapOf("/x/A.kt" to listOf(finding())), scannedAt = 1L)
        store.invalidate("/x/A.kt")
        store.clear() // already empty: no event
        assertEquals(2, events)
    }
}
