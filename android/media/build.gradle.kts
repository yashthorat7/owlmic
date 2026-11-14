import java.io.File
import java.util.zip.ZipFile

plugins {
    alias(libs.plugins.android.library)
}

val oboeAar = configurations.create("oboeAar") {
    isCanBeResolved = true
    isCanBeConsumed = false
}

android {
    namespace = "com.owlmic.media"
    compileSdk = 37

    defaultConfig {
        minSdk = 26
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a")
        }
        externalNativeBuild {
            cmake {
                // Oboe ships with the shared C++ runtime, so the JNI library uses it too.
                arguments += "-DANDROID_STL=c++_shared"
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        prefab = true
    }

    // libopus, Oboe and the JNI layer. Needs CMake and the opus submodule (git submodule update --init).
    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            // Any CMake from here up, whether from the SDK manager or the PATH.
            version = "3.22.1+"
        }
    }
}

dependencies {
    implementation(project(":core"))
    api(libs.oboe)
    oboeAar(libs.oboe)
    implementation(libs.androidx.camera.core)
    implementation(libs.androidx.camera.camera2)
    implementation(libs.androidx.camera.lifecycle)
    implementation(libs.androidx.lifecycle.runtime)

    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
}

abstract class ExtractOboeJni : DefaultTask() {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val oboeAarFiles: ConfigurableFileCollection

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun extract() {
        val out = outputDir.get().asFile
        out.deleteRecursively()
        out.mkdirs()
        for (file in oboeAarFiles.files) {
            if (file.name.endsWith(".aar")) {
                ZipFile(file).use { zip ->
                    for (entry in zip.entries().asSequence()) {
                        val name = entry.name
                        if (name.startsWith("prefab/modules/oboe/libs/android.") && name.endsWith("/liboboe.so")) {
                            val parts = name.split("/")
                            val abiPart = parts.getOrNull(4)
                            val abi = abiPart?.removePrefix("android.") ?: continue
                            val dest = File(out, "$abi/liboboe.so")
                            dest.parentFile.mkdirs()
                            zip.getInputStream(entry).use { input ->
                                dest.outputStream().use { output ->
                                    input.copyTo(output)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

val extractOboeJni = tasks.register<ExtractOboeJni>("extractOboeJni") {
    oboeAarFiles.from(oboeAar)
    outputDir.set(layout.buildDirectory.dir("generated/oboeJniLibs"))
}

androidComponents {
    onVariants { variant ->
        variant.sources.jniLibs?.addGeneratedSourceDirectory(extractOboeJni, ExtractOboeJni::outputDir)
    }
}
