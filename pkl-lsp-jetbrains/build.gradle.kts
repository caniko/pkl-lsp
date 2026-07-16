plugins {
    id("org.jetbrains.kotlin.jvm")
    id("org.jetbrains.intellij.platform")
}

group = providers.gradleProperty("pluginGroup").get()
version = providers.gradleProperty("pluginVersion").get()

base {
    archivesName.set("pkl-lsp")
}

repositories {
    mavenCentral()
    intellijPlatform {
        localPlatformArtifacts()
        defaultRepositories()
    }
}

kotlin {
    jvmToolchain(21)
}

dependencies {
    intellijPlatform {
        val localIdea = file("vendor/idea")
        if (localIdea.isDirectory) {
            local(localIdea)
        } else {
            intellijIdea("2025.1.7.1")
        }
    }
    testImplementation(kotlin("test"))
    testImplementation("junit:junit:4.13.2")
}

// Gradle 9.5 no longer exposes a task's project `configurations` property to
// nixpkgs' legacy init script. Keep dependency discovery explicit so the
// reproducible Nix MITM cache can resolve every project and buildscript
// configuration without relying on that deprecated delegation.
tasks.register("resolveGradleDependencies") {
    doLast {
        configurations.filter { it.isCanBeResolved }.forEach { it.resolve() }
        buildscript.configurations.filter { it.isCanBeResolved }.forEach { it.resolve() }
    }
}

tasks {
    withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>().configureEach {
        compilerOptions.jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_21)
    }

    test {
        useJUnitPlatform()
    }
}

intellijPlatform {
    buildSearchableOptions.set(false)
    signing {
        val certificateChain = providers.gradleProperty("intellijPlatformSigningCertificateChain")
            .orElse(providers.environmentVariable("CERTIFICATE_CHAIN_FILE"))
        val privateKey = providers.gradleProperty("intellijPlatformSigningPrivateKey")
            .orElse(providers.environmentVariable("PRIVATE_KEY_FILE"))
        certificateChainFile.set(certificateChain.map { layout.projectDirectory.file(it) })
        privateKeyFile.set(privateKey.map { layout.projectDirectory.file(it) })
        password.set(
            providers.gradleProperty("intellijPlatformSigningPrivateKeyPassword")
                .orElse(providers.environmentVariable("PRIVATE_KEY_PASSWORD"))
        )
    }
    pluginConfiguration {
        ideaVersion {
            sinceBuild.set("251")
        }
        changeNotes.set(
            "<p>Initial release with bundled Pkl language-server binaries.</p>"
        )
    }
}
