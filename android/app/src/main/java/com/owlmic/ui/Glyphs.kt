package com.owlmic.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.owlmic.core.design.Tokens

/** The Lucide icons Owlmic uses (section 29.4); the main screen's come from the prototype in website/proto-templates. */
enum class Glyph(vararg val paths: String) {
    MIC("M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3z", "M19 10v2a7 7 0 0 1-14 0v-2", "M12 19v3"),
    MIC_OFF(
        "M1 1l22 22", "M9 9v3a3 3 0 0 0 5.12 2.12M15 9.34V4a3 3 0 0 0-5.94 -.6",
        "M17 16.95A7 7 0 0 1 5 12v-2m14 0v2a7 7 0 0 1 -.11 1.23", "M12 19v3",
    ),
    CAMERA(
        "M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z",
        "M9 13a3 3 0 1 0 6 0a3 3 0 1 0-6 0",
    ),
    CAMERA_OFF(
        "M1 1l22 22", "M21 21H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h3m3-3h6l2.5 3H21a2 2 0 0 1 2 2v7.5",
        "M9 13a3 3 0 1 0 6 0a3 3 0 1 0-6 0",
    ),
    SPEAKER("M11 5L6 9H2v6h4l5 4V5z", "M15.54 8.46a5 5 0 0 1 0 7.07", "M19.07 4.93a10 10 0 0 1 0 14.14"),
    SPEAKER_OFF("M11 5L6 9H2v6h4l5 4V5z", "M23 9l-6 6", "M17 9l6 6"),
    FLIP(
        "M11 19H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h5", "M13 5h7a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2h-5",
        "M9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0", "M18 22l-3-3 3-3", "M6 2l3 3-3 3",
    ),
    CLOSE("M18 6L6 18", "M6 6l12 12"),
    HELP("M9.09 9.5a3 3 0 0 1 5.82 1c0 2-2.91 3-2.91 3.5", "M12 17.5h.01"),
    OPEN("M7 17L17 7", "M7 7h10v10"),

    /** The links, on the link tile and the connection list: plug (USB debugging), chain (USB tethering), Wi-Fi, Bluetooth. */
    WIFI("M5 10.36a10 10 0 0 1 14 0", "M8.5 13.93a5 5 0 0 1 7 0", "M12 17.5h.01"),
    PLUG("M10 13v5a2 2 0 0 0 4 0v-5", "M9 7h6a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1z", "M9 7V4", "M15 7V4"),
    CHAIN(
        "M10 13a5 5 0 0 0 7.54 .54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71",
        "M14 11a5 5 0 0 0-7.54 -.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71",
    ),
    BLUETOOTH("M7 7l10 10-5 5V2l5 5L7 17"),

    /** The link tile with no link yet: looking (the globe), or something needs the user (the alert). */
    GLOBE("M22 12A10 10 0 1 1 2 12a10 10 0 0 1 20 0z", "M12 2a14.5 14.5 0 0 0 0 20a14.5 14.5 0 0 0 0-20", "M2 12h20"),
    ALERT("M22 12A10 10 0 1 1 2 12a10 10 0 0 1 20 0z", "M12 8v4", "M12 16h.01"),

    BACK("M19 12H5", "M12 19l-7-7 7-7"),
    CHECK("M20 6L9 17l-5-5"),
    RECONNECT("M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8", "M21 3v5h-5"),
    EYE(
        "M2.06 12.35a1 1 0 0 1 0-.7 10.75 10.75 0 0 1 19.88 0 1 1 0 0 1 0 .7 10.75 10.75 0 0 1-19.88 0",
        "M9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0",
    ),
    EYE_OFF(
        "M10.73 5.08a10.74 10.74 0 0 1 11.2 6.57 1 1 0 0 1 0 .7 10.75 10.75 0 0 1-1.44 2.49", "M14.08 14.16a3 3 0 0 1-4.24-4.24",
        "M17.48 17.5a10.75 10.75 0 0 1-15.42-5.15 1 1 0 0 1 0-.7 10.75 10.75 0 0 1 4.45-5.14", "M2 2l20 20",
    ),
    SLIDERS("M21 4h-7", "M10 4H3", "M21 12h-9", "M8 12H3", "M21 20h-5", "M12 20H3", "M14 2v4", "M8 10v4", "M16 18v4"),
    MIRROR("M3 7l5 5-5 5V7", "M21 7l-5 5 5 5V7", "M12 20v2", "M12 14v2", "M12 8v2", "M12 2v2"),

    /** The info page's buttons. */
    HOW_TO("M10 12h11", "M10 18h11", "M10 6h11", "M4 10h2", "M4 6h1v4", "M6 18H4c0-1 2-2 2-3s-1-1.5-2-1"),
    SHIELD("M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"),
    STAR("M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"),
    GITHUB(
        "M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.4 5.4 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4",
        "M9 18c-4.51 2-5-2-7-2",
    ),
    COFFEE("M10 2v2", "M14 2v2", "M6 2v2", "M16 8a1 1 0 0 1 1 1v8a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4V9a1 1 0 0 1 1-1h14a4 4 0 1 1 0 8h-1"),
    PEOPLE(
        "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2", "M5 7a4 4 0 1 0 8 0a4 4 0 1 0-8 0",
        "M22 21v-2a4 4 0 0 0-3-3.87", "M16 3.13a4 4 0 0 1 0 7.75",
    ),
    ;

    private val vectors = HashMap<Float, ImageVector>()

    fun vector(stroke: Float): ImageVector = vectors.getOrPut(stroke) {
        ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f).apply {
            paths.forEach {
                addPath(
                    PathParser().parsePathString(it).toNodes(),
                    stroke = SolidColor(Color.White),
                    strokeLineWidth = stroke,
                    strokeLineCap = StrokeCap.Round,
                    strokeLineJoin = StrokeJoin.Round,
                )
            }
        }.build()
    }
}

@Composable
fun Icon(
    glyph: Glyph,
    color: Color,
    label: String?,
    modifier: Modifier = Modifier,
    size: Dp = Theme.icon,
    stroke: Float = Tokens.Icon.STROKE,
) {
    Image(rememberVectorPainter(glyph.vector(stroke)), label, modifier.size(size), colorFilter = ColorFilter.tint(color))
}
