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
import com.owlmic.core.design.GeneratedGlyphs
import com.owlmic.core.design.GlyphPath
import com.owlmic.core.design.Tokens

/** The Lucide icons Owlmic uses, dynamically sourced from shared/assets/icons/ via GeneratedGlyphs. */
enum class Glyph(val elements: () -> List<GlyphPath>) {
    ARROW({ GeneratedGlyphs.ARROW }),
    BLUETOOTH({ GeneratedGlyphs.BLUETOOTH }),
    CABLE({ GeneratedGlyphs.CABLE }),
    CAMERA({ GeneratedGlyphs.CAMERA }),
    CAMERA_OFF({ GeneratedGlyphs.CAMERA_OFF }),
    CHECK({ GeneratedGlyphs.CHECK }),
    CLOSE({ GeneratedGlyphs.CLOSE }),
    COFFEE({ GeneratedGlyphs.COFFEE }),
    COPY({ GeneratedGlyphs.COPY }),
    EXTERNAL({ GeneratedGlyphs.EXTERNAL }),
    EYE({ GeneratedGlyphs.EYE }),
    EYE_OFF({ GeneratedGlyphs.EYE_OFF }),
    FLIP({ GeneratedGlyphs.FLIP }),
    GIT_BRANCH({ GeneratedGlyphs.GIT_BRANCH }),
    GITHUB({ GeneratedGlyphs.GITHUB }),
    GLOBE({ GeneratedGlyphs.GLOBE }),
    HELP({ GeneratedGlyphs.HELP }),
    LINK({ GeneratedGlyphs.LINK }),
    LINK_OFF({ GeneratedGlyphs.LINK_OFF }),
    LIST({ GeneratedGlyphs.LIST }),
    MIC({ GeneratedGlyphs.MIC }),
    MIC_OFF({ GeneratedGlyphs.MIC_OFF }),
    MIRROR({ GeneratedGlyphs.MIRROR }),
    PHONE({ GeneratedGlyphs.PHONE }),
    PLUS({ GeneratedGlyphs.PLUS }),
    REFRESH({ GeneratedGlyphs.REFRESH }),
    SHIELD({ GeneratedGlyphs.SHIELD }),
    SPEAKER({ GeneratedGlyphs.SPEAKER }),
    SPEAKER_OFF({ GeneratedGlyphs.SPEAKER_OFF }),
    STAR({ GeneratedGlyphs.STAR }),
    UNPLUG({ GeneratedGlyphs.UNPLUG }),
    USB({ GeneratedGlyphs.USB }),
    USERS({ GeneratedGlyphs.USERS }),
    VIDEO({ GeneratedGlyphs.VIDEO }),
    VIDEO_OFF({ GeneratedGlyphs.VIDEO_OFF }),
    WIFI({ GeneratedGlyphs.WIFI });

    private val vectors = HashMap<Float, ImageVector>()

    fun vector(stroke: Float): ImageVector = vectors.getOrPut(stroke) {
        ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f).apply {
            elements().forEach { elem ->
                val nodes = PathParser().parsePathString(elem.data).toNodes()
                if (elem.isFill) {
                    addPath(
                        nodes,
                        fill = SolidColor(Color.White),
                    )
                } else {
                    addPath(
                        nodes,
                        stroke = SolidColor(Color.White),
                        strokeLineWidth = stroke,
                        strokeLineCap = StrokeCap.Round,
                        strokeLineJoin = StrokeJoin.Round,
                    )
                }
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
