package com.elumine.searchdeadcode.toolwindow

import com.elumine.searchdeadcode.findings.SdcFindingsListener
import com.elumine.searchdeadcode.findings.SdcFindingsStore
import com.elumine.searchdeadcode.sarif.SdcFinding
import com.intellij.openapi.Disposable
import com.intellij.openapi.actionSystem.ActionManager
import com.intellij.openapi.actionSystem.DefaultActionGroup
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.fileEditor.OpenFileDescriptor
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.SimpleToolWindowPanel
import com.intellij.openapi.vfs.LocalFileSystem
import com.intellij.ui.components.JBLabel
import com.intellij.ui.components.JBScrollPane
import com.intellij.ui.treeStructure.Tree
import com.intellij.util.text.DateFormatUtil
import com.intellij.util.ui.JBUI
import java.awt.BorderLayout
import java.awt.event.MouseAdapter
import java.awt.event.MouseEvent
import javax.swing.JPanel
import javax.swing.tree.DefaultMutableTreeNode
import javax.swing.tree.DefaultTreeModel
import javax.swing.tree.TreeSelectionModel

/**
 * Project-wide view of the last scan: a file → findings tree with
 * double-click navigation, fed by the store's message-bus topic. The editor
 * shows one file at a time; two hundred findings need a list.
 */
class SdcToolWindowPanel(
    private val project: Project,
    parentDisposable: Disposable,
) : SimpleToolWindowPanel(true, true) {

    private sealed interface NodeData {
        data class FileNode(val path: String, val relative: String, val count: Int) : NodeData {
            override fun toString() = "$relative ($count)"
        }

        data class FindingNode(val path: String, val finding: SdcFinding) : NodeData {
            override fun toString() =
                "${finding.line + 1}: ${finding.message} [${finding.ruleId}]"
        }
    }

    private val root = DefaultMutableTreeNode()
    private val model = DefaultTreeModel(root)
    private val tree = Tree(model).apply {
        isRootVisible = false
        showsRootHandles = true
        selectionModel.selectionMode = TreeSelectionModel.SINGLE_TREE_SELECTION
    }
    private val statusLabel = JBLabel().apply {
        border = JBUI.Borders.empty(4, 8)
    }

    init {
        val group = DefaultActionGroup(
            ActionManager.getInstance().getAction("SearchDeadCode.Scan"),
            ActionManager.getInstance().getAction("SearchDeadCode.Clear"),
        )
        val actionToolbar = ActionManager.getInstance()
            .createActionToolbar("SearchDeadCodeToolWindow", group, true)
        actionToolbar.targetComponent = this
        toolbar = actionToolbar.component

        tree.addMouseListener(object : MouseAdapter() {
            override fun mouseClicked(e: MouseEvent) {
                if (e.clickCount == 2) navigateToSelection()
            }
        })

        setContent(JPanel(BorderLayout()).apply {
            add(statusLabel, BorderLayout.NORTH)
            add(JBScrollPane(tree), BorderLayout.CENTER)
        })

        project.messageBus.connect(parentDisposable).subscribe(
            SdcFindingsListener.TOPIC,
            object : SdcFindingsListener {
                override fun findingsChanged() {
                    // the topic can fire from any thread, including inside a
                    // write action; UI work waits its turn
                    ApplicationManager.getApplication().invokeLater(
                        { refresh() },
                        project.disposed,
                    )
                }
            },
        )
        refresh()
    }

    private fun refresh() {
        val store = SdcFindingsStore.getInstance(project)
        val basePath = (project.basePath ?: "").trimEnd('/')

        root.removeAllChildren()
        for ((path, findings) in store.allByPath().toSortedMap()) {
            val relative = if (basePath.isNotEmpty() && path.startsWith("$basePath/")) {
                path.removePrefix("$basePath/")
            } else {
                path
            }
            val fileNode = DefaultMutableTreeNode(NodeData.FileNode(path, relative, findings.size))
            for (finding in findings.sortedBy { it.line }) {
                fileNode.add(DefaultMutableTreeNode(NodeData.FindingNode(path, finding)))
            }
            root.add(fileNode)
        }
        model.reload()

        val summary = store.summary()
        statusLabel.text = when {
            summary.scannedAtMillis == 0L -> "No scan yet. Tools > SearchDeadCode > Scan for Dead Code."
            summary.totalFindings == 0 -> "No dead code found (scanned at ${time(summary.scannedAtMillis)})."
            else -> buildString {
                append("${summary.totalFindings} findings in ${summary.fileCount} files")
                append(" — scanned at ${time(summary.scannedAtMillis)}")
                if (summary.invalidatedFiles > 0) {
                    append("; ${summary.invalidatedFiles} files edited since, their findings dropped")
                }
            }
        }
    }

    private fun time(millis: Long): String = DateFormatUtil.formatTime(millis)

    private fun navigateToSelection() {
        val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
        val data = node.userObject as? NodeData.FindingNode ?: return
        val file = LocalFileSystem.getInstance().findFileByPath(data.path) ?: return
        OpenFileDescriptor(project, file, data.finding.line, data.finding.column).navigate(true)
    }
}
