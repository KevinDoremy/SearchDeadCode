package com.elumine.searchdeadcode.actions

import com.elumine.searchdeadcode.scan.SdcScanService
import com.intellij.openapi.actionSystem.ActionUpdateThread
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.project.DumbAware

/**
 * DumbAware: Android Studio spends a lot of its life indexing after Gradle
 * syncs, and the scan needs no index at all.
 */
class ScanAction : AnAction(), DumbAware {

    override fun getActionUpdateThread(): ActionUpdateThread = ActionUpdateThread.BGT

    override fun update(e: AnActionEvent) {
        val project = e.project
        e.presentation.isEnabled =
            project != null && !SdcScanService.getInstance(project).isRunning()
    }

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        SdcScanService.getInstance(project).runScan()
    }
}
