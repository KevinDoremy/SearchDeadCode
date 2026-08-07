package com.elumine.searchdeadcode.scan

import com.elumine.searchdeadcode.baseline.BaselineWriter
import com.elumine.searchdeadcode.binary.SdcBinaryLocator
import com.elumine.searchdeadcode.findings.SdcFindingsStore
import com.elumine.searchdeadcode.notify.SdcNotifier
import com.elumine.searchdeadcode.sarif.SarifParser
import com.elumine.searchdeadcode.sarif.SdcFinding
import com.elumine.searchdeadcode.settings.SdcProjectSettings
import com.intellij.codeInsight.daemon.DaemonCodeAnalyzer
import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.execution.process.CapturingProcessHandler
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.application.PathManager
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.Task
import com.intellij.openapi.project.Project
import com.intellij.openapi.util.io.FileUtil
import com.intellij.openapi.vfs.LocalFileSystem
import java.io.File
import java.nio.file.Files
import java.nio.file.Paths
import java.util.concurrent.atomic.AtomicBoolean

/**
 * One scan at a time: resolve the binary, save every open document (the CLI
 * reads the DISK, the editor shows the buffer — scanning unsaved state was a
 * VS Code bug this port refuses to inherit), spawn, parse, publish.
 */
@Service(Service.Level.PROJECT)
class SdcScanService(private val project: Project) {

    private val running = AtomicBoolean(false)

    fun isRunning(): Boolean = running.get()

    fun runScan() {
        val settings = SdcProjectSettings.getInstance(project).state
        if (!settings.enabled) {
            SdcNotifier.info(project, "SearchDeadCode is disabled in settings.")
            return
        }
        if (project.basePath == null) {
            SdcNotifier.warn(project, "Open a project before scanning for dead code.")
            return
        }
        if (!running.compareAndSet(false, true)) return

        object : Task.Backgroundable(project, "Scanning for dead code", true) {
            override fun run(indicator: ProgressIndicator) {
                try {
                    doScan(indicator)
                } finally {
                    running.set(false)
                }
            }
        }.queue()
    }

    private fun doScan(indicator: ProgressIndicator) {
        val basePath = project.basePath ?: return

        when (val resolution = SdcBinaryLocator.getInstance().resolve(indicator)) {
            is SdcBinaryLocator.Resolution.NotFound ->
                SdcNotifier.binaryMissing(project) { runScan() }
            is SdcBinaryLocator.Resolution.TooOld ->
                SdcNotifier.binaryTooOld(project, resolution.version, resolution.path)
            is SdcBinaryLocator.Resolution.Found ->
                scanWith(resolution.path, basePath, indicator)
        }
    }

    private fun scanWith(binaryPath: String, basePath: String, indicator: ProgressIndicator) {
        val settings = SdcProjectSettings.getInstance(project).state

        // Save buffers, then snapshot the open documents' stamps: a file
        // edited DURING the five-minute scan gets findings computed on its
        // old content, and those must be dropped on arrival.
        val stamps = mutableMapOf<String, Long>()
        ApplicationManager.getApplication().invokeAndWait {
            FileDocumentManager.getInstance().saveAllDocuments()
            for (file in FileEditorManager.getInstance(project).openFiles) {
                val doc = FileDocumentManager.getInstance().getCachedDocument(file) ?: continue
                stamps[file.path] = doc.modificationStamp
            }
        }
        val scanStartedAt = System.currentTimeMillis()

        val outputFile = FileUtil.createTempFile("searchdeadcode", ".sarif", true)
        try {
            val baseline = File(basePath, BaselineWriter.FILE_NAME)
            val args = ScanArgs.build(
                rootPath = basePath,
                outputFile = outputFile.absolutePath,
                minConfidence = settings.minConfidence,
                cacheFile = cacheFilePath(basePath),
                baselineFile = if (baseline.isFile) baseline.absolutePath else null,
                exclude = settings.exclude,
                extraArgs = settings.extraArgs,
            )
            val command = GeneralCommandLine(binaryPath)
                .withParameters(args)
                .withWorkDirectory(basePath)
                .withCharset(Charsets.UTF_8)
                .withEnvironment(System.getenv())

            val output = try {
                CapturingProcessHandler(command)
                    .runProcessWithProgressIndicator(indicator, TIMEOUT_MS, true)
            } catch (e: com.intellij.execution.ExecutionException) {
                // The binary vanished between resolution and spawn (uninstall,
                // brew cleanup). A guided notification, not an IDE error blob.
                SdcBinaryLocator.getInstance().invalidate()
                SdcNotifier.binaryMissing(project) { runScan() }
                LOG.info("searchdeadcode could not be started: ${e.message}")
                return
            }

            if (output.isCancelled) return
            if (output.isTimeout) {
                SdcNotifier.error(
                    project,
                    "Scan timed out",
                    "searchdeadcode did not finish within ${TIMEOUT_MS / 1000}s.",
                )
                return
            }

            // 0 = clean, 1 = findings (the gate is never armed here), 3 = a
            // gate the user opted into through extraArgs said no — a verdict,
            // not a crash. Everything else means the tool could not work.
            val exitCode = output.exitCode
            if (exitCode >= 2 && exitCode != 3) {
                LOG.warn("searchdeadcode exited with $exitCode\n${output.stderr}")
                SdcNotifier.error(
                    project,
                    "searchdeadcode exited with code $exitCode",
                    output.stderr.lineSequence().take(15).joinToString("\n").trim()
                        .ifEmpty { "No error output. See idea.log." },
                )
                return
            }
            if (exitCode == 3) {
                SdcNotifier.warn(
                    project,
                    "searchdeadcode gate failed (exit 3: ratchet or grade). Findings shown anyway.",
                )
            }

            val sarifText = try {
                Files.readString(outputFile.toPath())
            } catch (_: Exception) {
                LOG.warn("no SARIF report\n${output.stderr}")
                SdcNotifier.error(project, "Scan failed", "searchdeadcode produced no report.")
                return
            }
            val findings = try {
                SarifParser.parse(sarifText, settings.rules)
            } catch (e: SarifParser.SarifParseException) {
                SdcNotifier.error(project, "Scan failed", "Unreadable report: ${e.message}")
                return
            }

            publish(basePath, findings, stamps, scannedAt = scanStartedAt)
        } finally {
            FileUtil.delete(outputFile)
        }
    }

    private fun publish(
        basePath: String,
        findings: List<SdcFinding>,
        stamps: Map<String, Long>,
        scannedAt: Long,
    ) {
        val grouped = mutableMapOf<String, MutableList<SdcFinding>>()
        for (finding in findings) {
            val path = SarifParser.resolvePath(basePath, finding.uri) { File(it).exists() }
            grouped.getOrPut(path) { mutableListOf() }.add(finding)
        }

        ApplicationManager.getApplication().invokeLater({
            val fileDocumentManager = FileDocumentManager.getInstance()
            val localFs = LocalFileSystem.getInstance()
            var dropped = 0
            val fresh = grouped.filterKeys { path ->
                val vf = localFs.findFileByPath(path)
                val doc = vf?.let { fileDocumentManager.getCachedDocument(it) }
                val editedInEditor =
                    doc != null && stamps[path] != null && stamps[path] != doc.modificationStamp
                val editedOnDisk = vf != null && vf.timeStamp > scannedAt
                // A file OPENED during the scan and typed into has no stamp
                // snapshot and an untouched disk file — but everything was
                // saved at scan start, so unsaved now means edited since.
                val editedUnsaved = doc != null && fileDocumentManager.isDocumentUnsaved(doc)
                val stale = editedInEditor || editedOnDisk || editedUnsaved
                if (stale) dropped++
                !stale
            }

            SdcFindingsStore.getInstance(project).replaceAll(fresh, scannedAt)
            DaemonCodeAnalyzer.getInstance(project).restart()

            val total = fresh.values.sumOf { it.size }
            val message = when {
                total == 0 && dropped == 0 -> "No dead code found."
                total == 0 -> "No current findings ($dropped files changed during the scan)."
                else -> buildString {
                    append("Found $total dead code finding")
                    if (total > 1) append("s")
                    append(" in ${fresh.size} file")
                    if (fresh.size > 1) append("s")
                    if (dropped > 0) append(" ($dropped files changed during the scan, skipped)")
                    append(".")
                }
            }
            SdcNotifier.info(project, message)
        }, project.disposed)
    }

    private fun cacheFilePath(basePath: String): String {
        val dir = Paths.get(PathManager.getSystemPath(), "searchdeadcode", "cache")
        Files.createDirectories(dir)
        val key = Integer.toHexString(basePath.hashCode())
        return dir.resolve("${project.name.replace(Regex("[^A-Za-z0-9._-]"), "_")}-$key.json")
            .toString()
    }

    companion object {
        private const val TIMEOUT_MS = 5 * 60 * 1000
        private val LOG = Logger.getInstance(SdcScanService::class.java)

        fun getInstance(project: Project): SdcScanService = project.service()
    }
}
