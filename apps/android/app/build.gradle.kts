plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

val packagedAbis = providers.gradleProperty("needleAbis").orNull
    ?.split(',')?.map { it.trim() } ?: listOf("arm64-v8a", "x86_64")
// armeabi-v7a (32-bit Arm, for TVs and older phones) is its own APK: -PneedleAbis=armeabi-v7a.
require(packagedAbis.isNotEmpty() && packagedAbis.all { it in listOf("arm64-v8a", "x86_64", "armeabi-v7a") }) {
    "needleAbis must contain arm64-v8a, x86_64, or armeabi-v7a, separated by commas"
}

android {
    namespace = "fyi.nnx.needle"
    compileSdk = 37

    defaultConfig {
        applicationId = "fyi.nnx.needle"
        minSdk = 26
        targetSdk = 36
        versionCode = 7
        versionName = "1.8.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk {
            // The Rust core is built for these by scripts/android-build.sh.
            abiFilters += packagedAbis
        }
    }

    buildTypes {
        debug {
            // -PneedleIdSuffix=.dev: a build that installs beside Needle from the website, which
            // has another signature.
            providers.gradleProperty("needleIdSuffix").orNull?.let { applicationIdSuffix = it }
        }
        release {
            isMinifyEnabled = false
        }
    }

    packaging {
        // Library files are unpacked on the phone, so Needle's Dolby decoder (a program, named
        // libneedle_ffmpeg.so) can be run from there.
        jniLibs {
            useLegacyPackaging = true
        }
    }

    buildFeatures {
        compose = true
    }

    // The website's Privacy and Help pages ship inside the app, read in its own pages.
    sourceSets["main"].assets.directories.add(layout.buildDirectory.dir("generated/site-assets").get().asFile.path)
    // Needle's fonts and their licences, from assets/fonts (see copyFonts).
    sourceSets["main"].res.directories.add(layout.buildDirectory.dir("generated/needle-fonts-res").get().asFile.path)
    sourceSets["main"].assets.directories.add(layout.buildDirectory.dir("generated/font-licences").get().asFile.path)

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    val composeBom = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(composeBom)
    // Material 3 Expressive is still marked experimental, so the newest Material 3.
    implementation("androidx.compose.material3:material3:1.5.0-alpha29")
    implementation("androidx.compose.material:material-icons-extended:1.7.8")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.activity:activity-compose:1.13.0")
    // The QR scanner requests Fragment 1.0.0; result launchers require 1.3 or newer.
    implementation("androidx.fragment:fragment:1.9.1")
    implementation("androidx.core:core-ktx:1.19.1")
    // The splash screen, the same on every Android version.
    implementation("androidx.core:core-splashscreen:1.2.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.11.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
    implementation("io.coil-kt.coil3:coil-compose:3.6.3")
    // Covers from Needle on a computer, over the home network.
    implementation("io.coil-kt.coil3:coil-network-okhttp:3.6.3")
    implementation("androidx.media3:media3-session:1.11.1")
    // The playing cover's colour for the full player.
    implementation("androidx.palette:palette-ktx:1.0.0")
    // A full Material colour scheme from one cover colour (Material's own colour maths).
    implementation("com.materialkolor:material-color-utilities:5.0.1")
    // UniFFI's Kotlin bindings call the Rust library through JNA.
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    // Scans the QR code of Needle on a computer (Google's scanner: no camera permission).
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")
    // Frosted glass for the dock and the floating player: the page behind blurred (Android 12+).
    implementation("dev.chrisbanes.haze:haze:2.0.1")
    implementation("dev.chrisbanes.haze:haze-blur:2.0.1")
    // Lyrics translated on the phone (Google's ML Kit: the language found, then translated
    // on the device; each language's model downloads once, on Wi-Fi).
    implementation("com.google.mlkit:translate:17.0.3")
    implementation("com.google.mlkit:language-id:17.0.6")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-play-services:1.11.0")
    // Needle on a Wear OS watch (the wear app): what plays, to the watch; its presses, back.
    implementation("com.google.android.gms:play-services-wearable:20.0.1")
    // Home screen widgets.
    implementation("androidx.glance:glance-appwidget:1.2.0")
    implementation("androidx.glance:glance-material3:1.2.0")
    // Discord's library, for the Discord status: closed source, so not in git (see MOBILE.md).
    // Without it the app builds the same, with no Discord status.
    val discordSdk = file("libs/discord_partner_sdk.aar")
    if (discordSdk.exists()) implementation(files(discordSdk))
}

// The fonts every Needle has (assets/fonts, fetched by scripts/fetch-fonts.py), as Android
// resources (lowercase names: Inter-400.ttf is R.font.inter_400), and their licences in the
// app's assets. Poster stays Roboto Flex on Android (res/font/roboto_flex.ttf).
val copyFonts by tasks.registering(Copy::class) {
    from(rootProject.file("../../assets/fonts")) {
        include("*.ttf")
        exclude("Poster-*")
        rename { it.lowercase().replace(Regex("[^a-z0-9.]"), "_") }
    }
    into(layout.buildDirectory.dir("generated/needle-fonts-res/font"))
}
val copyFontLicences by tasks.registering(Copy::class) {
    from(rootProject.file("../../assets/fonts")) {
        include("*-OFL.txt")
        exclude("Poster-*")
    }
    into(layout.buildDirectory.dir("generated/font-licences/licenses"))
}
tasks.named("preBuild") { dependsOn(copyFonts, copyFontLicences) }

// Copies the website's Privacy and Help pages in, so the app always has the current words.
val copySitePages by tasks.registering(Copy::class) {
    from(rootProject.file("../../website/public")) { include("privacy.html", "help.html") }
    into(layout.buildDirectory.dir("generated/site-assets/site"))
}
tasks.named("preBuild") { dependsOn(copySitePages) }
