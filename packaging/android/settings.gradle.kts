// Android packaging lives here rather than beside the Rust crate, matching
// packaging/{flatpak,macos,windows}. The Rust side is
// `crates/harmonicon-android` (a cdylib exporting `android_main`); this
// project only compiles it via cargo-ndk and wraps the result in an APK.

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode = RepositoriesMode.FAIL_ON_PROJECT_REPOS
    repositories {
        google()
        mavenCentral()
        // The Kotlin half of rustls-platform-verifier, which content packs'
        // https downloads verify certificates through (app/build.gradle.kts).
        // Published only here, not on Maven Central; scoped to its group so
        // nothing else is ever resolved from it.
        maven {
            url = uri("https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/")
            content { includeGroup("org.rustls") }
        }
    }
}

rootProject.name = "harmonicon"
include(":app")
