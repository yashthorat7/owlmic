import groovy.json.JsonSlurper

plugins {
    alias(libs.plugins.android.library)
}

android {
    namespace = "com.owlmic.core"
    compileSdk = 37

    defaultConfig {
        minSdk = 26
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    api(libs.kotlinx.coroutines.android)

    testImplementation(libs.junit)
    testImplementation(libs.org.json)
    testImplementation(libs.kotlinx.coroutines.test)
}

/**
 * Turns design/tokens.json, copy.json and settings.json into Kotlin constants and string resources, so the
 * phone uses exactly the colours, sizes, texts and settings the PC and the website use.
 */
abstract class GenerateDesign : DefaultTask() {
    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val designDir: DirectoryProperty

    @get:OutputDirectory
    abstract val kotlinDir: DirectoryProperty

    @get:OutputDirectory
    abstract val resDir: DirectoryProperty

    @TaskAction
    fun generate() {
        val tokens = read("tokens.json")
        val copy = read("copy.json")
        val settings = read("settings.json")

        val pkg = kotlinDir.get().asFile.resolve("com/owlmic/core/design").apply { mkdirs() }
        pkg.resolve("Tokens.kt").writeText(tokensKotlin(tokens))
        pkg.resolve("Names.kt").writeText(namesKotlin(copy))
        pkg.resolve("SettingsModel.kt").writeText(settingsKotlin(settings))

        val values = resDir.get().asFile.resolve("values").apply { mkdirs() }
        values.resolve("strings.xml").writeText(stringsXml(copy))
    }

    @Suppress("UNCHECKED_CAST")
    private fun read(name: String): Map<String, Any?> =
        JsonSlurper().parse(designDir.get().asFile.resolve(name)) as Map<String, Any?>

    @Suppress("UNCHECKED_CAST")
    private fun Map<String, Any?>.obj(key: String) = this[key] as Map<String, Any?>

    /** `notFoundHelp` → `NOT_FOUND_HELP`, `status.searching` → `STATUS_SEARCHING`. */
    private fun constant(key: String) = buildString {
        key.forEachIndexed { i, c ->
            when {
                c == '.' || c == '-' -> append('_')
                c.isUpperCase() && i > 0 && !endsWith("_") -> append('_').append(c)
                else -> append(c.uppercaseChar())
            }
        }
    }

    private fun resourceName(key: String) = constant(key).lowercase()

    private fun kotlinString(s: String) =
        "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"").replace("$", "\\$") + "\""

    private fun float(v: Any?) = (v as Number).toFloat().toString() + "f"

    private fun tokensKotlin(t: Map<String, Any?>) = buildString {
        appendLine("// Generated from design/tokens.json. Do not edit.")
        appendLine("package com.owlmic.core.design")
        appendLine()
        appendLine("data class TextSpec(val sizeSp: Float, val lineSp: Float, val weight: Int)")
        appendLine()
        appendLine("object Tokens {")
        appendLine("    object Color {")
        t.obj("color").forEach { (k, v) ->
            appendLine("        const val ${constant(k)}: Long = 0xFF${(v as String).removePrefix("#").uppercase()}")
        }
        appendLine("    }")
        appendLine("    object Text {")
        t.obj("type").forEach { (k, v) ->
            @Suppress("UNCHECKED_CAST") val s = v as Map<String, Any?>
            appendLine("        val ${constant(k)} = TextSpec(${float(s["phoneSp"])}, ${float(s["phoneLineSp"])}, ${(s["weight"] as Number).toInt()})")
        }
        appendLine("    }")
        for ((group, name) in listOf("space" to "Space", "radius" to "Radius", "motion" to "Motion", "icon" to "Icon", "phone" to "Phone")) {
            appendLine("    object $name {")
            t.obj(group).forEach { (k, v) -> appendLine("        const val ${constant(k)} = ${float(v)}") }
            appendLine("    }")
        }
        appendLine("}")
    }

    private fun namesKotlin(c: Map<String, Any?>) = buildString {
        appendLine("// Generated from design/copy.json. Do not edit.")
        appendLine("package com.owlmic.core.design")
        appendLine()
        appendLine("object Names {")
        c.obj("names").forEach { (k, v) -> appendLine("    const val ${constant(k)} = ${kotlinString(v as String)}") }
        appendLine("}")
    }

    @Suppress("UNCHECKED_CAST")
    private fun settingsKotlin(s: Map<String, Any?>) = buildString {
        appendLine("// Generated from design/settings.json. Do not edit.")
        appendLine("package com.owlmic.core.design")
        appendLine()
        appendLine("enum class Scope { SHARED, PHONE, PC }")
        appendLine()
        appendLine("class SettingDef(val id: String, val values: List<String>, val default: String, val scope: Scope, val onPhone: Boolean, val onPc: Boolean)")
        appendLine()
        appendLine("object SettingsModel {")
        appendLine("    val ALL: List<SettingDef> = listOf(")
        (s["settings"] as List<Map<String, Any?>>).forEach { d ->
            val values = (d["values"] as List<String>).joinToString(", ") { kotlinString(it) }
            val shown = d["shownOn"] as List<String>
            appendLine(
                "        SettingDef(${kotlinString(d["id"] as String)}, listOf($values), ${kotlinString(d["default"] as String)}, " +
                    "Scope.${(d["scope"] as String).uppercase()}, ${"phone" in shown}, ${"pc" in shown}),",
            )
        }
        appendLine("    )")
        appendLine()
        appendLine("    fun find(id: String): SettingDef? = ALL.firstOrNull { it.id == id }")
        appendLine("}")
    }

    /** `{pc}` style placeholders become `%1$s`, numbered by first appearance. */
    private fun androidText(text: String): String {
        val order = mutableListOf<String>()
        val withArgs = Regex("\\{(\\w+)\\}").replace(text) { m ->
            val name = m.groupValues[1]
            if (name !in order) order += name
            "%${order.indexOf(name) + 1}\$s"
        }
        return withArgs.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace("'", "\\'").replace("\"", "\\\"")
    }

    private fun stringsXml(c: Map<String, Any?>) = buildString {
        appendLine("<?xml version=\"1.0\" encoding=\"utf-8\"?>")
        appendLine("<!-- Generated from design/copy.json. Do not edit. -->")
        appendLine("<resources>")
        appendLine("    <string name=\"app_name\">${androidText(c.obj("names")["product"] as String)}</string>")
        c.obj("messages").forEach { (k, v) ->
            appendLine("    <string name=\"${resourceName(k)}\">${androidText(v as String)}</string>")
        }
        appendLine("</resources>")
    }
}

val generateDesign = tasks.register<GenerateDesign>("generateDesign") {
    designDir.set(rootProject.layout.projectDirectory.dir("../shared/design"))
    kotlinDir.set(layout.buildDirectory.dir("generated/design/kotlin"))
    resDir.set(layout.buildDirectory.dir("generated/design/res"))
}

androidComponents {
    onVariants { variant ->
        variant.sources.kotlin?.addGeneratedSourceDirectory(generateDesign, GenerateDesign::kotlinDir)
        variant.sources.res?.addGeneratedSourceDirectory(generateDesign, GenerateDesign::resDir)
    }
}
