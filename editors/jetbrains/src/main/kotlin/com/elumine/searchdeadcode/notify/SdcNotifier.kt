package com.elumine.searchdeadcode.notify

import com.elumine.searchdeadcode.binary.SdcBinaryDownloader
import com.elumine.searchdeadcode.binary.SdcBinaryLocator
import com.elumine.searchdeadcode.settings.SdcConfigurable
import com.intellij.ide.BrowserUtil
import com.intellij.notification.Notification
import com.intellij.notification.NotificationAction
import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.ide.CopyPasteManager
import com.intellij.openapi.options.ShowSettingsUtil
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.Task
import com.intellij.openapi.project.Project
import java.awt.datatransfer.StringSelection

object SdcNotifier {

    private fun group() =
        NotificationGroupManager.getInstance().getNotificationGroup("SearchDeadCode")

    fun info(project: Project, content: String) {
        group().createNotification(content, NotificationType.INFORMATION).notify(project)
    }

    fun warn(project: Project, content: String) {
        group().createNotification(content, NotificationType.WARNING).notify(project)
    }

    fun error(project: Project, title: String, content: String) {
        group().createNotification(title, content, NotificationType.ERROR).notify(project)
    }

    fun binaryTooOld(project: Project, version: String, path: String) {
        warn(
            project,
            "searchdeadcode $version at $path is too old for this plugin " +
                "(needs ${SdcBinaryLocator.MIN_VERSION} or newer).",
        )
    }

    /**
     * The guided-install notification, one action per install channel. The
     * download action runs in its own background task and calls [onInstalled]
     * (typically a rescan) when the binary is in place.
     */
    fun binaryMissing(project: Project, onInstalled: () -> Unit) {
        val notification = group().createNotification(
            "searchdeadcode was not found",
            "Install the analyzer to scan this project for dead code.",
            NotificationType.WARNING,
        )

        val cliVersion = SdcBinaryDownloader.cliVersion()
        if (cliVersion != null && SdcBinaryDownloader.available()) {
            notification.addAction(object : NotificationAction("Download searchdeadcode $cliVersion") {
                override fun actionPerformed(e: AnActionEvent, n: Notification) {
                    n.expire()
                    object : Task.Backgroundable(project, "Downloading searchdeadcode", true) {
                        override fun run(indicator: ProgressIndicator) {
                            try {
                                val path = SdcBinaryDownloader.download(indicator)
                                SdcBinaryLocator.getInstance().invalidate()
                                info(project, "searchdeadcode $cliVersion installed at $path.")
                                onInstalled()
                            } catch (ex: Exception) {
                                error(project, "Download failed", ex.message ?: ex.toString())
                            }
                        }
                    }.queue()
                }
            })
        }
        notification.addAction(NotificationAction.createSimple("Copy Homebrew command") {
            CopyPasteManager.getInstance()
                .setContents(StringSelection("brew install KevinDoremy/tap/searchdeadcode"))
        })
        notification.addAction(NotificationAction.createSimple("Copy cargo command") {
            CopyPasteManager.getInstance()
                .setContents(StringSelection("cargo install searchdeadcode"))
        })
        notification.addAction(NotificationAction.createSimple("Open releases") {
            BrowserUtil.browse("https://github.com/KevinDoremy/SearchDeadCode/releases")
        })
        notification.addAction(NotificationAction.createSimple("Set path…") {
            ShowSettingsUtil.getInstance().showSettingsDialog(project, SdcConfigurable::class.java)
        })
        notification.notify(project)
    }
}
