use crate::{HostError, HostResult};
use hscrcpy_contracts::VideoCodec;

pub mod h264;
pub mod jpeg;

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
}

#[cfg(test)]
mod tests {
    use super::{
        classify_negotiated_video_path, NegotiatedVideoPath, PreparedVideoIngress,
        VideoTransportPacket,
    };
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
}
