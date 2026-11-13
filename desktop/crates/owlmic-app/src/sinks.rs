//! The carriers hand media straight to the pipelines through these (SYSTEM_DESIGN section 10).

use owlmic_link::MediaSink;
use owlmic_media::audio::mic::MicReceiver;
use owlmic_media::video::receiver::VideoReceiver;
use owlmic_proto::frame::{FragmentHeader, MediaHeader};
use std::sync::Arc;

pub struct MicSink(pub Arc<MicReceiver>);

impl MediaSink for MicSink {
    fn deliver(&self, h: &MediaHeader, payload: &[u8]) {
        self.0.packet(h.seq, h.timestamp_us, payload);
    }
}

pub struct CameraSink(pub Arc<VideoReceiver>);

impl MediaSink for CameraSink {
    fn deliver(&self, h: &MediaHeader, payload: &[u8]) {
        if let Some(f) = FragmentHeader::decode(payload) {
            self.0.fragment(
                f.frame,
                f.index,
                f.count,
                h.keyframe,
                h.timestamp_us,
                &payload[FragmentHeader::LEN..],
            );
        }
    }
}
