package com.elumine.searchdeadcode.sarif

enum class SdcLevel { ERROR, WARNING, NOTE }

data class SdcFinding(
    val ruleId: String,
    val message: String,
    /** SARIF level, defaulted to WARNING when the run omits it. */
    val level: SdcLevel,
    /** Path exactly as the CLI reported it (resolution is the caller's job). */
    val uri: String,
    /** 0-based, editor convention. */
    val line: Int,
    val column: Int,
    val helpUri: String? = null,
    /**
     * partialFingerprints value — a line-free hash of uri|name|rule, so it
     * survives edits elsewhere in the file. Absent from CLI 0.10 output.
     */
    val fingerprint: String? = null,
    /** 0-based inclusive line range the CLI says is safe to delete. */
    val fix: DeletedRegion? = null,
) {
    data class DeletedRegion(val startLine: Int, val endLine: Int)
}
