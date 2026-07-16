package com.tartanoglu.pkllsp

import com.intellij.openapi.options.SearchableConfigurable
import com.intellij.openapi.project.Project
import java.awt.BorderLayout
import javax.swing.JComponent
import javax.swing.JLabel
import javax.swing.JPanel
import javax.swing.JTextField

class PklLspConfigurable(private val project: Project) : SearchableConfigurable {
    private val serverPath = JTextField()
    private val panel = JPanel(BorderLayout(8, 8)).apply {
        add(JLabel("Custom Pkl language-server executable (optional):"), BorderLayout.NORTH)
        add(serverPath, BorderLayout.CENTER)
    }

    override fun getId() = "com.tartanoglu.pkllsp.settings"
    override fun getDisplayName() = "Pkl LSP"
    override fun createComponent(): JComponent = panel

    override fun isModified(): Boolean = serverPath.text != PklLspSettings.getInstance(project).state.serverPath

    override fun apply() {
        PklLspSettings.getInstance(project).state.serverPath = serverPath.text.trim()
    }

    override fun reset() {
        serverPath.text = PklLspSettings.getInstance(project).state.serverPath
    }
}
