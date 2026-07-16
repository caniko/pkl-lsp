package com.tartanoglu.pkllsp

import com.intellij.icons.AllIcons
import com.intellij.lang.Language
import com.intellij.openapi.fileTypes.LanguageFileType

object PklLanguage : Language("Pkl")

class PklFileType : LanguageFileType(PklLanguage) {
    override fun getName() = "Pkl"
    override fun getDescription() = "Pkl source file"
    override fun getDefaultExtension() = "pkl"
    override fun getIcon() = AllIcons.FileTypes.Text
}
