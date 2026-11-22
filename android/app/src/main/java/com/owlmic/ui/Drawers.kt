package com.owlmic.ui

import android.text.format.DateUtils
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.owlmic.Owlmic
import com.owlmic.core.design.Tokens
import com.owlmic.core.hub.AppState
import com.owlmic.core.hub.Connection
import com.owlmic.core.hub.LinkKind
import com.owlmic.hub.AppMsg
import kotlinx.coroutines.launch
import com.owlmic.core.R as Copy

/** The pages that open over the preview from the top bar: the PCs and links, and setup and help. */
enum class Drawer { CONNECTION, HELP }

/**
 * Slides down from the top bar, where its tile is, and fades in. Content only: no header; the tile that opened it, or
 * Back, closes it.
 */
@Composable
fun AnimatedDrawer(seq: TurnSequence, drawer: Drawer?, state: AppState) {
    var shown by remember { mutableStateOf(drawer ?: Drawer.CONNECTION) }
    if (drawer != null) shown = drawer
    val ease = CubicBezierEasing(0.16f, 1f, 0.3f, 1f)
    val slide = Tokens.Phone.DRAWER_SLIDE_MS.toInt()
    val fade = Tokens.Phone.DRAWER_FADE_MS.toInt()
    AnimatedVisibility(
        drawer != null,
        Modifier.fillMaxSize(),
        enter = slideInVertically(tween(slide, easing = ease)) { -it } + fadeIn(tween(fade)),
        exit = slideOutVertically(tween(slide, easing = ease)) { -it } + fadeOut(tween(fade)),
    ) {
        val fading by seq.fade
        Column(Modifier.fillMaxSize().background(Theme.bg).upright(seq.layout, swap = true).graphicsLayer { alpha = fading }) {
            when (shown) {
                Drawer.CONNECTION -> ConnectionDrawer(state)
                Drawer.HELP -> InfoDrawer()
            }
        }
    }
}

/**
 * The PCs: the connected one first, then the others this phone used, new ones that answered, and manual addresses;
 * blocked PCs below a line. Includes inline sync-beam, inline Add Host row, and 6-column transport footer.
 */
@Composable
private fun ColumnScope.ConnectionDrawer(state: AppState) {
    val list = pcList(state)
    var opened by rememberSaveable { mutableStateOf<String?>(null) }
    val toggle = { key: String -> opened = if (opened == key) null else key }

    var showAdd by remember { mutableStateOf(false) }
    var addAddressText by remember { mutableStateOf("") }

    Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).animateContentSize()) {
        if (list.rows.isEmpty()) {
            BasicText(stringResource(Copy.string.phone_no_pcs), Modifier.padding(20.dp), style = Theme.pageDetail)
        }
        list.rows.forEach { row -> PcEntry(state, row, opened == row.key) { toggle(row.key) } }

        // Inline Add Host
        if (!showAdd) {
            Box(
                Modifier
                    .fillMaxWidth()
                    .height(48.dp)
                    .drawBehind { drawLine(Color(0x1AFFFFFF), Offset(0f, size.height - 1f), Offset(size.width, size.height - 1f), 1f) }
                    .clickable(role = Role.Button, onClick = { showAdd = true }),
                contentAlignment = Alignment.Center,
            ) {
                Icon(Glyph.PLUS, Color.White.copy(alpha = 0.40f), stringResource(Copy.string.phone_add_pc), size = 18.dp, stroke = 2.15f)
            }
        } else {
            Row(
                Modifier
                    .fillMaxWidth()
                    .height(49.dp)
                    .drawBehind { drawLine(Color(0x1AFFFFFF), Offset(0f, size.height - 1f), Offset(size.width, size.height - 1f), 1f) }
                    .padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Box(Modifier.weight(1f)) {
                    if (addAddressText.isEmpty()) {
                        BasicText(
                            stringResource(Copy.string.phone_address_hint),
                            style = Theme.pageDetail.copy(color = Color(0x4DFFFFFF), fontSize = 15.sp),
                        )
                    }
                    BasicTextField(
                        value = addAddressText,
                        onValueChange = { addAddressText = it },
                        modifier = Modifier.fillMaxWidth(),
                        textStyle = Theme.pageText.copy(color = Color.White, fontSize = 15.sp, fontFeatureSettings = "tnum"),
                        cursorBrush = SolidColor(Color.White),
                        singleLine = true,
                    )
                }
                Box(
                    Modifier
                        .size(32.dp)
                        .clickable(role = Role.Button, onClick = { showAdd = false; addAddressText = "" }),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(Glyph.CLOSE, Color.White.copy(alpha = 0.40f), stringResource(Copy.string.ui_cancel), size = 16.dp, stroke = 2.3f)
                }
                Box(
                    Modifier
                        .size(32.dp)
                        .clickable(role = Role.Button, onClick = {
                            val trimmed = addAddressText.trim()
                            if (trimmed.isNotEmpty()) {
                                Owlmic.post(AppMsg.AddAddress(trimmed))
                                addAddressText = ""
                                showAdd = false
                            }
                        }),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(Glyph.ARROW, Color.White.copy(alpha = 0.70f), stringResource(Copy.string.ui_add), size = 16.dp, stroke = 2f)
                }
            }
        }

        if (list.blocked.isNotEmpty()) {
            Box(Modifier.fillMaxWidth().padding(top = 16.dp).height(1.dp).background(Color(0x26FFFFFF)))
            list.blocked.forEach { row -> PcEntry(state, row, opened == row.key) { toggle(row.key) } }
        }
    }
    Footer(state)
}

@Composable
private fun PcEntry(state: AppState, row: PcRow, open: Boolean, onToggle: () -> Unit) {
    val name = row.name?.takeIf { it.isNotEmpty() } ?: if (row.kind == PcKind.ADDRESS) stringResource(Copy.string.phone_pc_direct_ip) else row.address.orEmpty()
    val connected = stringResource(Copy.string.ui_connected)
    val isCurrent = row.kind == PcKind.CURRENT
    val isPairing = (isCurrent || row.canConnect) && (state.connection is Connection.Connecting || state.connection is Connection.Approving || state.connection is Connection.Reconnecting)

    val infiniteTransition = rememberInfiniteTransition(label = "syncBeam")
    val beamProgress by infiniteTransition.animateFloat(
        initialValue = -0.5f,
        targetValue = 1.5f,
        animationSpec = infiniteRepeatable(
            animation = tween(2100, easing = CubicBezierEasing(0.4f, 0f, 0.2f, 1f)),
            repeatMode = RepeatMode.Restart,
        ),
        label = "beamP",
    )
    val iconAlpha by infiniteTransition.animateFloat(
        initialValue = 0.35f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(1100, easing = CubicBezierEasing(0.4f, 0f, 0.2f, 1f)),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "pulseIc",
    )

    Column(
        Modifier
            .fillMaxWidth()
            .drawBehind {
                val y = size.height - 0.5.dp.toPx()
                drawLine(Color(0x1AFFFFFF), Offset(0f, y), Offset(size.width, y), 1.dp.toPx())
                if (isPairing) {
                    val bw1 = size.width * 0.5f
                    val x1 = (size.width * beamProgress) - bw1 / 2f
                    drawRect(
                        brush = Brush.linearGradient(
                            colors = listOf(Color.Transparent, Color.White.copy(alpha = 0.95f), Color.Transparent),
                            start = Offset(x1, 0f),
                            end = Offset(x1 + bw1, 0f),
                        ),
                        topLeft = Offset(x1, size.height - 1.5.dp.toPx()),
                        size = Size(bw1, 1.5.dp.toPx()),
                    )
                    val bw2 = size.width * 0.35f
                    val x2 = (size.width * (beamProgress - 0.25f)) - bw2 / 2f
                    drawRect(
                        brush = Brush.linearGradient(
                            colors = listOf(Color.Transparent, Color.White.copy(alpha = 0.72f), Color.Transparent),
                            start = Offset(x2, 0f),
                            end = Offset(x2 + bw2, 0f),
                        ),
                        topLeft = Offset(x2, size.height - 1.5.dp.toPx()),
                        size = Size(bw2, 1.5.dp.toPx()),
                    )
                }
            },
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .semantics { if (isCurrent) stateDescription = connected }
                .clickable(role = Role.Button, onClick = onToggle)
                .padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            val linkIc = row.link?.let { glyphOf(it) } ?: Glyph.GLOBE
            Icon(
                linkIc,
                Color.White,
                null,
                Modifier.size(19.dp).graphicsLayer { alpha = if (isPairing) iconAlpha else 1f },
                stroke = 2f,
            )
            Column(Modifier.weight(1f)) {
                BasicText(name, style = Theme.pageName.copy(fontSize = 16.5.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                row.address?.takeIf { it.isNotEmpty() }?.let { addr ->
                    BasicText(
                        addr,
                        Modifier.padding(top = 2.dp),
                        style = Theme.pageDetail.copy(color = Color(0x73FFFFFF), fontSize = 14.sp, fontFeatureSettings = "tnum"),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            if (isCurrent) {
                Icon(Glyph.CHECK, Color.White, null, size = 18.dp, stroke = 2.6f)
            } else if (row.canConnect && row.id != null) {
                TextAction(stringResource(Copy.string.ui_connect)) { Owlmic.post(AppMsg.ChoosePc(row.id)) }
            }
        }
        AnimatedVisibility(open, enter = expandVertically() + fadeIn(), exit = shrinkVertically() + fadeOut()) {
            Column(Modifier.fillMaxWidth().padding(start = 49.dp, end = 16.dp, bottom = 14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    BasicText(
                        if (row.kind == PcKind.ADDRESS) "" else lastConnected(row),
                        Modifier.weight(1f),
                        style = Theme.pageDetail.copy(color = Theme.text2, fontSize = 14.sp),
                        maxLines = 1,
                    )
                    val id = row.id
                    when {
                        row.kind == PcKind.ADDRESS -> row.address?.let { a -> TextAction(stringResource(Copy.string.ui_remove)) { Owlmic.post(AppMsg.RemoveAddress(a)) } }
                        id == null -> Unit
                        row.kind == PcKind.BLOCKED -> TextAction(stringResource(Copy.string.ui_unblock)) { Owlmic.post(AppMsg.Unblock(id)) }
                        else -> TextAction(stringResource(Copy.string.ui_block)) { Owlmic.post(AppMsg.Block(id, row.name.orEmpty())) }
                    }
                }
            }
        }
    }
}

@Composable
private fun lastConnected(row: PcRow): String = when {
    row.kind == PcKind.CURRENT -> stringResource(Copy.string.phone_pc_now)
    row.lastConnectedAt <= 0 -> stringResource(Copy.string.phone_pc_never)
    else -> DateUtils.getRelativeTimeSpanString(row.lastConnectedAt, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS).toString()
}

@Composable
private fun TextAction(text: String, onClick: () -> Unit) {
    BasicText(
        text,
        Modifier.clip(RoundedCornerShape(6.dp)).clickable(role = Role.Button, onClick = onClick).padding(vertical = 8.dp),
        style = Theme.pageText.copy(fontSize = 15.sp, fontWeight = FontWeight.SemiBold),
    )
}

fun linkName(link: LinkKind) = when (link) {
    LinkKind.USB_DEBUGGING -> Copy.string.setting_link_usb_debugging
    LinkKind.USB_TETHERING -> Copy.string.setting_link_usb_tethering
    LinkKind.WIFI -> Copy.string.setting_link_wifi
    LinkKind.BLUETOOTH -> Copy.string.setting_link_bluetooth
}

/**
 * 6-column transport footer: Refresh button + divider + 4 transport toggles (USB, Tether, Wi-Fi, Bluetooth).
 */
@Composable
private fun Footer(state: AppState) {
    Row(
        Modifier
            .fillMaxWidth()
            .drawBehind { drawLine(Color(0x26FFFFFF), Offset(0f, 0.5.dp.toPx()), Offset(size.width, 0.5.dp.toPx()), 1.dp.toPx()) }
            .padding(horizontal = 10.dp, vertical = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val spin = remember { Animatable(0f) }
        val scope = rememberCoroutineScope()
        Box(
            Modifier
                .weight(1f)
                .height(44.dp)
                .clip(RoundedCornerShape(8.dp))
                .background(Color.Black)
                .border(1.dp, Color(0x33FFFFFF), RoundedCornerShape(8.dp))
                .clickable(role = Role.Button, onClick = {
                    Owlmic.post(AppMsg.Reconnect)
                    scope.launch {
                        spin.snapTo(0f)
                        spin.animateTo(360f, tween(420))
                    }
                }),
            contentAlignment = Alignment.Center,
        ) {
            Icon(Glyph.REFRESH, Color.White.copy(alpha = 0.80f), stringResource(Copy.string.ui_refresh), Modifier.graphicsLayer { rotationZ = spin.value }, size = Tokens.Phone.LINK_GLYPH_DP.dp, stroke = 2f)
        }
        Box(Modifier.width(1.dp).height(32.dp).background(Color(0x26FFFFFF)))
        LINK_SETTINGS.forEach { (link, id) ->
            val on = (state.settings[id] ?: "on") == "on"
            val name = stringResource(linkName(link))
            val desc = "$name, ${stringResource(if (on) Copy.string.value_on else Copy.string.value_off)}"
            val bg = if (on) Color(0x1AFFFFFF) else Color.Black
            val border = if (on) Color(0x3DFFFFFF) else Color(0x24FFFFFF)
            val fg = if (on) Color.White else Color.White.copy(alpha = 0.42f)
            Box(
                Modifier
                    .weight(1f)
                    .height(44.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(bg)
                    .border(1.dp, border, RoundedCornerShape(8.dp))
                    .semantics { contentDescription = desc }
                    .clickable(role = Role.Switch, onClick = { Owlmic.post(AppMsg.Setting(id, if (on) "off" else "on")) }),
                contentAlignment = Alignment.Center,
            ) {
                Icon(glyphOf(link), fg, null, size = Tokens.Phone.LINK_GLYPH_DP.dp, stroke = 2f)
            }
        }
    }
}

private enum class InfoPage { GRID, HOW_TO, PRIVACY, CREDITS }

@Composable
private fun ColumnScope.InfoDrawer() {
    var page by rememberSaveable { mutableStateOf(InfoPage.GRID) }
    BackHandler(page != InfoPage.GRID) { page = InfoPage.GRID }
    AnimatedContent(
        page,
        Modifier.weight(1f).fillMaxWidth(),
        transitionSpec = { fadeIn(tween(Theme.STATE_MS * 2)) togetherWith fadeOut(tween(Theme.STATE_MS)) },
        label = "info",
    ) { p ->
        when (p) {
            InfoPage.GRID -> InfoGrid { page = it }
            InfoPage.HOW_TO -> SubPage(Copy.string.ui_how_to, { page = InfoPage.GRID }) { HowTo() }
            InfoPage.PRIVACY -> SubPage(Copy.string.ui_privacy, { page = InfoPage.GRID }) { Privacy() }
            InfoPage.CREDITS -> SubPage(Copy.string.ui_credits, { page = InfoPage.GRID }) { Credits() }
        }
    }
}

/** 2x3 bento grid matching prototype: How-to, Privacy, Rate, GitHub, Donate, Credits; version at bottom. */
@Composable
private fun InfoGrid(onPage: (InfoPage) -> Unit) {
    val context = LocalContext.current
    val version = remember { context.packageManager.getPackageInfo(context.packageName, 0).versionName.orEmpty() }
    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.SpaceBetween,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .widthIn(max = 236.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                InfoBentoTile(Glyph.LIST, Copy.string.ui_how_to, Modifier.weight(1f)) { onPage(InfoPage.HOW_TO) }
                InfoBentoTile(Glyph.SHIELD, Copy.string.ui_privacy, Modifier.weight(1f)) { onPage(InfoPage.PRIVACY) }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                InfoBentoTile(Glyph.STAR, Copy.string.ui_rate, Modifier.weight(1f)) { Links.rate(context) }
                InfoBentoTile(Glyph.GITHUB, Copy.string.ui_github, Modifier.weight(1f)) { Links.open(context, Links.REPO) }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                InfoBentoTile(Glyph.COFFEE, Copy.string.ui_donate, Modifier.weight(1f)) { Links.open(context, Links.DONATE) }
                InfoBentoTile(Glyph.USERS, Copy.string.ui_credits, Modifier.weight(1f)) { onPage(InfoPage.CREDITS) }
            }
        }
        BasicText(
            stringResource(Copy.string.ui_version_tag, version),
            Modifier.padding(top = 16.dp, bottom = 4.dp),
            style = Theme.pageDetail.copy(color = Color(0x73FFFFFF), fontFamily = Theme.mono, fontSize = 12.sp, textAlign = TextAlign.Center),
        )
    }
}

@Composable
private fun InfoBentoTile(glyph: Glyph, label: Int, modifier: Modifier, onClick: () -> Unit) {
    val shape = RoundedCornerShape(12.dp)
    Box(
        modifier
            .aspectRatio(1f)
            .clip(shape)
            .background(Color.Black)
            .border(1.dp, Color(0x26FFFFFF), shape)
            .clickable(role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(glyph, Color.White, null, size = 26.dp, stroke = 2f)
            BasicText(
                stringResource(label),
                style = Theme.pageDetail.copy(color = Color(0xB3FFFFFF), fontSize = 13.5.sp, fontWeight = FontWeight.Medium),
            )
        }
    }
}

@Composable
private fun SubPage(titleRes: Int, onBack: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxSize()) {
        val back = stringResource(Copy.string.ui_back)
        Row(
            Modifier
                .fillMaxWidth()
                .height(48.dp)
                .drawBehind { drawLine(Color(0x26FFFFFF), Offset(0f, size.height - 1f), Offset(size.width, size.height - 1f), 1f) }
                .padding(horizontal = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Box(
                Modifier
                    .size(32.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .clickable(role = Role.Button, onClickLabel = back, onClick = onBack),
                contentAlignment = Alignment.Center,
            ) {
                Icon(Glyph.ARROW, Color.White, back, Modifier.graphicsLayer { rotationZ = 180f }, size = 18.dp, stroke = 2.4f)
            }
            BasicText(
                stringResource(titleRes),
                style = Theme.pageName.copy(fontSize = 16.sp, fontWeight = FontWeight.SemiBold),
            )
        }
        Column(
            Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 24.dp),
            content = content,
        )
    }
}

@Composable
private fun HowTo() {
    val context = LocalContext.current
    Step(1, Copy.string.phone_step_download, Copy.string.phone_step_download_detail, last = false) { Links.open(context, Links.RELEASES) }
    Step(2, Copy.string.phone_step_connect, Copy.string.phone_step_connect_detail, last = false)
    Step(3, Copy.string.phone_step_stream, Copy.string.phone_step_stream_detail, last = true)
}

@Composable
private fun Privacy() {
    BasicText(
        stringResource(Copy.string.help_privacy_statement),
        Modifier.padding(vertical = 4.dp),
        style = Theme.pageDetail.copy(fontSize = 14.sp, lineHeight = 21.sp, color = Color(0xB3FFFFFF)),
    )
}

@Composable
private fun Credits() {
    val context = LocalContext.current
    Links.CONTRIBUTORS_LIST.forEach { (name, login) ->
        Row(
            Modifier
                .fillMaxWidth()
                .drawBehind { drawLine(Color(0x1AFFFFFF), Offset(0f, size.height - 1f), Offset(size.width, size.height - 1f), 1f) }
                .clickable(role = Role.Button) { Links.open(context, Links.profile(login)) }
                .padding(vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            BasicText(name, Modifier.weight(1f), style = Theme.pageText.copy(fontSize = 15.sp, fontWeight = FontWeight.Medium))
            BasicText("@$login", style = Theme.pageDetail.copy(fontFamily = Theme.mono, fontSize = 13.5.sp, color = Color(0x73FFFFFF)))
        }
    }
    Row(
        Modifier
            .fillMaxWidth()
            .clickable(role = Role.Button) { Links.open(context, Links.CONTRIBUTORS) }
            .padding(vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        BasicText(stringResource(Copy.string.ui_github), Modifier.weight(1f), style = Theme.pageDetail.copy(fontSize = 14.sp))
        Icon(Glyph.EXTERNAL, Color(0xB3FFFFFF), null, size = 15.dp, stroke = 2f)
    }
}

@Composable
private fun Step(number: Int, name: Int, detail: Int, last: Boolean, onClick: (() -> Unit)? = null) {
    val tap = if (onClick != null) Modifier.clickable(role = Role.Button, onClick = onClick) else Modifier
    Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min).then(tap), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Column(Modifier.width(36.dp).fillMaxHeight(), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(Modifier.size(36.dp).clip(CircleShape).border(1.5.dp, Theme.ink, CircleShape), contentAlignment = Alignment.Center) {
                BasicText(number.toString(), style = Theme.pageName.copy(fontFamily = Theme.mono, fontSize = 16.sp, color = Theme.ink))
            }
            if (!last) Box(Modifier.width(1.5.dp).weight(1f).background(Theme.chipOnEdge))
        }
        Column(Modifier.weight(1f).padding(top = 5.dp, bottom = if (last) 0.dp else 28.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                BasicText(stringResource(name), style = Theme.pageName.copy(fontSize = 16.sp))
                if (onClick != null) Icon(Glyph.EXTERNAL, Theme.text2, null, Modifier.padding(start = 8.dp), size = 18.dp, stroke = Tokens.Phone.GLYPH_STROKE)
            }
            BasicText(stringResource(detail), style = Theme.pageDetail.copy(fontSize = 13.5.sp, lineHeight = 19.sp))
        }
    }
}
