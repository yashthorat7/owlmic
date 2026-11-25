package com.owlmic.core.proto

import org.json.JSONArray
import org.json.JSONObject

/** Control message payloads (protocol/README.md, section 4): JSON objects with camelCase keys. */
sealed interface Message {
    val kind: Int

    fun toJson(): JSONObject

    fun toPayload(): ByteArray = toJson().toString().toByteArray()

    companion object {
        /** Null for a type this version doesn't know, so newer peers can add messages. */
        fun decode(kind: Int, payload: ByteArray): Message? {
            val j = JSONObject(String(payload))
            return when (kind) {
                Kind.HELLO -> Hello(
                    j.getInt("proto"), j.getString("phoneId"), j.getString("name"), j.getString("model"),
                    j.getString("staticPub"), j.getString("ephPub"), j.getString("nonce"), j.getInt("link"), j.optStringOrNull("resume"),
                )
                Kind.HELLO_ACK -> HelloAck(
                    j.getInt("proto"), j.getString("pcId"), j.getString("name"), j.getString("staticPub"), j.getString("ephPub"),
                    j.getString("nonce"), AckStatus.parse(j.getString("status")), j.optStringOrNull("btAddr"),
                )
                Kind.PROOF -> Proof(j.getString("mac"))
                Kind.PENDING -> Pending(j.getString("code"))
                Kind.WELCOME -> Welcome(
                    j.getString("sessionId"), j.getString("mac"),
                    j.optJSONObject("settings")?.let { s -> s.keys().asSequence().associateWith { s.getString(it) } } ?: emptyMap(),
                    j.optJSONArray("caps")?.let { a -> List(a.length()) { a.getString(it) } } ?: emptyList(),
                )
                Kind.REJECT -> Reject(RejectReason.parse(j.getString("reason")), j.optStringOrNull("owner"), if (j.has("proto")) j.getInt("proto") else null)
                Kind.PING -> Ping(j.getLong("t"))
                Kind.PONG -> Pong(j.getLong("t"))
                Kind.REPORT -> Report(
                    j.getDouble("lossPct"), j.getInt("jitterMs"), j.getInt("rttMs"),
                    j.optJSONObject("kbps")?.let { k -> k.keys().asSequence().associateWith { k.getInt(it) } } ?: emptyMap(),
                    if (j.has("thermal") && !j.isNull("thermal")) j.getInt("thermal") else null,
                    if (j.has("camLossPct") && !j.isNull("camLossPct")) j.getDouble("camLossPct") else null,
                    if (j.has("camFps") && !j.isNull("camFps")) j.getInt("camFps") else null,
                    if (j.has("camDropPct") && !j.isNull("camDropPct")) j.getDouble("camDropPct") else null,
                )
                Kind.STATE -> State(
                    FeatureState.parse(j.getString("mic")), FeatureState.parse(j.getString("camera")), FeatureState.parse(j.getString("speaker")),
                )
                Kind.SETTINGS -> Settings(
                    j.getJSONArray("changes").let { a ->
                        List(a.length()) { a.getJSONObject(it).run { SettingChange(getString("id"), getString("value"), getLong("version")) } }
                    },
                )
                Kind.STREAM_START -> StreamStart(
                    j.getInt("stream"), j.getString("codec"),
                    JSONObject(j.toString()).apply { remove("stream"); remove("codec") },
                )
                Kind.STREAM_STOP -> StreamStop(j.getInt("stream"))
                Kind.KEYFRAME_REQUEST -> KeyframeRequest
                Kind.RESTART_STREAM -> RestartStream(j.getInt("stream"))
                Kind.SWITCH -> Switch(j.getInt("link"))
                Kind.BYE -> Bye(j.optStringOrNull("reason"))
                else -> null
            }
        }
    }
}

private fun JSONObject.optStringOrNull(key: String): String? = if (has(key) && !isNull(key)) getString(key) else null

private fun JSONObject.putIfNotNull(key: String, value: Any?) = apply { if (value != null) put(key, value) }

enum class AckStatus(val wire: String) {
    KNOWN("known"), NEW("new"), BUSY("busy"), BLOCKED("blocked");

    companion object {
        fun parse(s: String) = entries.first { it.wire == s }
    }
}

enum class RejectReason(val wire: String) {
    BUSY("busy"), DENIED("denied"), BLOCKED("blocked"), VERSION("version");

    companion object {
        fun parse(s: String) = entries.first { it.wire == s }
    }
}

enum class FeatureState(val wire: String) {
    OFF("off"), ON("on"), PAUSED("paused");

    companion object {
        fun parse(s: String) = entries.first { it.wire == s }
    }
}

data class Hello(
    val proto: Int,
    /** 32 hex characters. */
    val phoneId: String,
    val name: String,
    val model: String,
    /** Base64 of a 65-byte uncompressed P-256 point. */
    val staticPub: String,
    val ephPub: String,
    /** Base64 of 32 random bytes. */
    val nonce: String,
    val link: Int,
    /** The current session's id (hex), when this connection is a second link for a running session. */
    val resume: String? = null,
) : Message {
    override val kind = Kind.HELLO

    override fun toJson(): JSONObject = JSONObject().put("proto", proto).put("phoneId", phoneId).put("name", name).put("model", model)
        .put("staticPub", staticPub).put("ephPub", ephPub).put("nonce", nonce).put("link", link).putIfNotNull("resume", resume)
}

data class HelloAck(
    val proto: Int,
    val pcId: String,
    val name: String,
    val staticPub: String,
    val ephPub: String,
    val nonce: String,
    val status: AckStatus,
    val btAddr: String? = null,
) : Message {
    override val kind = Kind.HELLO_ACK

    override fun toJson(): JSONObject = JSONObject().put("proto", proto).put("pcId", pcId).put("name", name).put("staticPub", staticPub)
        .put("ephPub", ephPub).put("nonce", nonce).put("status", status.wire).putIfNotNull("btAddr", btAddr)
}

data class Proof(val mac: String) : Message {
    override val kind = Kind.PROOF

    override fun toJson(): JSONObject = JSONObject().put("mac", mac)
}

data class Pending(val code: String) : Message {
    override val kind = Kind.PENDING

    override fun toJson(): JSONObject = JSONObject().put("code", code)
}

data class Welcome(val sessionId: String, val mac: String, val settings: Map<String, String>, val caps: List<String>) : Message {
    override val kind = Kind.WELCOME

    override fun toJson(): JSONObject =
        JSONObject().put("sessionId", sessionId).put("mac", mac).put("settings", JSONObject(settings)).put("caps", JSONArray(caps))
}

/** [owner] is the phone that has the PC (busy); [proto] is the PC's own version (version). */
data class Reject(val reason: RejectReason, val owner: String? = null, val proto: Int? = null) : Message {
    override val kind = Kind.REJECT

    override fun toJson(): JSONObject = JSONObject().put("reason", reason.wire).putIfNotNull("owner", owner).putIfNotNull("proto", proto)
}

/** [t] is the sender's clock in microseconds; a [Pong] echoes it. */
data class Ping(val t: Long) : Message {
    override val kind = Kind.PING

    override fun toJson(): JSONObject = JSONObject().put("t", t)
}

data class Pong(val t: Long) : Message {
    override val kind = Kind.PONG

    override fun toJson(): JSONObject = JSONObject().put("t", t)
}

data class Report(
    val lossPct: Double,
    val jitterMs: Int,
    val rttMs: Int,
    val kbps: Map<String, Int>,
    val thermal: Int? = null,
    val camLossPct: Double? = null,
    val camFps: Int? = null,
    val camDropPct: Double? = null,
) : Message {
    override val kind = Kind.REPORT

    override fun toJson(): JSONObject = JSONObject().put("lossPct", lossPct).put("jitterMs", jitterMs).put("rttMs", rttMs)
        .put("kbps", JSONObject(kbps))
        .putIfNotNull("thermal", thermal)
        .putIfNotNull("camLossPct", camLossPct)
        .putIfNotNull("camFps", camFps)
        .putIfNotNull("camDropPct", camDropPct)
}

data class State(val mic: FeatureState, val camera: FeatureState, val speaker: FeatureState) : Message {
    override val kind = Kind.STATE

    override fun toJson(): JSONObject = JSONObject().put("mic", mic.wire).put("camera", camera.wire).put("speaker", speaker.wire)
}

data class SettingChange(val id: String, val value: String, val version: Long)

data class Settings(val changes: List<SettingChange>) : Message {
    override val kind = Kind.SETTINGS

    override fun toJson(): JSONObject = JSONObject().put(
        "changes",
        JSONArray(changes.map { JSONObject().put("id", it.id).put("value", it.value).put("version", it.version) }),
    )
}

/** [params] holds codec parameters such as the sample rate or bitrate. */
data class StreamStart(val stream: Int, val codec: String, val params: JSONObject = JSONObject()) : Message {
    override val kind = Kind.STREAM_START

    override fun toJson(): JSONObject = JSONObject(params.toString()).put("stream", stream).put("codec", codec)
}

data class StreamStop(val stream: Int) : Message {
    override val kind = Kind.STREAM_STOP

    override fun toJson(): JSONObject = JSONObject().put("stream", stream)
}

data object KeyframeRequest : Message {
    override val kind = Kind.KEYFRAME_REQUEST

    override fun toJson() = JSONObject()
}

data class RestartStream(val stream: Int) : Message {
    override val kind = Kind.RESTART_STREAM

    override fun toJson(): JSONObject = JSONObject().put("stream", stream)
}

data class Switch(val link: Int) : Message {
    override val kind = Kind.SWITCH

    override fun toJson(): JSONObject = JSONObject().put("link", link)
}

data class Bye(val reason: String? = null) : Message {
    override val kind = Kind.BYE

    override fun toJson(): JSONObject = JSONObject().putIfNotNull("reason", reason)
}
