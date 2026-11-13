package com.owlmic

import android.annotation.SuppressLint
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import com.owlmic.core.hub.AppState
import com.owlmic.core.hub.Connection
import com.owlmic.core.hub.Feature
import com.owlmic.core.hub.FeaturePhase
import com.owlmic.core.hub.LinkKind
import com.owlmic.core.hub.SpeakerOutput
import com.owlmic.core.link.WifiLatencyLock
import com.owlmic.hub.AppHub
import com.owlmic.hub.AppMsg

/**
 * Owns the App Hub (section 12.1). Started when the app opens, so discovery begins at once; foreground, with exactly the
 * types in use, while a feature is on. Closing the app from Recents ends it (stopWithTask), and everything stops.
 */
class OwlmicService : Service() {
    private val main = Handler(Looper.getMainLooper())
    private lateinit var hub: AppHub
    private lateinit var notifier: Notifier
    private lateinit var wifi: WifiLatencyLock
    private var foreground: Set<Feature>? = null
    private var state = AppState()

    private val thermalListener = PowerManager.OnThermalStatusChangedListener { hub.post(AppMsg.Thermal(it)) }

    private val devices = object : AudioDeviceCallback() {
        override fun onAudioDevicesAdded(added: Array<out AudioDeviceInfo>) = output()

        override fun onAudioDevicesRemoved(removed: Array<out AudioDeviceInfo>) = output()
    }

    override fun onCreate() {
        super.onCreate()
        notifier = Notifier(this)
        wifi = WifiLatencyLock(this)
        hub = AppHub(application) { s -> main.post { changed(s) } }
        Owlmic.hub = hub
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) getSystemService(PowerManager::class.java).addThermalStatusListener(mainExecutor, thermalListener)
        getSystemService(AudioManager::class.java).registerAudioDeviceCallback(devices, main)
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val feature = intent?.getStringExtra(EXTRA_FEATURE)?.let { Feature.valueOf(it) }
        when (intent?.action) {
            ACTION_OPEN -> hub.post(AppMsg.Opened)
            ACTION_ON -> {
                // Android 14+ requires the service to be foreground with the feature's type before capture starts.
                when {
                    feature == null -> keepForegroundPromise()
                    enterForeground(inUse(state) + feature) -> hub.post(AppMsg.SetFeature(feature, true))
                    else -> {
                        // Not allowed with that type now (the permission was taken back): say so in the tile.
                        hub.post(AppMsg.PermissionMissing(feature))
                        keepForegroundPromise()
                    }
                }
                // A feature the hub refused (the PC went away meanwhile) changes no state, so check once it has had its say.
                main.postDelayed(::leaveForegroundIfIdle, IDLE_CHECK_MS)
            }
            ACTION_OFF -> if (feature != null) hub.post(AppMsg.SetFeature(feature, false))
            ACTION_PAUSE_MIC -> hub.post(AppMsg.ToggleMicPause)
            ACTION_STOP_ALL -> hub.post(AppMsg.StopAll)
        }
        // Not sticky: if Android kills Owlmic, nothing turns back on by itself.
        return START_NOT_STICKY
    }

    private fun inUse(s: AppState) = Feature.entries.filter { s.feature(it).phase != FeaturePhase.OFF && s.feature(it).phase != FeaturePhase.FAILED }.toSet()

    private fun changed(s: AppState) {
        state = s
        val c = s.connection
        val features = inUse(s)
        // Wi-Fi power save only costs latency while something streams over it.
        if (c is Connection.Connected && c.link == LinkKind.WIFI && features.isNotEmpty()) wifi.hold() else wifi.release()
        if (features.isEmpty()) {
            leaveForegroundIfIdle()
        } else {
            if (features != foreground) enterForeground(features)
            notifier.show(s, features)
        }
    }

    /** Foreground with only the types in use, so Android shows the right indicators. False when Android won't allow it now. */
    @SuppressLint("InlinedApi") // Android 10 ignores the mic and camera bits; the manifest declares all three types.
    private fun enterForeground(features: Set<Feature>): Boolean {
        return try {
            val notification = notifier.build(state, features)
            var types = 0
            if (Feature.MIC in features) types = types or ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
            if (Feature.CAMERA in features) types = types or ServiceInfo.FOREGROUND_SERVICE_TYPE_CAMERA
            if (Feature.SPEAKER in features) types = types or ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                if (types == 0) types = ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK
                startForeground(Notifier.ID, notification, types)
            } else {
                startForeground(Notifier.ID, notification)
            }
            foreground = features
            true
        } catch (_: Throwable) {
            false
        }
    }

    /**
     * Every startForegroundService must be followed by startForeground, or Android ends the app. When the feature's own
     * type is refused, a brief media playback foreground (which needs no permission) keeps the promise, and goes again.
     */
    @SuppressLint("InlinedApi")
    private fun keepForegroundPromise() {
        if (foreground != null) return
        runCatching {
            val notification = notifier.build(state, emptySet())
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                startForeground(Notifier.ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
            } else {
                startForeground(Notifier.ID, notification)
            }
            stopForeground(STOP_FOREGROUND_REMOVE)
        }
    }

    private fun leaveForegroundIfIdle() {
        if (foreground == null || inUse(state).isNotEmpty()) return
        stopForeground(STOP_FOREGROUND_REMOVE)
        foreground = null
    }

    private fun output() {
        val outputs = getSystemService(AudioManager::class.java).getDevices(AudioManager.GET_DEVICES_OUTPUTS).map { it.type }
        val output = when {
            outputs.any { it == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP || it == AudioDeviceInfo.TYPE_BLUETOOTH_SCO || it == TYPE_BLE_HEADSET } -> SpeakerOutput.BLUETOOTH
            outputs.any { it == AudioDeviceInfo.TYPE_WIRED_HEADPHONES || it == AudioDeviceInfo.TYPE_WIRED_HEADSET || it == AudioDeviceInfo.TYPE_USB_HEADSET } -> SpeakerOutput.HEADPHONES
            else -> SpeakerOutput.SPEAKER
        }
        hub.post(AppMsg.Output(output))
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        stopSelf()
    }

    override fun onDestroy() {
        getSystemService(AudioManager::class.java).unregisterAudioDeviceCallback(devices)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) getSystemService(PowerManager::class.java).removeThermalStatusListener(thermalListener)
        hub.close()
        Owlmic.hub = null
        Owlmic.mutableState.value = AppState()
        Owlmic.mutableLevel.value = 0f
        wifi.release()
        main.removeCallbacksAndMessages(null)
        super.onDestroy()
    }

    companion object {
        const val ACTION_OPEN = "com.owlmic.action.OPEN"
        const val ACTION_ON = "com.owlmic.action.ON"
        const val ACTION_OFF = "com.owlmic.action.OFF"
        const val ACTION_PAUSE_MIC = "com.owlmic.action.PAUSE_MIC"
        const val ACTION_STOP_ALL = "com.owlmic.action.STOP_ALL"
        private const val EXTRA_FEATURE = "feature"
        private const val IDLE_CHECK_MS = 1_000L

        /** AudioDeviceInfo.TYPE_BLE_HEADSET, Android 12+. */
        private const val TYPE_BLE_HEADSET = 26

        /** From the app on screen: discovery starts now. */
        fun open(context: Context) {
            context.startService(Intent(context, OwlmicService::class.java).setAction(ACTION_OPEN))
        }

        /** Only from the app on screen: Android refuses to start a mic or camera service from the background. */
        fun turnOn(context: Context, feature: Feature) {
            context.startForegroundService(Intent(context, OwlmicService::class.java).setAction(ACTION_ON).putExtra(EXTRA_FEATURE, feature.name))
        }

        fun turnOff(context: Context, feature: Feature) {
            context.startService(Intent(context, OwlmicService::class.java).setAction(ACTION_OFF).putExtra(EXTRA_FEATURE, feature.name))
        }

        internal fun intent(context: Context, action: String) = Intent(context, OwlmicService::class.java).setAction(action)
    }
}
