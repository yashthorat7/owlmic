//! libopus, as built by build.rs: the decoder for the phone's mic and the encoder for the speaker.

use std::ffi::c_int;

#[repr(C)]
struct RawDecoder {
    _private: [u8; 0],
}
#[repr(C)]
struct RawEncoder {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn opus_decoder_create(fs: i32, channels: c_int, error: *mut c_int) -> *mut RawDecoder;
    fn opus_decode(
        st: *mut RawDecoder,
        data: *const u8,
        len: i32,
        pcm: *mut i16,
        frame_size: c_int,
        decode_fec: c_int,
    ) -> c_int;
    fn opus_decoder_destroy(st: *mut RawDecoder);
    fn opus_encoder_create(
        fs: i32,
        channels: c_int,
        application: c_int,
        error: *mut c_int,
    ) -> *mut RawEncoder;
    fn opus_encode_float(
        st: *mut RawEncoder,
        pcm: *const f32,
        frame_size: c_int,
        data: *mut u8,
        max: i32,
    ) -> i32;
    fn opus_encoder_ctl(st: *mut RawEncoder, request: c_int, ...) -> c_int;
    fn opus_encoder_destroy(st: *mut RawEncoder);
}

const OPUS_APPLICATION_AUDIO: c_int = 2049;
const OPUS_SET_BITRATE_REQUEST: c_int = 4002;
const OPUS_SET_COMPLEXITY_REQUEST: c_int = 4010;
const OPUS_SET_INBAND_FEC_REQUEST: c_int = 4012;
const OPUS_SET_PACKET_LOSS_PERC_REQUEST: c_int = 4014;

pub const RATE: i32 = 48_000;

pub struct Decoder {
    raw: *mut RawDecoder,
    channels: usize,
}

// The decoder is only ever used by one thread at a time, behind its pipeline's lock.
unsafe impl Send for Decoder {}

impl Decoder {
    pub fn new(channels: usize) -> Option<Self> {
        let mut err = 0;
        let raw = unsafe { opus_decoder_create(RATE, channels as c_int, &mut err) };
        (!raw.is_null() && err == 0).then_some(Self { raw, channels })
    }

    /// Decodes `packet` into `out` (interleaved), returning samples per channel. With `fec`, it
    /// rebuilds the packet before this one from this one's redundancy instead. With no packet,
    /// it conceals one lost frame of `out`'s length.
    pub fn decode(&mut self, packet: Option<&[u8]>, out: &mut [i16], fec: bool) -> usize {
        let frame = (out.len() / self.channels) as c_int;
        let (ptr, len) = packet.map_or((std::ptr::null(), 0), |p| (p.as_ptr(), p.len() as i32));
        let n = unsafe { opus_decode(self.raw, ptr, len, out.as_mut_ptr(), frame, fec as c_int) };
        n.max(0) as usize
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe { opus_decoder_destroy(self.raw) }
    }
}

pub struct Encoder {
    raw: *mut RawEncoder,
    channels: usize,
}

unsafe impl Send for Encoder {}

impl Encoder {
    /// A general audio encoder for the speaker: `bitrate` bits a second, in-band FEC on.
    pub fn new(channels: usize, bitrate: i32) -> Option<Self> {
        let mut err = 0;
        let raw = unsafe {
            opus_encoder_create(RATE, channels as c_int, OPUS_APPLICATION_AUDIO, &mut err)
        };
        if raw.is_null() || err != 0 {
            return None;
        }
        unsafe {
            opus_encoder_ctl(raw, OPUS_SET_BITRATE_REQUEST, bitrate as c_int);
            opus_encoder_ctl(raw, OPUS_SET_COMPLEXITY_REQUEST, 8 as c_int);
            opus_encoder_ctl(raw, OPUS_SET_INBAND_FEC_REQUEST, 1 as c_int);
            opus_encoder_ctl(raw, OPUS_SET_PACKET_LOSS_PERC_REQUEST, 5 as c_int);
        }
        Some(Self { raw, channels })
    }

    /// Encodes one frame of interleaved samples into `out`, returning the packet length.
    pub fn encode(&mut self, pcm: &[f32], out: &mut [u8]) -> usize {
        let frame = (pcm.len() / self.channels) as c_int;
        let n = unsafe {
            opus_encode_float(
                self.raw,
                pcm.as_ptr(),
                frame,
                out.as_mut_ptr(),
                out.len() as i32,
            )
        };
        n.max(0) as usize
    }

    /// Expected loss, from the phone's reports, so FEC grows when Wi-Fi gets worse.
    pub fn set_loss(&mut self, pct: u8) {
        unsafe {
            opus_encoder_ctl(
                self.raw,
                OPUS_SET_PACKET_LOSS_PERC_REQUEST,
                pct.min(30) as c_int,
            );
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { opus_encoder_destroy(self.raw) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_survives_an_encode_and_decode_and_losses_are_concealed() {
        let mut enc = Encoder::new(1, 48_000).unwrap();
        let mut dec = Decoder::new(1).unwrap();
        let mut packet = [0u8; 1500];
        let mut out = [0i16; 480];
        let mut energy = 0.0;
        for f in 0..20 {
            let tone: Vec<f32> = (0..480)
                .map(|i| {
                    (2.0 * std::f32::consts::PI * 440.0 * (f * 480 + i) as f32 / 48_000.0).sin()
                        * 0.5
                })
                .collect();
            let n = enc.encode(&tone, &mut packet);
            assert!(n > 0);
            assert_eq!(dec.decode(Some(&packet[..n]), &mut out, false), 480);
            if f > 5 {
                energy += out.iter().map(|s| (*s as f64).powi(2)).sum::<f64>();
            }
        }
        assert!(energy > 1e9, "the tone came through");
        assert_eq!(
            dec.decode(None, &mut out, false),
            480,
            "a lost frame is concealed"
        );
    }
}
