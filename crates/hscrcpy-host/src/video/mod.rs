use crate::{official_scrcpy::OfficialScrcpyVideoMessage, HostError, HostResult};
use hscrcpy_contracts::VideoCodec;

pub mod capability;
pub mod h264;
pub mod jpeg;

pub use capability::{
    normalized_requested_codec_order, select_codec_for_startup, CodecSelectionDiagnostics,
    HostCodecCapability,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NegotiatedVideoPath {
    H264Mainline,
    JpegFallback,
    DeferredExperimental(VideoCodec),
}

pub fn classify_negotiated_video_path(selected_codec: &VideoCodec) -> NegotiatedVideoPath {
    match selected_codec {
        VideoCodec::H264 => NegotiatedVideoPath::H264Mainline,
        VideoCodec::Jpeg => NegotiatedVideoPath::JpegFallback,
        other => NegotiatedVideoPath::DeferredExperimental(other.clone()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoUnitMetadata {
    pub codec: VideoCodec,
    pub pts_us: u64,
    pub is_keyframe: bool,
    pub payload_length: usize,
}

impl VideoUnitMetadata {
    pub fn validate_payload_length(&self, payload: &[u8]) -> HostResult<()> {
        if payload.len() == self.payload_length {
            Ok(())
        } else {
            Err(HostError::ContractViolation(format!(
                "video payload length mismatch: expected {}, got {}",
                self.payload_length,
                payload.len()
            )))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTransportPacket {
    pub metadata: VideoUnitMetadata,
    pub payload: Vec<u8>,
}

impl VideoTransportPacket {
    pub fn new(codec: VideoCodec, pts_us: u64, is_keyframe: bool, payload: Vec<u8>) -> Self {
        let payload_length = payload.len();
        Self {
            metadata: VideoUnitMetadata {
                codec,
                pts_us,
                is_keyframe,
                payload_length,
            },
            payload,
        }
    }

    pub fn decode(packet: &[u8]) -> HostResult<Self> {
        const HEADER_LEN: usize = 14;

        if packet.len() < HEADER_LEN {
            return Err(HostError::ContractViolation(format!(
                "video packet is shorter than header: expected at least {HEADER_LEN} bytes, got {}",
                packet.len()
            )));
        }

        let codec = match packet[0] {
            1 => VideoCodec::H264,
            2 => VideoCodec::Jpeg,
            3 => VideoCodec::H265Experimental,
            tag => {
                return Err(HostError::ContractViolation(format!(
                    "unsupported video packet codec tag `{tag}`"
                )))
            }
        };
        let pts_us = u64::from_be_bytes(packet[1..9].try_into().expect("slice length"));
        let is_keyframe = match packet[9] {
            0 => false,
            1 => true,
            flag => {
                return Err(HostError::ContractViolation(format!(
                    "video packet keyframe flag must be 0 or 1, got {flag}"
                )))
            }
        };
        let payload_length = u32::from_be_bytes(packet[10..14].try_into().expect("slice length"));
        if payload_length == 0 {
            return Err(HostError::ContractViolation(
                "video packet payload_length must be positive".to_string(),
            ));
        }

        let payload = packet[HEADER_LEN..].to_vec();
        if payload.len() != payload_length as usize {
            return Err(HostError::ContractViolation(format!(
                "video payload length mismatch: expected {}, got {}",
                payload_length,
                payload.len()
            )));
        }

        Ok(Self::new(codec, pts_us, is_keyframe, payload))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedVideoIngress {
    H264(h264::H264AccessUnit),
    Jpeg(crate::render::JpegFrame),
    DeferredExperimental(VideoTransportPacket),
}

impl PreparedVideoIngress {
    pub fn from_packet(packet: VideoTransportPacket) -> HostResult<Self> {
        match packet.metadata.codec {
            VideoCodec::H264 => Ok(Self::H264(
                h264::H264VideoPath.ingest_unit(&packet.metadata, &packet.payload)?,
            )),
            VideoCodec::Jpeg => Ok(Self::Jpeg(
                jpeg::JpegVideoPath.ingest_unit(&packet.metadata, &packet.payload)?,
            )),
            VideoCodec::H265Experimental => Ok(Self::DeferredExperimental(packet)),
        }
    }

    /// Converts one official scrcpy stream message into renderer-facing ingress.
    ///
    /// The current official adapter facade exposes encoded bytes and codec only.
    /// It does not expose source timestamps or keyframe flags, so callers must
    /// provide a deterministic synthesized PTS and this conversion marks H.264
    /// units as non-keyframes.
    pub fn from_official_scrcpy_message(
        message: OfficialScrcpyVideoMessage,
        synthesized_pts_us: u64,
    ) -> HostResult<Self> {
        if message.codec != VideoCodec::H264 {
            return Err(HostError::ContractViolation(format!(
                "official scrcpy ingress only supports h264 payloads for renderer handoff, got {:?}: reply_type={} payload_key={}",
                message.codec, message.reply_type, message.payload_key
            )));
        }

        Self::from_packet(VideoTransportPacket::new(
            VideoCodec::H264,
            synthesized_pts_us,
            false,
            message.payload,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialScrcpyIngressAdapter {
    next_pts_us: u64,
    pts_step_us: u64,
}

impl OfficialScrcpyIngressAdapter {
    /// Creates a deterministic PTS synthesizer for official scrcpy messages.
    ///
    /// Use `pts_step_us=0` when no cadence assumption should be represented in
    /// diagnostics. Use a fixed step only when the caller has route launch
    /// context, such as a requested frame interval.
    pub fn new(initial_pts_us: u64, pts_step_us: u64) -> Self {
        Self {
            next_pts_us: initial_pts_us,
            pts_step_us,
        }
    }

    pub fn untimed() -> Self {
        Self::new(0, 0)
    }

    pub fn ingest_message(
        &mut self,
        message: OfficialScrcpyVideoMessage,
    ) -> HostResult<PreparedVideoIngress> {
        let pts_us = self.next_pts_us;
        self.next_pts_us = self.next_pts_us.saturating_add(self.pts_step_us);
        PreparedVideoIngress::from_official_scrcpy_message(message, pts_us)
    }
}

impl Default for OfficialScrcpyIngressAdapter {
    fn default() -> Self {
        Self::untimed()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_negotiated_video_path, NegotiatedVideoPath, OfficialScrcpyIngressAdapter,
        PreparedVideoIngress, VideoTransportPacket,
    };
    use crate::official_scrcpy::OfficialScrcpyVideoMessage;
    use hscrcpy_contracts::VideoCodec;

    #[test]
    fn classifies_jpeg_as_fallback_path() {
        assert_eq!(
            classify_negotiated_video_path(&VideoCodec::Jpeg),
            NegotiatedVideoPath::JpegFallback
        );
    }

    #[test]
    fn classifies_h264_as_mainline_path() {
        assert_eq!(
            classify_negotiated_video_path(&VideoCodec::H264),
            NegotiatedVideoPath::H264Mainline
        );
    }

    #[test]
    fn leaves_h265_as_experimental_path() {
        assert_eq!(
            classify_negotiated_video_path(&VideoCodec::H265Experimental),
            NegotiatedVideoPath::DeferredExperimental(VideoCodec::H265Experimental)
        );
    }

    #[test]
    fn decodes_jpeg_transport_packet_for_render_ingress() {
        let packet =
            VideoTransportPacket::decode(&[2, 0, 0, 0, 0, 0, 0, 0, 7, 1, 0, 0, 0, 3, 9, 8, 7])
                .expect("packet should decode");
        let ingress = PreparedVideoIngress::from_packet(packet).expect("ingress should prepare");

        match ingress {
            PreparedVideoIngress::Jpeg(frame) => {
                assert_eq!(frame.timestamp_micros, 7);
                assert_eq!(frame.encoded_bytes, vec![9, 8, 7]);
            }
            unexpected => panic!("unexpected ingress: {unexpected:?}"),
        }
    }

    #[test]
    fn maps_official_h264_message_to_route_neutral_ingress() {
        let payload = vec![0, 0, 1, 0x65, 0x88, 0x84];
        let mut adapter = OfficialScrcpyIngressAdapter::new(1_000, 16_666);

        let ingress = adapter
            .ingest_message(official_message(VideoCodec::H264, payload.clone()))
            .expect("official h264 payload should prepare");

        match ingress {
            PreparedVideoIngress::H264(access_unit) => {
                assert_eq!(access_unit.timestamp_micros, 1_000);
                assert!(!access_unit.is_keyframe);
                assert_eq!(access_unit.encoded_bytes, payload);
            }
            unexpected => panic!("unexpected ingress: {unexpected:?}"),
        }

        let second = adapter
            .ingest_message(official_message(
                VideoCodec::H264,
                vec![0, 0, 1, 0x41, 0x9a, 0x20],
            ))
            .expect("second official h264 payload should prepare");
        match second {
            PreparedVideoIngress::H264(access_unit) => {
                assert_eq!(access_unit.timestamp_micros, 17_666);
                assert!(!access_unit.is_keyframe);
            }
            unexpected => panic!("unexpected ingress: {unexpected:?}"),
        }
    }

    #[test]
    fn rejects_malformed_official_h264_payload() {
        let err = OfficialScrcpyIngressAdapter::default()
            .ingest_message(official_message(VideoCodec::H264, vec![0x65, 0x88, 0x84]))
            .expect_err("non-Annex-B official payload should fail");

        assert!(
            err.to_string().contains("Annex-B"),
            "unexpected error: {err}"
        );
        assert!(err.to_string().contains("h264"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_official_h264_empty_payload_before_renderer_handoff() {
        let err = OfficialScrcpyIngressAdapter::default()
            .ingest_message(official_message(VideoCodec::H264, Vec::new()))
            .expect_err("empty official h264 payload should fail");

        assert!(
            err.to_string()
                .contains("h264 access unit payload must not be empty"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_unsupported_official_codec_before_renderer_handoff() {
        let err = OfficialScrcpyIngressAdapter::default()
            .ingest_message(official_message(
                VideoCodec::H265Experimental,
                vec![0, 0, 1, 0x65, 0x88, 0x84],
            ))
            .expect_err("non-h264 official payload should fail");

        assert!(
            err.to_string().contains("only supports h264"),
            "unexpected error: {err}"
        );
        assert!(
            err.to_string().contains("reply_type=3"),
            "unexpected error: {err}"
        );
    }

    fn official_message(codec: VideoCodec, payload: Vec<u8>) -> OfficialScrcpyVideoMessage {
        OfficialScrcpyVideoMessage {
            codec,
            payload_key: "payload".to_string(),
            payload,
            reply_type: 3,
            data: Some("test-frame".to_string()),
        }
    }
}
