package com.elumine.searchdeadcode.scan

import com.elumine.searchdeadcode.sarif.SarifParser
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import java.io.File
import java.nio.file.Files
import java.util.concurrent.TimeUnit

/**
 * Runs the REAL CLI with the exact argument list the plugin generates. This
 * is the test that catches a flag the CLI does not have — `--cache-file`
 * shipped in a first draft where the real flag is `--cache-path`, and only a
 * live binary can object. Skips cleanly where no binary exists (plain CI
 * runners); the repo's own debug build or a PATH install makes it run.
 */
class RealCliScanTest {

    private fun findBinary(): String? {
        // the repo's own debug build first: it matches the source of truth
        val repoDebug = generateSequence(File(System.getProperty("user.dir"))) { it.parentFile }
            .take(4)
            .map { File(it, "target/debug/searchdeadcode") }
            .firstOrNull { it.canExecute() }
        if (repoDebug != null) return repoDebug.absolutePath

        val exe = if (System.getProperty("os.name").startsWith("Windows")) "searchdeadcode.exe" else "searchdeadcode"
        return System.getenv("PATH")?.split(File.pathSeparatorChar)
            ?.map { File(it, exe) }
            ?.firstOrNull { it.canExecute() }
            ?.absolutePath
    }

    @Test
    fun `the generated argument list is accepted by the real CLI`() {
        val binary = findBinary()
        assumeTrue("no searchdeadcode binary on this machine, skipping", binary != null)

        val projectDir = Files.createTempDirectory("sdc-plugin-args").toFile()
        val outputFile = File(projectDir, "out.sarif")
        val cacheFile = File(projectDir, "cache.json")
        try {
            File(projectDir, "src").mkdirs()
            File(projectDir, "src/Sample.kt").writeText(
                "package sample\n\nclass OrphanHelper { fun dead() = 1 }\n\nfun main() {}\n",
            )

            val args = ScanArgs.build(
                rootPath = projectDir.absolutePath,
                outputFile = outputFile.absolutePath,
                minConfidence = "medium",
                cacheFile = cacheFile.absolutePath,
                baselineFile = null,
                exclude = listOf("**/build/**"),
                extraArgs = emptyList(),
            )
            val process = ProcessBuilder(listOf(binary) + args)
                .directory(projectDir)
                .redirectErrorStream(false)
                .start()
            val finished = process.waitFor(120, TimeUnit.SECONDS)
            val stderr = process.errorStream.readBytes().toString(Charsets.UTF_8)
            assertTrue("the CLI did not finish", finished)

            // 0 = clean, 1 = findings; anything else means an argument the
            // CLI rejected or a broken contract
            val exit = process.exitValue()
            assertTrue("exit $exit — stderr:\n$stderr", exit == 0 || exit == 1)
            assertTrue("no SARIF report produced — stderr:\n$stderr", outputFile.isFile)
            SarifParser.parse(outputFile.readText())
        } finally {
            projectDir.deleteRecursively()
        }
    }
}
