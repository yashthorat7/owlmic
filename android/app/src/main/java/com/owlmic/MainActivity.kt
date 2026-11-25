package com.owlmic

import android.content.ActivityNotFoundException
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.view.OrientationEventListener
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.mutableIntStateOf
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import com.owlmic.core.hub.AppState
import com.owlmic.core.hub.Connection
import com.owlmic.core.hub.Feature
import com.owlmic.core.hub.FeaturePhase
import com.owlmic.core.hub.LinkKind
import com.owlmic.core.permission.PermissionGate
import com.owlmic.core.permission.PermissionGate.Decision
import com.owlmic.hub.AppMsg
import com.owlmic.ui.MainScreen
import com.owlmic.ui.TURN_NONE
import com.owlmic.ui.nextTurn
import kotlinx.coroutines.launch

/**
 * The main screen's host. It starts the service (discovery begins the moment the app opens) and asks for permissions
 * just in time (section 8.1): a feature's permission on its first tap, notifications once the first feature is on,
 * Nearby devices on "Try Bluetooth". Which permissions were asked before lives in the store, through AppState.
 */
class MainActivity : ComponentActivity() {
    /**
     * What the prompt on screen is for: a feature's name, [BLUETOOTH] or [NOTIFICATIONS]. Saved with the activity,
     * because Android's prompt outlives it: the answer may reach a new activity after a rotation or a restart.
     */
    private var asking: String? = null
    private val request = registerForActivityResult(ActivityResultContracts.RequestPermission(), ::answered)

    /** How the phone is held (see Turn.kt). The screen itself stays upright; only what is drawn on it turns. */
    private val turn = mutableIntStateOf(TURN_NONE)
    private val orientation by lazy {
        object : OrientationEventListener(this) {
            override fun onOrientationChanged(degrees: Int) {
                val next = nextTurn(degrees, turn.intValue)
                if (next != turn.intValue) turn.intValue = next
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge(SystemBarStyle.dark(android.graphics.Color.BLACK), SystemBarStyle.dark(android.graphics.Color.BLACK))
        super.onCreate(savedInstanceState)
        asking = savedInstanceState?.getString(ASKING)
        setContent {
            MainScreen(
                turn.intValue,
                // The status bar goes or comes back with the screen's content, while it is faded out.
                onLayoutTurn = { showStatusBar(it == TURN_NONE) },
                onFeature = ::tap,
                onTethering = ::openTethering,
            )
        }
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) { Owlmic.state.collect(::askNotificationsOnceOn) }
        }
    }

    override fun onStart() {
        super.onStart()
        OwlmicService.open(this)
    }

    override fun onResume() {
        super.onResume()
        orientation.enable()
    }

    override fun onPause() {
        orientation.disable()
        super.onPause()
    }

    /** Sideways, the status bar goes, as in the prototype; a swipe from the edge still shows it for a moment. */
    private fun showStatusBar(show: Boolean) {
        val bars = WindowCompat.getInsetsController(window, window.decorView)
        bars.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        if (show) bars.show(WindowInsetsCompat.Type.statusBars()) else bars.hide(WindowInsetsCompat.Type.statusBars())
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putString(ASKING, asking)
    }

    private fun tap(feature: Feature) {
        val state = Owlmic.state.value
        val v = state.feature(feature)
        if (feature == Feature.CAMERA && (state.connection as? Connection.Connected)?.link == LinkKind.BLUETOOTH && !v.isOn) {
            return
        }
        if (v.phase == FeaturePhase.PAUSED && state.connected && v.problem == null) {
            Owlmic.post(AppMsg.Resume(feature))
            return
        }
        // A failed feature is tried again by the same tap that turns an off one on.
        if (v.phase != FeaturePhase.OFF && v.phase != FeaturePhase.FAILED) {
            OwlmicService.turnOff(this, feature)
            return
        }
        if (!state.connected) return
        when (val d = PermissionGate.decide(feature, granted(), state.askedPermissions, ::shouldShowRequestPermissionRationale)) {
            Decision.Proceed -> OwlmicService.turnOn(this, feature)
            is Decision.Ask -> ask(d.permission, feature.name)
            Decision.OpenSettings -> {
                Owlmic.post(AppMsg.PermissionMissing(feature))
                startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", packageName, null)))
            }
        }
    }

    /** Notifications are asked once, after a feature is really on (section 8.1), not while it is still starting. */
    private fun askNotificationsOnceOn(state: AppState) {
        if (asking != null || Feature.entries.none { state.feature(it).phase == FeaturePhase.ON }) return
        if (PermissionGate.askNotifications(Build.VERSION.SDK_INT, granted(), state.askedPermissions)) ask(PermissionGate.POST_NOTIFICATIONS, NOTIFICATIONS)
    }

    private fun tryBluetooth() {
        val permission = PermissionGate.bluetoothPermission(Build.VERSION.SDK_INT)
        if (permission == null || permission in granted()) Owlmic.post(AppMsg.TryBluetooth) else ask(permission, BLUETOOTH)
    }

    /** The tethering page of the system settings, or the network page on phones that don't expose it. */
    private fun openTethering() {
        for (action in listOf(TETHER_SETTINGS, Settings.ACTION_WIRELESS_SETTINGS)) {
            try {
                startActivity(Intent(action))
                return
            } catch (_: ActivityNotFoundException) {
                // Try the next page.
            }
        }
    }

    private fun ask(permission: String, purpose: String) {
        Owlmic.post(AppMsg.Asked(permission))
        asking = purpose
        request.launch(permission)
    }

    private fun answered(granted: Boolean) {
        val purpose = asking ?: return
        asking = null
        when (purpose) {
            NOTIFICATIONS -> Unit
            BLUETOOTH -> if (granted) Owlmic.post(AppMsg.TryBluetooth)
            else -> {
                val feature = Feature.valueOf(purpose)
                if (granted) OwlmicService.turnOn(this, feature) else Owlmic.post(AppMsg.PermissionMissing(feature))
            }
        }
    }

    private fun granted(): Set<String> = PERMISSIONS.filter { checkSelfPermission(it) == PackageManager.PERMISSION_GRANTED }.toSet()

    private companion object {
        const val ASKING = "asking"
        const val BLUETOOTH = "bluetooth"
        const val NOTIFICATIONS = "notifications"

        /** Not a public constant, but the Settings app of most phones answers it; the network page is the fallback. */
        const val TETHER_SETTINGS = "android.settings.TETHER_SETTINGS"
        val PERMISSIONS = listOf(PermissionGate.RECORD_AUDIO, PermissionGate.CAMERA, PermissionGate.POST_NOTIFICATIONS, PermissionGate.BLUETOOTH_CONNECT)
    }
}
