package com.elumine.searchdeadcode.actions

import com.elumine.searchdeadcode.findings.SdcFindingsStore
import com.intellij.codeInsight.daemon.DaemonCodeAnalyzer
import com.intellij.openapi.actionSystem.ActionUpdateThread
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.project.DumbAware

class ClearFindingsAction : AnAction(), DumbAware {

    override fun getActionUpdateThread(): ActionUpdateThread = ActionUpdateThread.BGT

    override fun update(e: AnActionEvent) {
        val project = e.project
        e.presentation.isEnabled =
            project != null && SdcFindingsStore.getInstance(project).summary().totalFindings > 0
    }

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        SdcFindingsStore.getInstance(project).clear()
        DaemonCodeAnalyzer.getInstance(project).restart()
    }
}
