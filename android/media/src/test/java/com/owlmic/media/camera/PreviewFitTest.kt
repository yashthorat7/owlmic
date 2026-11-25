package com.owlmic.media.camera

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PreviewFitTest {
    @Test
    fun anUprightPictureZoomsToFitPreview() {
        // 1280×720 in a 1080×1400 preview: scaled to cover height, centred horizontally.
        assertArrayEquals(intArrayOf(-704, 0, 2488, 1400), fitPreview(1280, 720, 1080, 1400, 0))
    }

    @Test
    fun aPictureMadeSidewaysLiesOnItsSideAndFillsMore() {
        // The same picture turned a quarter is 720 wide and 1280 tall on the upright screen.
        assertArrayEquals(intArrayOf(0, -260, 1080, 1920), fitPreview(1280, 720, 1080, 1400, 90))
        assertArrayEquals(intArrayOf(0, -260, 1080, 1920), fitPreview(1280, 720, 1080, 1400, 270))
        assertArrayEquals(intArrayOf(-704, 0, 2488, 1400), fitPreview(1280, 720, 1080, 1400, 180))
    }

    @Test
    fun theTurnAFrameWasMadeForComesFromItsBufferRotation() {
        // A back camera with its sensor at 90°, as on most phones: CameraX's buffer rotation for each Surface rotation.
        val back = { r: Int -> (90 - r * 90 + 360) % 360 }
        assertEquals(0, pictureTurn(90, back))
        assertEquals(90, pictureTurn(0, back))
        assertEquals(270, pictureTurn(180, back))
        // A front camera at 270° turns the other way.
        val front = { r: Int -> (270 + r * 90) % 360 }
        assertEquals(90, pictureTurn(0, front))
        assertEquals(270, pictureTurn(180, front))
        assertNull(pictureTurn(45, back))
    }

    @Test
    fun opticalCropRectFitsAndInterpolatesSymmetricallyWithoutDistortion() {
        val full = cropRectFor(null, 1080, 1400)
        assertArrayEquals(floatArrayOf(0f, 0f, 1080f, 1400f), full, 0.01f)

        val wide = cropRectFor(16f / 9f, 1080, 1400)
        assertArrayEquals(floatArrayOf(0f, 396.25f, 1080f, 607.5f), wide, 0.01f)

        val tall = cropRectFor(9f / 16f, 1080, 1400)
        assertArrayEquals(floatArrayOf(146.25f, 0f, 787.5f, 1400f), tall, 0.01f)

        val mid = interpolateCropRect(full, wide, 0.5f, 1080, 1400)
        assertArrayEquals(floatArrayOf(0f, 198.125f, 1080f, 1003.75f), mid, 0.01f)

        assertEquals(0f, easeOutExpo(0f), 0.0001f)
        assertEquals(1f, easeOutExpo(1f), 0.0001f)
        assertEquals(true, easeOutExpo(0.3f) > 0.65f)
    }
}
