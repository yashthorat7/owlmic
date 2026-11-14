package com.owlmic.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.owlmic.R
import com.owlmic.core.design.TextSpec
import com.owlmic.core.design.Tokens

/** Section 29 on the phone: the tokens as Compose values. */
object Theme {
    val bg = Color(Tokens.Color.BG)
    val tile = Color(Tokens.Color.TILE)
    val hairline = Color(Tokens.Color.HAIRLINE)
    val text = Color(Tokens.Color.TEXT)
    val text2 = Color(Tokens.Color.TEXT2)
    val disabled = Color(Tokens.Color.DISABLED)
    val onContent = Color(Tokens.Color.ON_CONTENT)

    // The main screen's bento grid (the prototype in website/proto-templates).
    val ink = Color(Tokens.Color.INK)
    val seam = Color(Tokens.Color.GAP)
    val pressed = Color(Tokens.Color.PRESSED)
    val rule = Color(Tokens.Color.RULE)
    val chip = Color(Tokens.Color.CHIP)
    val faint = Color(Tokens.Color.FAINT)
    val chipOn = Color(Tokens.Color.CHIP_ON)
    val chipOnEdge = Color(Tokens.Color.CHIP_ON_EDGE)
    val chipOffEdge = Color(Tokens.Color.CHIP_OFF_EDGE)
    val stripEdge = Color.White.copy(alpha = Tokens.Phone.STRIP_EDGE_ALPHA)
    val gap = Tokens.Phone.GAP_DP.dp
    val radius = Tokens.Phone.TILE_RADIUS_DP.dp
    val glyph = Tokens.Phone.GLYPH_DP.dp

    val padding = Tokens.Space.TILE_PADDING_PHONE_DP.dp
    val touch = Tokens.Space.TOUCH_TARGET_DP.dp
    val icon = Tokens.Icon.PHONE_DP.dp
    const val STATE_MS = Tokens.Motion.STATE_MS.toInt()

    private fun family(res: Int) = FontFamily(
        listOf(400, 600).map { Font(res, FontWeight(it), variationSettings = FontVariation.Settings(FontVariation.weight(it))) },
    )

    val sans = family(R.font.geist)
    val mono = family(R.font.geist_mono)

    private fun style(spec: TextSpec, family: FontFamily = sans) = TextStyle(
        fontFamily = family,
        fontWeight = FontWeight(spec.weight),
        fontSize = spec.sizeSp.sp,
        lineHeight = spec.lineSp.sp,
        color = text,
    )

    val display = style(Tokens.Text.DISPLAY, mono)
    val title = style(Tokens.Text.TITLE)
    val body = style(Tokens.Text.BODY)
    val caption = style(Tokens.Text.CAPTION)

    // The pages over the preview (connection, info, settings) and the controls: larger, read at arm's length.
    val pageName = TextStyle(fontFamily = sans, fontWeight = FontWeight.SemiBold, fontSize = 19.sp, lineHeight = 24.sp, color = text)
    val pageText = TextStyle(fontFamily = sans, fontWeight = FontWeight.Normal, fontSize = 17.sp, lineHeight = 22.sp, color = text)
    val pageDetail = TextStyle(fontFamily = sans, fontWeight = FontWeight.Normal, fontSize = 15.sp, lineHeight = 20.sp, color = text2)
    val pageLabel = TextStyle(fontFamily = sans, fontWeight = FontWeight.SemiBold, fontSize = 14.sp, lineHeight = 18.sp, color = text2, letterSpacing = 0.6.sp)
}

/** One physical pixel, so the grid's lines stay crisp at any density (section 29.1). */
@Composable
fun hairline(): Dp = with(LocalDensity.current) { Tokens.Space.HAIRLINE_PX.toDp() }
