package com.owlmic

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.graphics.drawable.Icon
import com.owlmic.core.hub.AppState
import com.owlmic.core.hub.Connection
import com.owlmic.core.hub.Feature
import com.owlmic.core.hub.FeaturePhase
import com.owlmic.core.R as Copy

/** The notification while a feature is on (section 32): what is on and where, Pause or Resume mic, Stop all. Silent. */
class Notifier(private val service: Service) {
    private val manager = service.getSystemService(NotificationManager::class.java)

    /** The PC the features were last on, for the moments between links when the state names none. */
    private var lastPc = ""

    init {
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, service.getString(Copy.string.notification_channel), NotificationManager.IMPORTANCE_DEFAULT).apply {
                setSound(null, null)
                enableVibration(false)
                setShowBadge(false)
            },
        )
    }

    fun build(state: AppState, features: Set<Feature>): Notification {
        val names = joinNames(
            Feature.entries.filter { it in features }.map {
                service.getString(
                    when (it) {
                        Feature.MIC -> Copy.string.feature_mic
                        Feature.CAMERA -> Copy.string.feature_camera
                        Feature.SPEAKER -> Copy.string.feature_speaker
                    },
                )
            },
            service.getString(Copy.string.ui_and),
        )
        val pc = when (val c = state.connection) {
            is Connection.Connected -> c.pc
            is Connection.Reconnecting -> c.pc
            is Connection.Waiting -> c.pc
            else -> lastPc
        }
        lastPc = pc
        val builder = Notification.Builder(service, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(service.getString(Copy.string.app_name))
            .setContentText(service.getString(Copy.string.notification_text, names, pc))
            .setContentIntent(PendingIntent.getActivity(service, 0, Intent(service, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE))
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setVisibility(Notification.VISIBILITY_PUBLIC)
        if (Feature.MIC in features) {
            val paused = state.mic.phase == FeaturePhase.PAUSED
            builder.addAction(action(if (paused) Copy.string.notification_resume_mic else Copy.string.notification_pause_mic, OwlmicService.ACTION_PAUSE_MIC, 1))
        }
        builder.addAction(action(Copy.string.notification_stop_all, OwlmicService.ACTION_STOP_ALL, 2))
        return builder.build()
    }

    fun show(state: AppState, features: Set<Feature>) {
        manager.notify(ID, build(state, features))
    }

    private fun action(label: Int, action: String, code: Int) = Notification.Action.Builder(
        Icon.createWithResource(service, R.drawable.ic_notification),
        service.getString(label),
        PendingIntent.getService(service, code, OwlmicService.intent(service, action), PendingIntent.FLAG_IMMUTABLE),
    ).build()

    companion object {
        const val ID = 1

        /** "Mic", "Mic and Camera", "Mic, Camera and Speaker": the same joining as the PC tray. */
        fun joinNames(names: List<String>, and: String): String =
            if (names.size <= 1) names.firstOrNull().orEmpty() else names.dropLast(1).joinToString(", ") + " $and " + names.last()
        private const val CHANNEL_ID = "owlmic_live"
    }
}
