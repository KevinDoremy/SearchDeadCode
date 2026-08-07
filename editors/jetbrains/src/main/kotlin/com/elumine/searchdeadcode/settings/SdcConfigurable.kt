package com.elumine.searchdeadcode.settings

import com.elumine.searchdeadcode.binary.SdcBinaryLocator
import com.intellij.openapi.options.BoundConfigurable
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.DialogPanel
import com.intellij.ui.dsl.builder.AlignX
import com.intellij.ui.dsl.builder.bindItem
import com.intellij.ui.dsl.builder.bindSelected
import com.intellij.ui.dsl.builder.bindText
import com.intellij.ui.dsl.builder.panel
import com.intellij.ui.dsl.builder.rows

/**
 * Settings > Tools > SearchDeadCode. The binary path is application-level
 * (machine-local); everything else is project-level and lands in
 * .idea/searchdeadcode.xml, committable team policy.
 */
class SdcConfigurable(private val project: Project) : BoundConfigurable("SearchDeadCode") {

    override fun createPanel(): DialogPanel {
        val app = SdcAppSettings.getInstance().state
        val proj = SdcProjectSettings.getInstance(project).state

        return panel {
            row {
                checkBox("Enable scanning")
                    .bindSelected(proj::enabled)
            }
            row("Binary path:") {
                textField()
                    .align(AlignX.FILL)
                    .bindText(app::binaryPath)
                    .comment(
                        "Absolute path to searchdeadcode. Leave empty to use PATH, " +
                            "well-known install dirs, or the downloaded binary. " +
                            "Stored per machine, not in the project.",
                    )
            }
            row("Minimum confidence:") {
                comboBox(SdcProjectSettings.CONFIDENCE_VALUES)
                    .bindItem({ proj.minConfidence }, { proj.minConfidence = it ?: "medium" })
            }
            row("Rules:") {
                textField()
                    .align(AlignX.FILL)
                    .bindText(
                        { proj.rules.joinToString(",") },
                        { value ->
                            proj.rules = value.split(',')
                                .map(String::trim)
                                .filter(String::isNotEmpty)
                                .toMutableList()
                        },
                    )
                    .comment("Comma-separated detector allowlist, filtered plugin-side. Empty keeps everything.")
            }
            row {
                textArea()
                    .rows(3)
                    .align(AlignX.FILL)
                    .label("Exclude globs (one per line):")
                    .bindText(
                        { proj.exclude.joinToString("\n") },
                        { value -> proj.exclude = value.toLines() },
                    )
            }
            row {
                textArea()
                    .rows(3)
                    .align(AlignX.FILL)
                    .label("Extra CLI arguments (one per line):")
                    .bindText(
                        { proj.extraArgs.joinToString("\n") },
                        { value -> proj.extraArgs = value.toLines() },
                    )
                    .comment("Appended verbatim, e.g. --ratchet or --target app")
            }
        }
    }

    override fun apply() {
        super.apply()
        // the next scan re-resolves against the new path
        SdcBinaryLocator.getInstance().invalidate()
    }

    private fun String.toLines(): MutableList<String> =
        lines().map(String::trim).filter(String::isNotEmpty).toMutableList()
}
