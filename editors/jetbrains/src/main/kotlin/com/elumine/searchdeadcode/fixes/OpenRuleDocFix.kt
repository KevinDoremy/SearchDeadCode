package com.elumine.searchdeadcode.fixes

import com.intellij.codeInsight.intention.IntentionAction
import com.intellij.codeInsight.intention.LowPriorityAction
import com.intellij.ide.BrowserUtil
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.project.Project
import com.intellij.psi.PsiFile

/** The clickable rule link of the VS Code diagnostic, as an intention. */
class OpenRuleDocFix(private val helpUri: String) : IntentionAction, LowPriorityAction {

    override fun getFamilyName(): String = "SearchDeadCode"

    override fun getText(): String = "Open searchdeadcode rule documentation"

    override fun startInWriteAction(): Boolean = false

    override fun isAvailable(project: Project, editor: Editor?, file: PsiFile?): Boolean = true

    override fun invoke(project: Project, editor: Editor?, file: PsiFile?) {
        BrowserUtil.browse(helpUri)
    }
}
