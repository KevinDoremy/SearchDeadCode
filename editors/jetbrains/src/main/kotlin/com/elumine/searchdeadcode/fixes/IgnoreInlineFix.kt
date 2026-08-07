package com.elumine.searchdeadcode.fixes

import com.elumine.searchdeadcode.sarif.SdcFinding
import com.intellij.openapi.command.WriteCommandAction
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.Messages
import com.intellij.psi.PsiFile

/**
 * Inserts `// deadcode:ignore(reason)` above the declaration. The reason is
 * mandatory: an ignore without a why is how dead code became immortal in the
 * first place. Dialog first, write action after — never the other way.
 */
class IgnoreInlineFix(path: String, finding: SdcFinding) : SdcFixBase(path, finding) {

    override fun getText(): String = "Ignore here with a reason"

    override fun invoke(project: Project, editor: Editor?, file: PsiFile?) {
        val document = file?.viewProvider?.document ?: return
        if (finding.line >= document.lineCount) return

        val reason = Messages.showInputDialog(
            project,
            "searchdeadcode requires a reason for every inline ignore",
            "Ignore This Finding",
            null,
            "",
            object : com.intellij.openapi.ui.InputValidator {
                // validate what will actually be WRITTEN: a reason made of
                // parentheses alone cleans down to nothing, and the CLI
                // refuses a bare deadcode:ignore()
                override fun checkInput(input: String): Boolean =
                    cleanReason(input).isNotEmpty()

                override fun canClose(input: String): Boolean = checkInput(input)
            },
        )
        if (reason == null || cleanReason(reason).isEmpty()) return

        val lineStart = document.getLineStartOffset(finding.line)
        val lineEnd = document.getLineEndOffset(finding.line)
        val declarationLine = document.getText(com.intellij.openapi.util.TextRange(lineStart, lineEnd))
        val comment = buildIgnoreComment(declarationLine, reason)

        WriteCommandAction.runWriteCommandAction(project, text, null, {
            document.insertString(lineStart, comment)
            // the resulting document change invalidates the file's findings
        }, file)
    }

    companion object {
        /**
         * The CLI parses the reason with `\(([^)]*)\)`, so parentheses of any
         * kind would truncate it; they are removed rather than kept broken.
         */
        fun cleanReason(reason: String): String = reason
            .replace(Regex("[\r\n]+"), " ")
            .replace(Regex("[()]"), "")
            .replace(Regex("\\s+"), " ")
            .trim()

        fun buildIgnoreComment(declarationLine: String, reason: String): String {
            val indent = Regex("""^[ \t]*""").find(declarationLine)?.value ?: ""
            return "$indent// deadcode:ignore(${cleanReason(reason)})\n"
        }
    }
}
