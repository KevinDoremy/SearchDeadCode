package com.elumine.searchdeadcode.baseline

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * Writes the baseline shape the CLI actually accepts (src/baseline/mod.rs):
 * `{version, created_at, issues[{file,name,kind,line,fqn,rule}],
 * total_at_baseline}`.
 *
 * The VS Code extension writes a different shape (`{version, entries}`) that
 * `Baseline::load` rejects with exit 2 — this port fixes that instead of
 * reproducing it. The file is `.deadcode-baseline.json`, the conventional
 * name `--profile ci` picks up on its own, so an entry added from the IDE
 * takes effect in CI without any extra wiring.
 */
object BaselineWriter {

    @Serializable
    data class Issue(
        val file: String,
        val name: String,
        val kind: String,
        /** 1-based, CLI convention. */
        val line: Int,
        val fqn: String? = null,
        val rule: String? = null,
    )

    @Serializable
    data class Doc(
        val version: Int = 1,
        @SerialName("created_at") val createdAt: String,
        val issues: List<Issue>,
        @SerialName("total_at_baseline") val totalAtBaseline: Int,
    )

    const val FILE_NAME = ".deadcode-baseline.json"

    // encodeDefaults keeps "fqn": null / "rule": … in the output, the exact
    // shape serde emits; the CLI parses either way but a byte-similar file
    // makes diffs against CLI-generated baselines readable.
    @OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)
    private val json = Json {
        encodeDefaults = true
        prettyPrint = true
        prettyPrintIndent = "  "
        ignoreUnknownKeys = true
    }

    /** The CLI stamps created_at as epoch seconds in a string; match it. */
    fun now(): String = (System.currentTimeMillis() / 1000).toString()

    /**
     * Appends [entry] to an existing baseline document, creating the document
     * when absent or unreadable. Deduplicates the way
     * `IssueFingerprint::matches` matches on the Rust side: same file, name
     * and kind, AND within its ±10-line drift tolerance — two dead homonyms
     * 200 lines apart are two distinct entries, not a duplicate.
     * `created_at` and `total_at_baseline` are preserved on append.
     */
    fun append(existing: String?, entry: Issue, now: String = now()): String {
        val doc = existing?.takeIf { it.isNotBlank() }?.let {
            try {
                json.decodeFromString<Doc>(it)
            } catch (_: Exception) {
                null // unreadable baseline: start fresh rather than losing the action
            }
        }

        val issues = doc?.issues ?: emptyList()
        val duplicate = issues.any {
            it.file == entry.file && it.name == entry.name && it.kind == entry.kind &&
                kotlin.math.abs(it.line - entry.line) <= 10
        }
        val next = Doc(
            version = doc?.version ?: 1,
            createdAt = doc?.createdAt ?: now,
            issues = if (duplicate) issues else issues + entry,
            totalAtBaseline = doc?.totalAtBaseline ?: (issues.size + 1),
        )
        return json.encodeToString(Doc.serializer(), next) + "\n"
    }
}
