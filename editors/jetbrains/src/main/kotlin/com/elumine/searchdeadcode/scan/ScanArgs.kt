package com.elumine.searchdeadcode.scan

/** Builds the CLI argument list. Pure, exported for testing: no spawning. */
object ScanArgs {
    fun build(
        rootPath: String,
        outputFile: String,
        minConfidence: String,
        cacheFile: String,
        baselineFile: String?,
        exclude: List<String>,
        extraArgs: List<String>,
    ): List<String> {
        val args = mutableListOf(
            rootPath,
            "--format", "sarif",
            "--output", outputFile,
            "--min-confidence", minConfidence,
            // Without this the CLI drops a 3 MB .searchdeadcode-cache.json at
            // the project root: an unversioned file plus VFS refresh noise.
            "--cache-path", cacheFile,
        )
        if (baselineFile != null) {
            args.add("--baseline")
            args.add(baselineFile)
        }
        for (glob in exclude) {
            args.add("--exclude")
            args.add(glob)
        }
        args.addAll(extraArgs)
        return args
    }
}
