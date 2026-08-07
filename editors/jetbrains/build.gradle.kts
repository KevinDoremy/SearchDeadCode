import org.jetbrains.intellij.platform.gradle.IntelliJPlatformType
import org.jetbrains.intellij.platform.gradle.TestFrameworkType
import org.jetbrains.intellij.platform.gradle.models.ProductRelease
import org.jetbrains.kotlin.gradle.dsl.KotlinVersion

plugins {
    id("org.jetbrains.kotlin.jvm") version "2.3.21"
    id("org.jetbrains.kotlin.plugin.serialization") version "2.3.21"
    id("org.jetbrains.intellij.platform") version "2.18.1"
}

group = "com.elumine"
version = providers.gradleProperty("pluginVersion").get()

repositories {
    mavenCentral()
    intellijPlatform {
        defaultRepositories()
    }
}

dependencies {
    intellijPlatform {
        intellijIdeaCommunity(providers.gradleProperty("platformVersion"))
        testFramework(TestFrameworkType.Platform)
        pluginVerifier()
    }
    // The platform's own Jackson/Gson are not contractual API — we carry our
    // parser. 1.6.x is built with Kotlin 1.9, matching the stdlib bundled in
    // the 242 floor. The -jvm artifacts with no transitives: the IDE provides
    // the stdlib, and shipping a second copy is exactly what the Marketplace
    // verifier rejects.
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json-jvm:1.6.3") {
        isTransitive = false
    }
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-core-jvm:1.6.3") {
        isTransitive = false
    }
    testImplementation("junit:junit:4.13.2")
}

kotlin {
    jvmToolchain(21)
    compilerOptions {
        // The oldest supported IDE (242) bundles the 1.9 stdlib; compiling
        // against a newer API surface would NoSuchMethodError at runtime.
        apiVersion = KotlinVersion.KOTLIN_1_9
        // Without this, implementing a platform Kotlin interface
        // (ToolWindowFactory) generates DefaultImpls bridges that "override"
        // its @Internal members — the Plugin Verifier fails the build on
        // them. JetBrains recommends this flag for every plugin.
        freeCompilerArgs.add("-Xjvm-default=all")
    }
}

intellijPlatform {
    pluginConfiguration {
        id = "com.elumine.searchdeadcode"
        name = "SearchDeadCode"
        version = providers.gradleProperty("pluginVersion")
        ideaVersion {
            sinceBuild = providers.gradleProperty("pluginSinceBuild")
            // The Gradle plugin defaults to "242.*", which would mark the
            // plugin incompatible with every future IDE. The plugin drives an
            // external binary through stable APIs; open-ended is honest, and
            // a weekly Plugin Verifier run guards the promise.
            untilBuild = provider { null }
        }
        vendor {
            name = "elumine"
            email = "hello@elumine.ca"
            url = "https://github.com/KevinDoremy/SearchDeadCode"
        }
        changeNotes = providers.gradleProperty("pluginVersion").map { v ->
            """Tracks searchdeadcode $v — one version across every channel.
            |See the <a href="https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md">changelog</a>
            |for what changed in the analyzer.""".trimMargin()
        }
    }
    publishing {
        token = providers.environmentVariable("PUBLISH_TOKEN")
        channels = listOf("default")
    }
    pluginVerification {
        ides {
            recommended()
            // Android Studio is the audience this plugin exists for; verify
            // against its releases explicitly, not just IC equivalents.
            select {
                types = listOf(IntelliJPlatformType.AndroidStudio)
                channels = listOf(ProductRelease.Channel.RELEASE)
                sinceBuild = providers.gradleProperty("pluginSinceBuild")
            }
        }
    }
}

// Local manual testing inside the real target IDE, when one is installed.
val localAndroidStudio = sequenceOf(
    "${System.getProperty("user.home")}/Applications/Android Studio.app",
    "/Applications/Android Studio.app",
).map(::file).firstOrNull { it.exists() }

intellijPlatformTesting.runIde {
    if (localAndroidStudio != null) {
        register("runAndroidStudio") {
            localPath = localAndroidStudio
        }
    }
}

tasks.processResources {
    // The downloader pins its release asset to this exact version — never a
    // runtime "latest" lookup (this repo's latest release by date is often a
    // vscode-v* tag with no binaries).
    val cliVersion = providers.gradleProperty("pluginVersion")
    inputs.property("cliVersion", cliVersion)
    filesMatching("searchdeadcode/cli-version.properties") {
        expand("cliVersion" to cliVersion.get())
    }
}
