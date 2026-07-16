package com.tartanoglu.pkllsp

import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.openapi.components.StoragePathMacros
import com.intellij.openapi.project.Project

@State(
    name = "PklLspSettings",
    storages = [Storage(StoragePathMacros.WORKSPACE_FILE)]
)
@Service(Service.Level.PROJECT)
class PklLspSettings : PersistentStateComponent<PklLspSettings.State> {
    data class State(var serverPath: String = "")

    private var currentState = State()

    override fun getState() = currentState

    override fun loadState(state: State) {
        currentState = state
    }

    companion object {
        fun getInstance(project: Project): PklLspSettings = project.getService(PklLspSettings::class.java)
    }
}
