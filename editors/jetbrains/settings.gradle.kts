plugins {
    // Provisions the JDK the toolchain asks for, so a bare checkout builds
    // without a locally installed JDK 21.
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

rootProject.name = "searchdeadcode-jetbrains"
