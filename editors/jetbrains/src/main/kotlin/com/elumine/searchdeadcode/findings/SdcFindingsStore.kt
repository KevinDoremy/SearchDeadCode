package com.elumine.searchdeadcode.findings

import com.elumine.searchdeadcode.sarif.SdcFinding
import com.intellij.openapi.Disposable
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.editor.event.DocumentEvent
import com.intellij.openapi.editor.event.DocumentListener
import com.intellij.openapi.editor.EditorFactory
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.openapi.vfs.VirtualFileManager
import com.intellij.openapi.vfs.newvfs.BulkFileListener
import com.intellij.openapi.vfs.newvfs.events.VFileContentChangeEvent
import com.intellij.openapi.vfs.newvfs.events.VFileDeleteEvent
import com.intellij.openapi.vfs.newvfs.events.VFileEvent
import com.intellij.util.messages.Topic

/** Project-bus notification: the set of findings changed in any way. */
interface SdcFindingsListener {
    fun findingsChanged()

    companion object {
        @JvmField
        val TOPIC: Topic<SdcFindingsListener> =
            Topic.create("SearchDeadCode findings", SdcFindingsListener::class.java)
    }
}

/**
 * The findings from the last scan, keyed by system-independent absolute path.
 *
 * Staleness policy, the single most important behaviour of the bridge: a
 * file's findings are dropped the moment the file is edited. A stale line
 * number is worse than no marker, and it keeps the delete fix from ever
 * cutting the wrong lines. Document edits cover typing and quick fixes; the
 * VFS listener covers external changes (git checkout, generation).
 */
@Service(Service.Level.PROJECT)
class SdcFindingsStore(private val project: Project) : Disposable {

    data class Summary(
        val totalFindings: Int,
        val fileCount: Int,
        val scannedAtMillis: Long,
        val invalidatedFiles: Int,
    )

    @Volatile
    private var byPath: Map<String, List<SdcFinding>> = emptyMap()

    @Volatile
    private var scannedAtMillis: Long = 0

    @Volatile
    private var invalidatedFiles: Int = 0

    init {
        // The multicaster covers every open document, including edits made by
        // other plugins. The handler runs on the EDT under the write action:
        // strictly a lookup and a map swap, no I/O, no PSI.
        EditorFactory.getInstance().eventMulticaster.addDocumentListener(
            object : DocumentListener {
                override fun documentChanged(event: DocumentEvent) {
                    val file = FileDocumentManager.getInstance().getFile(event.document) ?: return
                    invalidate(file.path)
                }
            },
            this,
        )
        // VFS_CHANGES is an application-level topic.
        ApplicationManager.getApplication().messageBus.connect(this).subscribe(
            VirtualFileManager.VFS_CHANGES,
            object : BulkFileListener {
                override fun after(events: List<VFileEvent>) {
                    for (event in events) {
                        if (event !is VFileContentChangeEvent && event !is VFileDeleteEvent) continue
                        val path = event.file?.path ?: continue
                        invalidate(path)
                    }
                }
            },
        )
    }

    fun replaceAll(findings: Map<String, List<SdcFinding>>, scannedAt: Long) {
        byPath = findings.filterValues { it.isNotEmpty() }
        scannedAtMillis = scannedAt
        invalidatedFiles = 0
        notifyChanged()
    }

    fun clear() {
        if (byPath.isEmpty()) return
        byPath = emptyMap()
        invalidatedFiles = 0
        notifyChanged()
    }

    fun findingsFor(path: String): List<SdcFinding> = byPath[path] ?: emptyList()

    fun findingsFor(file: VirtualFile): List<SdcFinding> = findingsFor(file.path)

    fun allByPath(): Map<String, List<SdcFinding>> = byPath

    /** Quick fixes revalidate through this: the fix dies with its finding. */
    fun contains(path: String, finding: SdcFinding): Boolean =
        byPath[path]?.contains(finding) == true

    /** Removes one finding (e.g. after it was baselined) without a rescan. */
    fun remove(path: String, finding: SdcFinding) {
        val current = byPath[path] ?: return
        val next = current - finding
        byPath = if (next.isEmpty()) byPath - path else byPath + (path to next)
        notifyChanged()
    }

    fun invalidate(path: String) {
        if (path !in byPath) return
        byPath = byPath - path
        invalidatedFiles++
        notifyChanged()
    }

    fun summary(): Summary = Summary(
        totalFindings = byPath.values.sumOf { it.size },
        fileCount = byPath.size,
        scannedAtMillis = scannedAtMillis,
        invalidatedFiles = invalidatedFiles,
    )

    private fun notifyChanged() {
        if (project.isDisposed) return
        project.messageBus.syncPublisher(SdcFindingsListener.TOPIC).findingsChanged()
    }

    override fun dispose() {
        // listeners are tied to this Disposable
    }

    companion object {
        fun getInstance(project: Project): SdcFindingsStore = project.service()
    }
}
