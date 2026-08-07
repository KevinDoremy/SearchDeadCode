package com.elumine.searchdeadcode.binary

import com.intellij.openapi.application.PathManager
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.util.SystemInfo
import com.intellij.util.io.HttpRequests
import com.intellij.util.system.CpuArch
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.nio.file.StandardCopyOption
import java.nio.file.attribute.PosixFilePermission
import java.security.MessageDigest

/**
 * Downloads the release asset PINNED to the plugin's own version — one number
 * on every platform — and verifies it against the published SHA-256 before it
 * is ever executable. There is deliberately no runtime "latest" resolution:
 * this repo's latest release by date is routinely a vscode-v* tag that ships
 * no binaries.
 *
 * Uses the platform's HttpRequests so the IDE's proxy configuration applies.
 */
object SdcBinaryDownloader {

    private const val REPO = "KevinDoremy/SearchDeadCode"
    private val RELEASE_VERSION = Regex("""^\d+\.\d+\.\d+(\.\d+)?$""")

    class DownloadException(message: String) : Exception(message)

    /**
     * The CLI version this plugin build is pinned to, stamped at build time.
     * Null for dev builds (0.0.0-dev), which have no release to download.
     */
    fun cliVersion(): String? {
        val props = java.util.Properties()
        val stream = javaClass.getResourceAsStream("/searchdeadcode/cli-version.properties")
            ?: return null
        stream.use { props.load(it) }
        val version = props.getProperty("cli.version")?.trim() ?: return null
        // A plugin-only fix may carry a 4th segment (0.19.1.1); the binary it
        // pins is the 3-segment crate release.
        if (!RELEASE_VERSION.matches(version)) return null
        return version.split('.').take(3).joinToString(".")
    }

    /** Release asset for this machine, or null when no asset is published for it. */
    fun assetName(): String? {
        val arch = when (CpuArch.CURRENT) {
            CpuArch.X86_64 -> "x86_64"
            CpuArch.ARM64 -> "aarch64"
            else -> return null
        }
        return when {
            SystemInfo.isMac -> "searchdeadcode-macos-$arch"
            SystemInfo.isLinux -> "searchdeadcode-linux-$arch"
            // Only x64 is published for Windows; arm64 Windows runs it emulated.
            SystemInfo.isWindows -> "searchdeadcode-windows-x86_64.exe"
            else -> null
        }
    }

    /** Where the pinned download lives, or null for a dev build. */
    fun installedPath(): Path? {
        val version = cliVersion() ?: return null
        return installDir(version).resolve(SdcBinaryLocator.binaryName)
    }

    fun available(): Boolean = cliVersion() != null && assetName() != null

    private fun installDir(version: String): Path =
        Paths.get(PathManager.getSystemPath(), "searchdeadcode", version)

    /**
     * Downloads, verifies and installs the binary. Blocking — run it from a
     * background task. Returns the installed path.
     */
    fun download(indicator: ProgressIndicator): Path {
        val version = cliVersion() ?: throw DownloadException("dev builds have no pinned release")
        val asset = assetName() ?: throw DownloadException("no published binary for this machine")
        val base = "https://github.com/$REPO/releases/download/v$version"

        indicator.text = "Downloading searchdeadcode $version"
        val dir = installDir(version)
        Files.createDirectories(dir)
        val tmp = Files.createTempFile(dir, "download", ".part")
        try {
            HttpRequests.request("$base/$asset")
                .productNameAsUserAgent()
                .saveToFile(tmp, indicator)

            indicator.text = "Verifying checksum"
            val expected = HttpRequests.request("$base/$asset.sha256")
                .productNameAsUserAgent()
                .readString()
                .trim()
                .split(Regex("\\s+"))
                .firstOrNull()
                ?.lowercase()
                ?: throw DownloadException("empty checksum file for $asset")
            val actual = sha256(tmp)
            if (actual != expected) {
                throw DownloadException(
                    "checksum mismatch for $asset: expected $expected, got $actual",
                )
            }

            val target = dir.resolve(SdcBinaryLocator.binaryName)
            Files.move(tmp, target, StandardCopyOption.REPLACE_EXISTING)
            if (!SystemInfo.isWindows) {
                val perms = Files.getPosixFilePermissions(target).toMutableSet()
                perms.add(PosixFilePermission.OWNER_EXECUTE)
                perms.add(PosixFilePermission.GROUP_EXECUTE)
                perms.add(PosixFilePermission.OTHERS_EXECUTE)
                Files.setPosixFilePermissions(target, perms)
            }

            markInstallChannel(base)
            return target
        } finally {
            Files.deleteIfExists(tmp)
        }
    }

    /**
     * Fire-and-forget install counter, only after a successful download —
     * never on cache hits. The marker asset exists from 0.20.0 on; a 404 on
     * older releases is part of the contract.
     */
    private fun markInstallChannel(base: String) {
        try {
            HttpRequests.request("$base/channel-install-jetbrains")
                .connectTimeout(3_000)
                .readTimeout(3_000)
                .tryConnect()
        } catch (_: Exception) {
            // counting must never break an install
        }
    }

    private fun sha256(file: Path): String {
        val digest = MessageDigest.getInstance("SHA-256")
        Files.newInputStream(file).use { input ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                digest.update(buffer, 0, read)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }
}
