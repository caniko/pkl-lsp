package com.tartanoglu.pkllsp

import com.intellij.ide.plugins.PluginManagerCore
import com.intellij.openapi.application.PathManager
import com.intellij.openapi.extensions.PluginId
import com.intellij.openapi.project.Project
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption
import java.nio.file.attribute.PosixFilePermission
import java.util.Locale

internal object PklServerDistribution {
    private const val pluginId = "com.tartanoglu.pkl-lsp"

    internal enum class Target(
        val resourceName: String,
        val binaryName: String,
        val os: String,
        val arch: String,
    ) {
        LINUX_X64("linux-x64", "pkl-lsp", "linux", "x86_64"),
        LINUX_ARM64("linux-arm64", "pkl-lsp", "linux", "aarch64"),
        WINDOWS_X64("win32-x64", "pkl-lsp.exe", "windows", "x86_64"),
        DARWIN_X64("darwin-x64", "pkl-lsp", "macos", "x86_64"),
        DARWIN_ARM64("darwin-arm64", "pkl-lsp", "macos", "aarch64");
    }

    internal fun detectTarget(osName: String, osArch: String): Target? {
        val os = osName.lowercase(Locale.ROOT)
        val arch = osArch.lowercase(Locale.ROOT)
        return when {
            os.contains("win") && arch in setOf("x86_64", "amd64") -> Target.WINDOWS_X64
            os.contains("mac") || os.contains("darwin") -> when (arch) {
                "aarch64", "arm64" -> Target.DARWIN_ARM64
                "x86_64", "amd64" -> Target.DARWIN_X64
                else -> null
            }
            os.contains("linux") -> when (arch) {
                "aarch64", "arm64" -> Target.LINUX_ARM64
                "x86_64", "amd64" -> Target.LINUX_X64
                else -> null
            }
            else -> null
        }
    }

    internal fun resolve(project: Project): Path {
        val configured = PklLspSettings.getInstance(project).state.serverPath.trim()
        if (configured.isNotEmpty()) {
            val path = Path.of(configured)
            require(Files.isRegularFile(path)) { "Configured Pkl LSP server does not exist: $path" }
            return path
        }

        val target = detectTarget(System.getProperty("os.name"), System.getProperty("os.arch"))
            ?: error("No bundled Pkl LSP binary exists for ${System.getProperty("os.name")} / ${System.getProperty("os.arch")}; configure a custom server path in Settings | Tools | Pkl LSP.")
        val resource = "/servers/${target.resourceName}/${target.binaryName}"
        val stream = PklServerDistribution::class.java.getResourceAsStream(resource)
            ?: error("Bundled Pkl LSP resource is missing: $resource")

        val pluginVersion = PluginManagerCore.getPlugin(PluginId.getId(pluginId))?.version ?: "development"
        val cacheDirectory = Path.of(PathManager.getSystemPath(), "pkl-lsp", pluginVersion, target.resourceName)
        Files.createDirectories(cacheDirectory)
        val destination = cacheDirectory.resolve(target.binaryName)
        if (Files.isRegularFile(destination)) return destination

        val temporary = Files.createTempFile(cacheDirectory, ".pkl-lsp-", ".tmp")
        try {
            stream.use { input -> Files.copy(input, temporary, StandardCopyOption.REPLACE_EXISTING) }
            if (target.os != "windows") {
                runCatching {
                    Files.setPosixFilePermissions(
                        temporary,
                        setOf(
                            PosixFilePermission.OWNER_READ,
                            PosixFilePermission.OWNER_WRITE,
                            PosixFilePermission.OWNER_EXECUTE,
                        )
                    )
                }
            }
            runCatching {
                Files.move(temporary, destination, StandardCopyOption.ATOMIC_MOVE)
            }.getOrElse {
                Files.move(temporary, destination, StandardCopyOption.REPLACE_EXISTING)
            }
        } finally {
            Files.deleteIfExists(temporary)
        }
        return destination
    }
}
