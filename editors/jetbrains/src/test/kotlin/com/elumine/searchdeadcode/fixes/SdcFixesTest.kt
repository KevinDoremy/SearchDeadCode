package com.elumine.searchdeadcode.fixes

import com.elumine.searchdeadcode.findings.SdcFindingsStore
import com.elumine.searchdeadcode.sarif.SdcFinding
import com.elumine.searchdeadcode.sarif.SdcLevel
import com.intellij.openapi.ui.TestDialog
import com.intellij.openapi.ui.TestDialogManager
import com.intellij.openapi.ui.TestInputDialog
import com.intellij.testFramework.fixtures.BasePlatformTestCase

class SdcFixesTest : BasePlatformTestCase() {

    override fun tearDown() {
        try {
            TestDialogManager.setTestInputDialog(TestInputDialog.DEFAULT)
        } finally {
            super.tearDown()
        }
    }

    private fun seeded(text: String, finding: SdcFinding): String {
        val psiFile = myFixture.configureByText("A.kt", text)
        val path = psiFile.virtualFile.path
        SdcFindingsStore.getInstance(project)
            .replaceAll(mapOf(path to listOf(finding)), scannedAt = 1L)
        return path
    }

    fun `test the delete fix removes exactly the whole-line region`() {
        val finding = SdcFinding(
            ruleId = "DC001",
            message = "class 'Orphan' is never used",
            level = SdcLevel.WARNING,
            uri = "A.kt",
            line = 1,
            column = 0,
            fix = SdcFinding.DeletedRegion(1, 2),
        )
        val path = seeded("class Kept {}\nclass Orphan {\n}\nclass Also {}\n", finding)

        val fix = DeleteDeadCodeFix(path, finding)
        assertEquals("Delete unused class 'Orphan'", fix.text)
        assertTrue(fix.isAvailable(project, myFixture.editor, myFixture.file))
        fix.invoke(project, myFixture.editor, myFixture.file)

        assertEquals("class Kept {}\nclass Also {}\n", myFixture.editor.document.text)
        // the deletion itself invalidated the file: the fix cannot fire twice
        assertFalse(fix.isAvailable(project, myFixture.editor, myFixture.file))
    }

    fun `test the delete fix falls back to a generic label`() {
        val finding = SdcFinding(
            ruleId = "DC001",
            message = "unrecognizable shape",
            level = SdcLevel.WARNING,
            uri = "A.kt",
            line = 0,
            column = 0,
            fix = SdcFinding.DeletedRegion(0, 0),
        )
        assertEquals("Delete unused declaration", DeleteDeadCodeFix("/x", finding).text)
    }

    fun `test the ignore fix inserts an indented reason comment above the line`() {
        val finding = SdcFinding(
            ruleId = "DC013",
            message = "function 'helper' is never used",
            level = SdcLevel.WARNING,
            uri = "A.kt",
            line = 1,
            column = 0,
        )
        val path = seeded("class A {\n    fun helper() {}\n}\n", finding)
        TestDialogManager.setTestInputDialog { "kept for (reflection)" }

        IgnoreInlineFix(path, finding).invoke(project, myFixture.editor, myFixture.file)

        assertEquals(
            "class A {\n    // deadcode:ignore(kept for reflection)\n    fun helper() {}\n}\n",
            myFixture.editor.document.text,
        )
    }

    fun `test the ignore fix does nothing when the dialog is cancelled`() {
        val finding = SdcFinding(
            ruleId = "DC013",
            message = "function 'helper' is never used",
            level = SdcLevel.WARNING,
            uri = "A.kt",
            line = 0,
            column = 0,
        )
        val path = seeded("fun helper() {}\n", finding)
        TestDialogManager.setTestInputDialog { null }

        IgnoreInlineFix(path, finding).invoke(project, myFixture.editor, myFixture.file)

        assertEquals("fun helper() {}\n", myFixture.editor.document.text)
    }

    fun `test a fix offered before an edit dies with its finding`() {
        val finding = SdcFinding(
            ruleId = "DC001",
            message = "class 'Orphan' is never used",
            level = SdcLevel.WARNING,
            uri = "A.kt",
            line = 0,
            column = 0,
            fix = SdcFinding.DeletedRegion(0, 0),
        )
        val path = seeded("class Orphan {}\n", finding)
        val fix = DeleteDeadCodeFix(path, finding)
        assertTrue(fix.isAvailable(project, myFixture.editor, myFixture.file))

        myFixture.type("x")

        assertFalse(
            "the store dropped the file, the fix must refuse to cut blind",
            fix.isAvailable(project, myFixture.editor, myFixture.file),
        )
    }
}
