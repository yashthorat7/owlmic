//! The Device Hub: starts and stops the device side of each feature as the Media Hub asks,
//! keeps the PC speakers quiet while Speaker runs, and reports device health.

use crate::DeviceHealth;
use crate::audio::Com;
use crate::camera::{CameraOutput, Feed, Shape};
use crate::loopback::SpeakerCapture;
use crate::quiet::Quiet;
use crate::render::MicOutput;
use crate::softcam::Softcam;
use owlmic_hub::{Hub, Outbox};
use owlmic_media::audio::pipeline::JitterBuffer;
use owlmic_media::audio::speaker::SpeakerSender;
use owlmic_media::video::Framing;
use owlmic_media::video::receiver::VideoReceiver;
use owlmic_settings::store::Store;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// Devices come and go in bursts; check once they settle.
const SETTLE: Duration = Duration::from_secs(1);
const QUIET_CHECK_EVERY: Duration = Duration::from_millis(250);

pub enum DeviceMsg {
    Mic(bool),
    Speaker(bool),
    Camera(bool),
    QuietPc(bool),
    Shape {
        framing: Framing,
        mirror: bool,
    },
    /// Check now (at startup, and after a repair).
    Check,
    DevicesChanged,
    /// The default output moved: Quiet PC speakers follows it.
    DefaultOutputChanged,
    Repair,
    RepairDone,
    /// Owlmic is quitting: stop everything and unmute, then answer.
    Shutdown(std::sync::mpsc::SyncSender<()>),
}

pub enum DeviceEvent {
    Health(DeviceHealth),
    Repairing(bool),
}

pub struct Shared {
    pub store: Arc<Store>,
    pub jitter: Arc<JitterBuffer>,
    pub speaker: Arc<SpeakerSender>,
    pub video: Arc<VideoReceiver>,
    pub preview: Arc<owlmic_ui::preview::Preview>,
    pub win11: bool,
}

pub struct DeviceHub {
    me: Outbox<DeviceMsg>,
    emit: Box<dyn Fn(DeviceEvent) + Send>,
    shared: Shared,
    shape: Arc<Shape>,
    level: Arc<AtomicU32>,
    com: Option<Com>,
    quiet: Option<Quiet>,
    quiet_on: bool,
    softcam: Option<Arc<Softcam>>,
    placeholder: Option<Arc<Vec<u8>>>,
    mic: Option<MicOutput>,
    capture: Option<SpeakerCapture>,
    camera: Option<CameraOutput>,
    watch: Option<crate::health::Watch>,
    health: DeviceHealth,
    check_at: Option<Instant>,
    quiet_check_at: Option<Instant>,
    repairing: bool,
}

// Every COM object here is created on the hub's own thread (in the multithreaded apartment) and
// stays there; the hub is moved to that thread before any exists.
unsafe impl Send for DeviceHub {}

impl DeviceHub {
    pub fn new(
        me: Outbox<DeviceMsg>,
        shared: Shared,
        emit: impl Fn(DeviceEvent) + Send + 'static,
    ) -> Self {
        Self {
            me,
            emit: Box::new(emit),
            shared,
            shape: Arc::default(),
            level: Arc::default(),
            com: None,
            quiet: None,
            quiet_on: true,
            softcam: None,
            placeholder: None,
            mic: None,
            capture: None,
            camera: None,
            watch: None,
            health: DeviceHealth::default(),
            check_at: None,
            quiet_check_at: None,
            repairing: false,
        }
    }

    /// First message on the hub thread: COM, the crash-mute restore, the device watch, and on
    /// Windows 10 softcam with its placeholder, so apps find a working camera.
    fn ensure_started(&mut self) {
        if self.com.is_some() {
            return;
        }
        self.com = Some(Com::init());
        self.quiet = Some(Quiet::new(self.shared.store.clone()));
        let (me, me2) = (self.me.clone(), self.me.clone());
        self.watch = crate::health::watch(
            move || me.send(DeviceMsg::DevicesChanged),
            move || me2.send(DeviceMsg::DefaultOutputChanged),
        );
        if !self.shared.win11 {
            self.softcam = Softcam::load().map(Arc::new);
            if let Some(cam) = &self.softcam {
                let placeholder = Arc::new(crate::camera::placeholder_bgr());
                cam.send(&placeholder);
                self.placeholder = Some(placeholder);
            }
        }
    }

    fn check(&mut self) {
        self.check_at = None;
        self.health = crate::health::check(self.shared.win11);
        (self.emit)(DeviceEvent::Health(self.health));
    }

    fn feed(&self) -> Option<Feed> {
        if self.shared.win11 {
            return Some(Feed::Ring {
                mapping: None,
                tried: None,
            });
        }
        Some(Feed::Softcam {
            cam: self.softcam.clone()?,
            bgr: Vec::new(),
            placeholder: self.placeholder.clone()?,
        })
    }

    fn set_quiet(&mut self) {
        let Some(quiet) = self.quiet.as_mut() else {
            return;
        };
        if self.capture.is_some() && self.quiet_on {
            quiet.engage();
            self.quiet_check_at = Some(Instant::now() + QUIET_CHECK_EVERY);
        } else {
            quiet.release();
            self.quiet_check_at = None;
        }
    }

    /// Repairs what the check found broken; with nothing found (the settings' "Repair Owlmic Mic
    /// and Cam"), it reinstalls both anyway.
    fn repair(&mut self) {
        if self.repairing {
            return;
        }
        let mut broken = self.health;
        if broken.all_ok() {
            broken.mic = false;
            broken.cam = false;
        }
        let Some((dir, exe)) = std::env::current_exe().ok().and_then(|e| {
            Some((
                e.parent()?.to_string_lossy().into_owned(),
                e.to_string_lossy().into_owned(),
            ))
        }) else {
            return;
        };
        self.repairing = true;
        (self.emit)(DeviceEvent::Repairing(true));
        let script = crate::repair::script(&dir, &exe, broken);
        let me = self.me.clone();
        let _ = std::thread::Builder::new()
            .name("repair".into())
            .spawn(move || {
                crate::repair::run(&script);
                me.send(DeviceMsg::RepairDone);
            });
    }
}

impl Hub for DeviceHub {
    type Msg = DeviceMsg;

    fn handle(&mut self, msg: DeviceMsg) {
        self.ensure_started();
        match msg {
            DeviceMsg::Mic(on) => {
                self.mic = on.then(|| MicOutput::start(self.shared.jitter.clone()));
            }
            DeviceMsg::Speaker(on) => {
                // Unmute before the capture stops, so no sound is lost in between.
                if !on {
                    self.capture = None;
                    self.set_quiet();
                } else if self.capture.is_none() {
                    self.capture =
                        SpeakerCapture::start(self.shared.speaker.clone(), self.level.clone());
                    self.set_quiet();
                }
            }
            DeviceMsg::Camera(on) => {
                self.camera = None;
                if on && let Some(feed) = self.feed() {
                    let (video, preview) = (self.shared.video.clone(), self.shared.preview.clone());
                    self.camera = Some(CameraOutput::start(
                        video,
                        self.shape.clone(),
                        feed,
                        preview,
                    ));
                }
            }
            DeviceMsg::QuietPc(on) => {
                self.quiet_on = on;
                self.set_quiet();
            }
            DeviceMsg::Shape { framing, mirror } => self.shape.set(framing, mirror),
            DeviceMsg::Check => self.check(),
            DeviceMsg::DevicesChanged => self.check_at = Some(Instant::now() + SETTLE),
            DeviceMsg::DefaultOutputChanged => {
                if let Some(q) = self.quiet.as_mut()
                    && self.capture.is_some()
                    && self.quiet_on
                {
                    q.follow_default();
                    self.quiet_check_at = Some(Instant::now() + QUIET_CHECK_EVERY);
                }
            }
            DeviceMsg::Repair => self.repair(),
            DeviceMsg::Shutdown(done) => {
                self.mic = None;
                self.camera = None;
                self.capture = None;
                if let Some(q) = self.quiet.as_mut() {
                    q.release();
                }
                let _ = done.send(());
            }
            DeviceMsg::RepairDone => {
                self.repairing = false;
                (self.emit)(DeviceEvent::Repairing(false));
                self.check();
            }
        }
    }

    fn tick(&mut self, now: Instant) {
        if self.check_at.is_some_and(|t| now >= t) {
            self.check();
        }
        if self.quiet_check_at.is_some_and(|t| now >= t) {
            let peak = f32::from_bits(self.level.load(Ordering::Relaxed));
            match self.quiet.as_mut() {
                Some(q) if q.is_checking(now) => {
                    q.check(peak, now);
                    self.quiet_check_at = Some(now + QUIET_CHECK_EVERY);
                }
                _ => self.quiet_check_at = None,
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        [self.check_at, self.quiet_check_at]
            .into_iter()
            .flatten()
            .min()
    }
}

impl Drop for DeviceHub {
    fn drop(&mut self) {
        self.capture = None;
        if let Some(q) = self.quiet.as_mut() {
            q.release();
        }
    }
}
