package com.owlmic.core.proto

import org.json.JSONObject
import java.io.File

/** The shared golden vectors in shared/protocol/vectors, the same files the PC's tests read. */
internal object Vectors {
    fun load(name: String) = JSONObject(File("../../shared/protocol/vectors/$name").readText())

    fun hex(s: String) = ByteArray(s.length / 2) { s.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
}
