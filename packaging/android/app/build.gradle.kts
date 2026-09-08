import java.util.Properties

plugins {
    id("com.android.application")
}

// Repo root, two levels up from packaging/android.
val repoRoot: File = rootProject.projectDir.parentFile.parentFile

// Where cargo-ndk drops `lib<name>.so`, one subdirectory per ABI. Under
// build/ rather than src/, because it is generated output.
val rustJniLibs: File = layout.buildDirectory.dir("rustJniLibs").get().asFile

// Must match the NDK below and the `-P` platform handed to cargo-ndk:
// `libaaudio.so` (cpal's Android backend links it) only exists in the NDK
// sysroot from API 26 up, so cargo-ndk's default of 21 fails to link with a
// bare "unable to find library -laaudio".
val minSdkVersion = 28
val ndkVersionUsed = "28.2.13676358"

// Which ABIs to build. Defaults to arm64-v8a alone — every phone worth
// targeting is arm64, and each extra ABI means another full Rust build plus
// ~108 MB of APK. Override for the emulator, which is x86_64 on a desktop:
//
//     ./gradlew installDebug -Pharmonicon.abis=x86_64
//
// Comma-separated for more than one.
val abis: List<String> =
    (findProperty("harmonicon.abis") as String? ?: "arm64-v8a")
        .split(",")
        .map(String::trim)
        .filter(String::isNotEmpty)

/** SDK location, the way AGP itself resolves it. `android.sdkDirectory` was
 *  removed in AGP 9, so read local.properties then fall back to the env. */
val sdkDir: File = run {
    val props = File(rootProject.projectDir, "local.properties")
    val fromProps = if (props.exists()) {
        Properties().apply { props.inputStream().use { load(it) } }.getProperty("sdk.dir")
    } else {
        null
    }
    val path = fromProps
        ?: System.getenv("ANDROID_HOME")
        ?: System.getenv("ANDROID_SDK_ROOT")
        ?: error("Set ANDROID_HOME, or sdk.dir in packaging/android/local.properties")
    File(path)
}

android {
    namespace = "io.github.tcanabrava.harmonicon"
    compileSdk = 35
    ndkVersion = ndkVersionUsed

    defaultConfig {
        applicationId = "io.github.tcanabrava.harmonicon"
        minSdk = minSdkVersion
        targetSdk = 35
        versionCode = 11
        versionName = "0.0.11"
        ndk { abiFilters.addAll(abis) }
    }

    sourceSets.named("main") {
        // assets/ belongs to the root package and is shared with every other
        // platform; nothing is copied into packaging/.
        assets.directories.add(File(repoRoot, "assets").absolutePath)
        jniLibs.directories.add(rustJniLibs.absolutePath)
    }

    // Dev-only content with no business in a shipped APK: `debug_songs` is
    // note_bench's synthetic benchmark corpus (see PLAN.md).
    androidResources {
        ignoreAssetsPatterns.add("debug_songs")
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // Debug-signed, so `assembleRelease` yields an installable APK
            // without anyone needing a keystore first. Replace before any
            // real distribution.
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    // The Java half of GameActivity. The version is NOT free to choose: the
    // C++ half is vendored inside the `android-activity` crate, whose
    // GameActivity.h declares GAMEACTIVITY_MAJOR/MINOR/BUGFIX_VERSION 4/4/0.
    // A mismatch fails at *runtime* (RegisterNatives aborts), not at build
    // time, so re-check those defines before bumping android-activity.
    implementation("androidx.games:games-activity:4.4.0")
    // NOT transitive, and its absence is invisible until launch: the
    // games-activity POM declares no dependencies at all, yet GameActivity
    // extends androidx.appcompat.app.AppCompatActivity. Without this the
    // class is present in classes.dex but cannot resolve, and the app dies
    // with ClassNotFoundException for GameActivity itself — the real cause
    // only showing up as a *suppressed* NoClassDefFoundError for
    // AppCompatActivity.
    implementation("androidx.appcompat:appcompat:1.7.1")
}

/**
 * Builds the Rust cdylib for each wanted ABI into [rustJniLibs].
 *
 * `-P 28` is the Android platform; the `-p harmonicon-android` after `build`
 * is cargo's own package flag — cargo-ndk spells platform `-P` precisely so
 * the two don't collide (passing `-p 28` gets you "unknown package: 28").
 *
 * That `-p` also has to name `harmonicon-android` specifically, and the
 * Android-only Bevy feature selection has to live in *that* crate's
 * Cargo.toml, or feature unification silently drops it.
 */
val cargoNdkBuild = tasks.register<Exec>("cargoNdkBuild") {
    group = "build"
    description = "Compile the Rust cdylib via cargo-ndk into jniLibs"
    workingDir = repoRoot
    environment("ANDROID_NDK_HOME", File(sdkDir, "ndk/$ndkVersionUsed").absolutePath)
    commandLine(
        buildList {
            add("cargo"); add("ndk")
            abis.forEach { add("-t"); add(it) }
            add("-P"); add(minSdkVersion.toString())
            add("-o"); add(rustJniLibs.absolutePath)
            add("build"); add("--release")
            add("-p"); add("harmonicon-android")
        }
    )
}

// Hook every variant's jniLibs merge, so assembleDebug/assembleRelease/
// installDebug all rebuild the Rust side first.
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("JniLibFolders") }
    .configureEach { dependsOn(cargoNdkBuild) }
