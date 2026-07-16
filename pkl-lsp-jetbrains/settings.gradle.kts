pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
        maven { url = uri("https://cache-redirector.jetbrains.com/plugins.gradle.org") }
    }
    plugins {
        id("org.jetbrains.intellij.platform") version "2.18.1"
        id("org.jetbrains.kotlin.jvm") version "2.1.20"
    }
    resolutionStrategy {
        eachPlugin {
            if (requested.id.id == "org.jetbrains.intellij.platform") {
                useModule("org.jetbrains.intellij.platform:intellij-platform-gradle-plugin:2.18.1")
            }
        }
    }
}

dependencyResolutionManagement {
    repositories {
        mavenCentral()
        maven { url = uri("https://cache-redirector.jetbrains.com/intellij-dependencies") }
        maven { url = uri("https://cache-redirector.jetbrains.com/www.jetbrains.com/intellij-repository/releases") }
    }
}

rootProject.name = "pkl-lsp"
