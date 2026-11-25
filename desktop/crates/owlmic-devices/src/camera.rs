//! The camera's way out (SYSTEM_DESIGN section 17.2): whole H.264 frames are decoded, shaped
//! (Fill or Fit, Mirror) to 1920 x 1080 and delivered to Owlmic Cam: the shared frame ring on
//! Windows 11, softcam on Windows 10. Runs only while the phone's camera streams.

use crate::audio::Com;
use crate::decoder::Decoder;
use crate::softcam::Softcam;
use owlmic_media::video::receiver::VideoReceiver;
use owlmic_media::video::{Framing, Nv12, Nv12Shaper};
use owlmic_ui::preview::{self, Preview};
use owlmic_vcam::shared::{HEIGHT, Mapping, STALE_MS, WIDTH};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::System::SystemInformation::GetTickCount64;

/// The ring exists only while an app has Owlmic Cam open; look for it this often.
const RETRY_OPEN: Duration = Duration::from_secs(1);
/// Decoder errors in a row before the stream waits for a keyframe (SYSTEM_DESIGN section 14.8).
const ERRORS_BEFORE_RESYNC: u32 = 3;

/// What the PC shows, changed live from the panel (SYSTEM_DESIGN section 17.2). Fit until told
/// otherwise: the whole picture, never cropped by default.
#[derive(Default)]
pub struct Shape {
    fill: AtomicBool,
    mirror: AtomicBool,
}

impl Shape {
    pub fn set(&self, framing: Framing, mirror: bool) {
        self.fill.store(framing == Framing::Fill, Ordering::Relaxed);
        self.mirror.store(mirror, Ordering::Relaxed);
    }

    fn get(&self) -> (Framing, bool) {
        let framing = if self.fill.load(Ordering::Relaxed) {
            Framing::Fill
        } else {
            Framing::Fit
        };
        (framing, self.mirror.load(Ordering::Relaxed))
    }
}

/// Where pictures go.
pub enum Feed {
    Ring {
        mapping: Option<Mapping>,
        tried: Option<Instant>,
    },
    Softcam {
        cam: Arc<Softcam>,
        bgr: Vec<u8>,
        placeholder: Arc<Vec<u8>>,
    },
}

impl Feed {
    fn write(&mut self, picture: &Nv12) {
        match self {
            Feed::Ring { mapping, tried } => {
                if mapping.is_none() && tried.is_none_or(|t| t.elapsed() >= RETRY_OPEN) {
                    *mapping = Mapping::open();
                    *tried = Some(Instant::now());
                }
                if let Some(m) = mapping {
                    m.ring.write(&picture.data, unsafe { GetTickCount64() });
                }
            }
            Feed::Softcam { cam, bgr, .. } => {
                picture.to_bgr(bgr);
                cam.send(bgr);
            }
        }
    }

    /// Clears any retained frame so both Windows 11 Ring and Windows 10 Softcam show the placeholder.
    fn placeholder(&mut self) {
        match self {
            Feed::Ring { mapping, tried } => {
                if mapping.is_none() {
                    *mapping = Mapping::open();
                }
                if let Some(m) = mapping.take() {
                    m.ring.clear();
                }
                *tried = None;
            }
            Feed::Softcam {
                cam,
                bgr,
                placeholder,
            } => {
                bgr.fill(0);
                cam.send(placeholder);
            }
        }
    }
}

/// The placeholder picture in softcam's BGR, drawn once.
pub fn placeholder_bgr() -> Vec<u8> {
    let mut bgr = Vec::new();
    owlmic_vcam::placeholder::render(WIDTH, HEIGHT).to_bgr(&mut bgr);
    bgr
}

pub struct CameraOutput {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl CameraOutput {
    pub fn start(
        receiver: Arc<VideoReceiver>,
        shape: Arc<Shape>,
        feed: Feed,
        preview: Arc<Preview>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let thread = std::thread::Builder::new()
            .name("video".into())
            .spawn(move || run(&receiver, &shape, feed, &preview, &s))
            .ok();
        Self { stop, thread }
    }
}

impl Drop for CameraOutput {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    receiver: &VideoReceiver,
    shape: &Shape,
    mut feed: Feed,
    preview: &Preview,
    stop: &AtomicBool,
) {
    let _com = Com::init();
    preview.clear();
    feed.placeholder();
    // Windows N editions ship without Media Foundation: the camera keeps its placeholder.
    let Ok(mut decoder) = Decoder::new() else {
        return;
    };
    let mut out = Nv12::black(WIDTH, HEIGHT);
    let mut shaper = Nv12Shaper::new();
    let mut has_picture = false;
    let mut last = Instant::now();
    let mut previewed = Instant::now();
    let mut last_pic_size = (0usize, 0usize);
    let mut errors = 0;
    while !stop.load(Ordering::Acquire) {
        let Some(next) = receiver.next_frame(Duration::from_millis(120)) else {
            if has_picture && last.elapsed() >= Duration::from_millis(STALE_MS) {
                has_picture = false;
                out = Nv12::black(WIDTH, HEIGHT);
                last_pic_size = (0, 0);
                preview.clear();
                feed.placeholder();
            }
            continue;
        };
        match decoder.decode_frame(&next.frame.data, next.frame.timestamp_us, !next.more) {
            // A newer frame waits behind this one: decode it, but only the newest is shown.
            Ok(Some(_)) if next.more => {
                errors = 0;
                receiver.record_behind();
            }
            Ok(Some(picture)) => {
                errors = 0;
                receiver.record_delivered();
                preview.set_stream_size(picture.width, picture.height);
                let (framing, mirror) = shape.get();
                shaper.shape_into(picture, &mut out, framing, !mirror);
                feed.write(&out);
                has_picture = true;
                last = Instant::now();
                if preview.wanted() {
                    if (picture.width, picture.height) != last_pic_size
                        || previewed.elapsed() >= Duration::from_millis(preview::EVERY_MS)
                    {
                        previewed = last;
                        last_pic_size = (picture.width, picture.height);
                        let (pw, ph) = if picture.width >= picture.height {
                            let w = preview::WIDTH;
                            let h = ((w * picture.height) / picture.width).max(1);
                            (w, h)
                        } else {
                            let h = preview::HEIGHT;
                            let w = ((h * picture.width) / picture.height).max(1);
                            (w, h)
                        };
                        preview.put(pw, ph, |px| picture.to_bgra_scaled(pw, ph, mirror, px));
                    }
                } else {
                    last_pic_size = (0, 0);
                }
            }
            Ok(None) => errors = 0,
            Err(_) => {
                errors += 1;
                if errors >= ERRORS_BEFORE_RESYNC {
                    errors = 0;
                    decoder.flush();
                    receiver.resync();
                }
            }
        }
    }
    receiver.clear();
    preview.clear();
    feed.placeholder();
}
