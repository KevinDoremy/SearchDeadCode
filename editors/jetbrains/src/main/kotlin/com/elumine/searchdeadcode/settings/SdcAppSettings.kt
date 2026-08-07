package com.elumine.searchdeadcode.settings

import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.openapi.components.service

/**
 * Machine-local settings. The binary path names a file on THIS machine, so it
 * lives in the IDE's global config, never in the project's shareable .idea/.
 */
@Service(Service.Level.APP)
@State(name = "SearchDeadCodeApp", storages = [Storage("searchdeadcode.xml")])
class SdcAppSettings : PersistentStateComponent<SdcAppSettings.State> {

    class State {
        var binaryPath: String = ""
    }

    private var state = State()

    override fun getState(): State = state

    override fun loadState(state: State) {
        this.state = state
    }

    companion object {
        fun getInstance(): SdcAppSettings = service()
    }
}
