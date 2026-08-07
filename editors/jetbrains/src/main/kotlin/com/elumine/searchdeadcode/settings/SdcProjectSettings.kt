package com.elumine.searchdeadcode.settings

import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.openapi.components.service
import com.intellij.openapi.project.Project

/**
 * Per-project settings, stored in .idea/searchdeadcode.xml so a team can
 * commit its scan policy alongside the code.
 */
@Service(Service.Level.PROJECT)
@State(name = "SearchDeadCode", storages = [Storage("searchdeadcode.xml")])
class SdcProjectSettings : PersistentStateComponent<SdcProjectSettings.State> {

    class State {
        var enabled: Boolean = true

        /** low | medium | high | confirmed — passed to --min-confidence. */
        var minConfidence: String = "medium"

        /**
         * Rule allowlist, filtered on the plugin side after parsing (the CLI
         * has no per-rule flag). The default set is the one the VS Code
         * extension curated for editor noise. Empty = keep everything.
         */
        var rules: MutableList<String> = DEFAULT_RULES.toMutableList()

        /** Passed as repeated --exclude. */
        var exclude: MutableList<String> = mutableListOf()

        /** Appended verbatim after every other argument. */
        var extraArgs: MutableList<String> = mutableListOf()
    }

    private var state = State()

    override fun getState(): State = state

    override fun loadState(state: State) {
        this.state = state
    }

    companion object {
        val DEFAULT_RULES = listOf(
            "DC001", "DC002", "DC003", "DC004", "DC005", "DC008",
            "DC010", "DC011", "DC012", "DC013", "DC019",
        )
        val CONFIDENCE_VALUES = listOf("low", "medium", "high", "confirmed")

        fun getInstance(project: Project): SdcProjectSettings = project.service()
    }
}
