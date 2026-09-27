plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "fyi.nnx.needle"
    compileSdk = 37

    defaultConfig {
        applicationId = "fyi.nnx.needle"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "1.6.1"
        ndk {
            // The Rust core is built for these by scripts/android-build.sh.
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(composeBom)
    // Material 3 Expressive is still marked experimental, so the newest Material 3.
    implementation("androidx.compose.material3:material3:1.5.0-alpha29")
    implementation("androidx.compose.material:material-icons-extended:1.7.8")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.core:core-ktx:1.19.1")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.11.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
    implementation("io.coil-kt.coil3:coil-compose:3.6.3")
    implementation("androidx.media3:media3-session:1.11.1")
    // The playing cover's colour for the full player.
    implementation("androidx.palette:palette-ktx:1.0.0")
    // UniFFI's Kotlin bindings call the Rust library through JNA.
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    // Scans the QR code of Needle on a computer (Google's scanner: no camera permission).
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")
}
