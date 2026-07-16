package com.tartanoglu.pkllsp

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.platform.lsp.api.ProjectWideLspServerDescriptor

class PklLspServerSupportProvider : LspServerSupportProvider {
    override fun fileOpened(
        project: Project,
        file: VirtualFile,
        serverStarter: LspServerSupportProvider.LspServerStarter,
    ) {
        if (file.extension == "pkl") {
            serverStarter.ensureServerStarted(PklLspServerDescriptor(project))
        }
    }
}

private class PklLspServerDescriptor(project: Project) : ProjectWideLspServerDescriptor(project, "Pkl") {
    override fun isSupportedFile(file: VirtualFile) = file.extension == "pkl"

    override fun createCommandLine(): GeneralCommandLine =
        GeneralCommandLine(PklServerDistribution.resolve(project).toString(), "--stdio")
}
