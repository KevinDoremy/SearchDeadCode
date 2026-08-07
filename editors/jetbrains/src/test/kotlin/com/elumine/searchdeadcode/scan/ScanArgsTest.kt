package com.elumine.searchdeadcode.scan

import org.junit.Assert.assertEquals
import org.junit.Test

class ScanArgsTest {

    @Test
    fun `builds the argument list in the VS Code order, cache redirected`() {
        val args = ScanArgs.build(
            rootPath = "/ws/proj",
            outputFile = "/tmp/scan.sarif",
            minConfidence = "medium",
            cacheFile = "/sys/searchdeadcode/cache/p.json",
            baselineFile = null,
            exclude = listOf("**/build/**", "**/gen/**"),
            extraArgs = listOf("--ratchet"),
        )
        assertEquals(
            listOf(
                "/ws/proj",
                "--format", "sarif",
                "--output", "/tmp/scan.sarif",
                "--min-confidence", "medium",
                "--cache-path", "/sys/searchdeadcode/cache/p.json",
                "--exclude", "**/build/**",
                "--exclude", "**/gen/**",
                "--ratchet",
            ),
            args,
        )
    }

    @Test
    fun `passes the conventional baseline only when it exists`() {
        val args = ScanArgs.build(
            rootPath = "/ws/proj",
            outputFile = "/tmp/scan.sarif",
            minConfidence = "high",
            cacheFile = "/sys/c.json",
            baselineFile = "/ws/proj/.deadcode-baseline.json",
            exclude = emptyList(),
            extraArgs = emptyList(),
        )
        val baselineIndex = args.indexOf("--baseline")
        assertEquals("/ws/proj/.deadcode-baseline.json", args[baselineIndex + 1])
    }
}
