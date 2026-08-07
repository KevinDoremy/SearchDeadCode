package com.elumine.searchdeadcode.sarif

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.intOrNull

/**
 * SARIF 2.1.0 → findings. Pure module: no IntelliJ APIs, fully unit tested.
 *
 * Ported from editors/vscode/src/SarifParser.ts, against real output of the
 * searchdeadcode CLI, which varies by version: 0.10 emits neither
 * `partialFingerprints` nor `fixes[]`, newer versions add both. Everything
 * past the required core is treated as optional so the bridge degrades
 * instead of breaking.
 */
object SarifParser {

    class SarifParseException(message: String) : Exception(message)

    private val LEVELS = mapOf(
        "error" to SdcLevel.ERROR,
        "warning" to SdcLevel.WARNING,
        "note" to SdcLevel.NOTE,
    )

    private fun JsonObject?.obj(key: String): JsonObject? = this?.get(key) as? JsonObject
    private fun JsonObject?.arr(key: String): JsonArray? = this?.get(key) as? JsonArray
    private fun JsonObject?.str(key: String): String? =
        (this?.get(key) as? JsonPrimitive)?.takeIf { it.isString }?.content
    private fun JsonObject?.int(key: String): Int? = (this?.get(key) as? JsonPrimitive)?.intOrNull

    /**
     * @param ruleAllowlist rule IDs to keep; empty keeps everything
     */
    fun parse(text: String, ruleAllowlist: Collection<String> = emptyList()): List<SdcFinding> {
        val doc = try {
            Json.parseToJsonElement(text)
        } catch (e: Exception) {
            throw SarifParseException("not valid JSON: ${e.message}")
        }
        val root = doc as? JsonObject ?: throw SarifParseException("root is not an object")
        val version = root.str("version")
        if (version == null || !version.startsWith("2.1")) {
            throw SarifParseException("unsupported SARIF version: $version")
        }
        val runs = root.arr("runs") ?: throw SarifParseException("missing runs[]")

        val allow = ruleAllowlist.filter { it.isNotBlank() }.toSet()
        val findings = mutableListOf<SdcFinding>()

        for (rawRun in runs) {
            val run = rawRun as? JsonObject ?: continue

            // rules[] carries helpUri and the default level; both are optional
            val helpUris = mutableMapOf<String, String>()
            val defaultLevels = mutableMapOf<String, String>()
            val driver = run.obj("tool").obj("driver")
            for (rawRule in driver.arr("rules") ?: JsonArray(emptyList())) {
                val rule = rawRule as? JsonObject ?: continue
                val id = rule.str("id") ?: continue
                rule.str("helpUri")?.let { helpUris[id] = it }
                rule.obj("defaultConfiguration").str("level")?.let { defaultLevels[id] = it }
            }

            for (rawResult in run.arr("results") ?: JsonArray(emptyList())) {
                val result = rawResult as? JsonObject ?: continue
                val ruleId = result.str("ruleId") ?: continue
                if (allow.isNotEmpty() && ruleId !in allow) continue

                val physical = (result.arr("locations")?.firstOrNull() as? JsonObject)
                    .obj("physicalLocation")
                val uri = physical.obj("artifactLocation").str("uri") ?: continue
                val region = physical.obj("region") ?: continue
                val startLine = region.int("startLine") ?: continue
                val startColumn = region.int("startColumn") ?: 1

                val message = result.obj("message").str("text")
                val rawLevel = result.str("level") ?: defaultLevels[ruleId]
                val level = LEVELS[rawLevel] ?: SdcLevel.WARNING

                val fingerprint = result.obj("partialFingerprints")?.values
                    ?.filterIsInstance<JsonPrimitive>()
                    ?.firstOrNull { it.isString }
                    ?.content

                // fixes[]: only a whole-line deletion is understood; anything
                // else is ignored
                var fix: SdcFinding.DeletedRegion? = null
                val replacement = ((result.arr("fixes")?.firstOrNull() as? JsonObject)
                    .arr("artifactChanges")?.firstOrNull() as? JsonObject)
                    .arr("replacements")?.firstOrNull() as? JsonObject
                val deleted = replacement.obj("deletedRegion")
                val insertedText = replacement.obj("insertedContent").str("text")
                val deletedStart = deleted.int("startLine")
                val deletedEnd = deleted.int("endLine")
                if (deletedStart != null && deletedEnd != null && insertedText.isNullOrEmpty()) {
                    fix = SdcFinding.DeletedRegion(deletedStart - 1, deletedEnd - 1)
                }

                findings.add(
                    SdcFinding(
                        ruleId = ruleId,
                        message = message ?: "$ruleId reported here",
                        level = level,
                        uri = uri,
                        line = (startLine - 1).coerceAtLeast(0),
                        column = (startColumn - 1).coerceAtLeast(0),
                        helpUri = helpUris[ruleId],
                        fingerprint = fingerprint,
                        fix = fix,
                    ),
                )
            }
        }
        return findings
    }

    /**
     * Resolves a SARIF uri against the scanned root. Paths use '/' throughout;
     * they map directly onto VirtualFile paths on every OS.
     *
     * The CLI has reported paths relative to the PARENT of the scanned
     * directory (`myProject/src/Main.kt` for a scan of `myProject`), so a
     * naive join produces `myProject/myProject/src/Main.kt`. Strips the
     * root's own basename when the uri starts with it; `exists` lets the
     * caller confirm.
     */
    fun resolvePath(rootPath: String, uri: String, exists: (String) -> Boolean): String {
        val root = rootPath.replace('\\', '/').trimEnd('/')
        val cleanUri = uri.replace('\\', '/')
        if (cleanUri.startsWith("/")) return cleanUri

        val direct = "$root/${cleanUri.trimStart('/')}"
        if (exists(direct)) return direct

        val rootName = root.substringAfterLast('/')
        if (rootName.isNotEmpty() && (cleanUri == rootName || cleanUri.startsWith("$rootName/"))) {
            val stripped = "$root/${cleanUri.removePrefix(rootName).trimStart('/')}"
            if (exists(stripped)) return stripped
        }
        // last resort: report against the root so the finding is never dropped
        return direct
    }

    /**
     * Message shapes the CLI uses, e.g. `class 'LegacyEncoder' is never used`.
     * Returns kind to name, or null when the message does not match — a
     * baseline entry must never be invented from a guess.
     */
    fun describe(finding: SdcFinding): Pair<String, String>? {
        val m = Regex("""^(\w+)\s+'([^']+)'""").find(finding.message) ?: return null
        return m.groupValues[1] to m.groupValues[2]
    }

    /**
     * The single-word kinds `DeclarationKind::display_name` can emit
     * (src/graph/declaration.rs). `IssueFingerprint::matches` compares kind
     * to these LOWERCASE names, while messages capitalize the first word
     * ("Parameter 'ctx' is never used") — an entry written with the message's
     * casing would never suppress anything.
     */
    private val CLI_KINDS = setOf(
        "class", "interface", "object", "enum", "annotation", "function",
        "method", "constructor", "property", "field", "parameter", "import",
    )

    /**
     * (kind, name) normalized for a baseline entry: lowercased and validated
     * against the CLI's kind vocabulary. Null = do not offer the fix.
     */
    fun baselineKey(finding: SdcFinding): Pair<String, String>? {
        val (kind, name) = describe(finding) ?: return null
        val normalized = kind.lowercase()
        return if (normalized in CLI_KINDS) normalized to name else null
    }
}
