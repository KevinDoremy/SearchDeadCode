package com.elumine.searchdeadcode.fixes

import com.elumine.searchdeadcode.findings.SdcFindingsStore
import com.elumine.searchdeadcode.sarif.SdcFinding
import com.intellij.codeInsight.intention.IntentionAction
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.project.Project
import com.intellij.psi.PsiFile

/**
 * Base of the three quick fixes. `startInWriteAction` is false on purpose:
 * fixes show dialogs first and wrap their edits in their own
 * WriteCommandAction. Availability revalidates against the store, so a fix
 * offered before an edit dies with its finding instead of cutting blind.
 */
abstract class SdcFixBase(
    protected val path: String,
    protected val finding: SdcFinding,
) : IntentionAction {

    override fun getFamilyName(): String = "SearchDeadCode"

    override fun startInWriteAction(): Boolean = false

    override fun isAvailable(project: Project, editor: Editor?, file: PsiFile?): Boolean =
        SdcFindingsStore.getInstance(project).contains(path, finding)
}
