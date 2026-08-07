package com.elumine.searchdeadcode.binary

import com.elumine.searchdeadcode.settings.SdcAppSettings
import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.execution.process.CapturingProcessHandler
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.util.SystemInfo
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

/**
 * Finds a usable binary: configured path, then the pinned download, then
 * PATH, then well-known install dirs. An IDE launched from the Dock inherits
 * a PATH without Homebrew or cargo, so the bare lookup alone is not enough —
 * the same problem the VS Code extension already solved.
 *
 * Never call [resolve] on the EDT: the probe blocks up to 10 s per candidate.
 */
@Service(Service.Level.APP)
class SdcBinaryLocator {

    enum class Source { SETTING, DOWNLOADED, PATH, KNOWN_DIR }

    sealed interface Resolution {
        /** Where the binary came from is kept for diagnostics. */
        data class Found(val path: String, val version: String, val source: Source) : Resolution
        data class TooOld(val path: String, val version: String) : Resolution
        object NotFound : Resolution
    }

    companion object {
        /** Minimum CLI version whose SARIF output this plugin understands. */
        const val MIN_VERSION = "0.10.0"
        private const val PROBE_TIMEOUT_MS = 10_000

        fun getInstance(): SdcBinaryLocator = service()

        private val KNOWN_DIRS: List<String> = if (SystemInfo.isWindows) {
            // scoop shims and winget's portable links: a PATH updated by the
            // installer is invisible to an IDE that was already running
            listOf(
                "${System.getProperty("user.home")}\\scoop\\shims",
                "${System.getenv("LOCALAPPDATA") ?: "${System.getProperty("user.home")}\\AppData\\Local"}\\Microsoft\\WinGet\\Links",
                "${System.getProperty("user.home")}\\.cargo\\bin",
            )
        } else {
            listOf(
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "${System.getProperty("user.home")}/.cargo/bin",
                "${System.getProperty("user.home")}/.local/bin",
            )
        }

        val binaryName: String
            get() = if (SystemInfo.isWindows) "searchdeadcode.exe" else "searchdeadcode"
    }

    @Volatile
    private var cached: Resolution.Found? = null

    /** Called when the configured path changes; the next scan re-resolves. */
    fun invalidate() {
        cached = null
    }

    fun resolve(indicator: ProgressIndicator? = null): Resolution {
        // A cached path whose file vanished (uninstall, brew cleanup) must
        // re-resolve, not crash the next spawn — PATH hits are absolute paths
        // too, no exemption.
        cached?.let { if (Files.exists(Paths.get(it.path))) return it }

        var tooOld: Resolution.TooOld? = null
        for ((candidate, source) in candidates()) {
            val versionText = probe(candidate, indicator) ?: continue
            val (major, minor, patch) = Versions.parse(versionText)
            val version = "$major.$minor.$patch"
            if (Versions.isAtLeast(version, MIN_VERSION)) {
                val found = Resolution.Found(candidate, version, source)
                cached = found
                return found
            }
            // remember the first too-old hit but keep looking for a newer one
            if (tooOld == null) tooOld = Resolution.TooOld(candidate, version)
        }
        return tooOld ?: Resolution.NotFound
    }

    private fun candidates(): List<Pair<String, Source>> {
        val list = mutableListOf<Pair<String, Source>>()
        val configured = SdcAppSettings.getInstance().state.binaryPath.trim()
        if (configured.isNotEmpty()) list.add(configured to Source.SETTING)

        val downloaded = SdcBinaryDownloader.installedPath()
        if (downloaded != null && Files.isRegularFile(downloaded)) {
            list.add(downloaded.toString() to Source.DOWNLOADED)
        }

        findOnPath()?.let { list.add(it.toString() to Source.PATH) }

        for (dir in KNOWN_DIRS) {
            val p = Paths.get(dir, binaryName)
            if (Files.isRegularFile(p)) list.add(p.toString() to Source.KNOWN_DIR)
        }
        return list
    }

    private fun findOnPath(): Path? {
        val pathVar = System.getenv("PATH") ?: return null
        for (dir in pathVar.split(java.io.File.pathSeparatorChar)) {
            if (dir.isBlank()) continue
            val p = Paths.get(dir, binaryName)
            if (Files.isRegularFile(p)) return p
        }
        return null
    }

    /** Runs `<candidate> --version`, returning its stdout, or null if unusable. */
    private fun probe(candidate: String, indicator: ProgressIndicator?): String? = try {
        val cmd = GeneralCommandLine(candidate, "--version")
            .withCharset(Charsets.UTF_8)
            .withEnvironment(System.getenv())
        val handler = CapturingProcessHandler(cmd)
        // With an indicator, Cancel kills the probe instead of waiting out
        // 10 s per candidate on a binary that blocks.
        val output = if (indicator != null) {
            handler.runProcessWithProgressIndicator(indicator, PROBE_TIMEOUT_MS, true)
        } else {
            handler.runProcess(PROBE_TIMEOUT_MS, true)
        }
        if (output.exitCode == 0 && !output.isTimeout && !output.isCancelled) output.stdout else null
    } catch (_: Exception) {
        null // not here, try the next candidate
    }
}
