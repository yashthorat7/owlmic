package com.owlmic

import android.view.Surface
import com.owlmic.core.hub.AppState
import com.owlmic.hub.AppHub
import com.owlmic.hub.AppMsg
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** What the UI reads and where it sends intents. The App Hub lives in [OwlmicService]; the flows outlive it. */
object Owlmic {
    internal val mutableState = MutableStateFlow(AppState())
    internal val mutableLevel = MutableStateFlow(0f)

    val state: StateFlow<AppState> = mutableState.asStateFlow()
    val level: StateFlow<Float> = mutableLevel.asStateFlow()

    @Volatile internal var hub: AppHub? = null

    fun post(message: AppMsg) {
        hub?.post(message)
    }

    /** The camera tile's preview surface while it is on screen. */
    fun setPreview(surface: Surface?, width: Int, height: Int) {
        hub?.media?.setPreview(surface, width, height)
    }

    /** Target preview crop aspect ratio (null = fill playground) and optical border width in px. */
    fun setPreviewAspect(aspect: Float?, borderPx: Int) {
        hub?.media?.setPreviewAspect(aspect, borderPx)
    }
}
