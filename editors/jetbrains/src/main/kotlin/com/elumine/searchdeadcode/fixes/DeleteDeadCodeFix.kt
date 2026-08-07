package com.elumine.searchdeadcode.fixes

import com.elumine.searchdeadcode.sarif.SarifParser
import com.elumine.searchdeadcode.sarif.SdcFinding
import com.intellij.codeInsight.intention.HighPriorityAction
import com.intellij.openapi.command.WriteCommandAction
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.project.Project
import com.intellij.psi.PsiFile

/**
 * Deletes the whole-line region the CLI marked safe. Only constructed when
 * the SARIF carries a fix. High priority — it is the reason the finding exists.
 */
class DeleteDeadCodeFix(path: String, finding: SdcFinding) :
    SdcFixBase(path, finding), HighPriorityAction {

    override fun getText(): String {
        val described = SarifParser.describe(finding)
            ?: return "Delete unused declaration"
        return "Delete unused ${described.first} '${described.second}'"
    }

    override fun invoke(project: Project, editor: Editor?, file: PsiFile?) {
        val fix = finding.fix ?: return
        val document = file?.viewProvider?.document ?: return
        WriteCommandAction.runWriteCommandAction(project, text, null, {
            // The store guarantees the file is untouched since the scan, but
            // the region still has to exist before anything is cut.
            if (fix.startLine >= document.lineCount) return@runWriteCommandAction
            val start = document.getLineStartOffset(fix.startLine)
            val end = if (fix.endLine + 1 < document.lineCount) {
                document.getLineStartOffset(fix.endLine + 1)
            } else {
                document.textLength
            }
            if (start <= end) document.deleteString(start, end)
            // the resulting document change invalidates the file's findings
        }, file)
    }
}
