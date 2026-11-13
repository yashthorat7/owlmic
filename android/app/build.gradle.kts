plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "com.owlmic"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.owlmic"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"

        // The native code (Oboe, Opus) is built for phones only; Android has been 64-bit or ARMv7 on every phone since 8.0.
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a") }
    }

    signingConfigs {
        create("release") {
            val keyStorePath = System.getenv("KEYSTORE_FILE")
            val keyStoreFile = keyStorePath?.let { file(it) }
            if (keyStoreFile != null && keyStoreFile.exists()) {
                storeFile = keyStoreFile
                storePassword = System.getenv("KEYSTORE_PASSWORD")
                keyAlias = System.getenv("KEY_ALIAS")
                keyPassword = System.getenv("KEY_PASSWORD")
            } else {
                // Self-signed debug key fallback for sideloadable local release builds
                initWith(getByName("debug"))
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            signingConfig = signingConfigs.getByName("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        prefab = true
    }

    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }
}

dependencies {
    implementation(project(":core"))
    implementation(project(":media"))
    implementation(libs.oboe)
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.foundation)

    testImplementation(libs.junit)
}

/**
 * The third-party notices shown under Settings → About → Open-source licences: the texts of the bundled libopus (BSD)
 * and Geist fonts (OFL) as they ship, so the app carries the notices their licences ask for.
 */
abstract class GenerateNotices : DefaultTask() {
    @get:InputFile
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val opus: RegularFileProperty

    @get:InputFile
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val geist: RegularFileProperty

    @get:OutputDirectory
    abstract val assetsDir: DirectoryProperty

    @TaskAction
    fun generate() {
        val text = listOf("libopus" to opus, "Geist" to geist).joinToString("\n\n\n") { (name, file) ->
            name + "\n\n" + file.get().asFile.readText().trim()
        }
        assetsDir.get().asFile.apply { mkdirs() }.resolve("licences.txt").writeText(text + "\n")
    }
}

val generateNotices = tasks.register<GenerateNotices>("generateNotices") {
    opus.set(rootProject.layout.projectDirectory.file("../shared/vendor/opus/COPYING"))
    geist.set(rootProject.layout.projectDirectory.file("licenses/geist-OFL.txt"))
    assetsDir.set(layout.buildDirectory.dir("generated/notices"))
}

androidComponents {
    onVariants { variant ->
        variant.sources.assets?.addGeneratedSourceDirectory(generateNotices, GenerateNotices::assetsDir)
    }
}
