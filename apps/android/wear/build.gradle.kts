// Needle for Wear OS: the watch app, controlling Needle on the phone (the Wearable Data Layer).
// The same application id and signature as the phone's, so the two find each other.
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "fyi.nnx.needle.wear"
    compileSdk = 37

    defaultConfig {
        applicationId = "fyi.nnx.needle"
        // Wear OS 3 and newer.
        minSdk = 30
        targetSdk = 35
        versionCode = 7
        versionName = "1.8.0"
    }

    buildTypes {
        release {
            // Only the icons it uses: a small download for a watch.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
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
    implementation("androidx.wear.compose:compose-material3:1.7.1")
    implementation("androidx.wear.compose:compose-foundation:1.7.1")
    implementation("androidx.compose.material:material-icons-extended:1.7.8")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.core:core-ktx:1.19.1")
    // The phone: messages to it, and what plays there (the Wearable Data Layer).
    implementation("com.google.android.gms:play-services-wearable:20.0.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-play-services:1.11.0")
}
