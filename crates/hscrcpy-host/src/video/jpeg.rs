use super::VideoUnitMetadata;
use crate::render::JpegFrame;
use crate::{HostError, HostResult};
use hscrcpy_contracts::VideoCodec;

#[derive(Debug, Default, Clone)]
pub struct JpegVideoPath;

impl JpegVideoPath {
    pub fn ingest_unit(
        &self,
        metadata: &VideoUnitMetadata,
        payload: &[u8],
    ) -> HostResult<JpegFrame> {
        if metadata.codec != VideoCodec::Jpeg {
            return Err(HostError::ContractViolation(format!(
                "jpeg path received incompatible codec: {:?}",
                metadata.codec
            )));
        }

        metadata.validate_payload_length(payload)?;
        if payload.is_empty() {
            return Err(HostError::ContractViolation(
                "jpeg frame payload must not be empty".to_string(),
            ));
        }

        Ok(JpegFrame {
            timestamp_micros: metadata.pts_us,
            encoded_bytes: payload.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::JpegVideoPath;
    use crate::video::VideoUnitMetadata;
    use hscrcpy_contracts::VideoCodec;

    #[test]
    fn rejects_non_jpeg_units() {
        let path = JpegVideoPath;
        let err = path
            .ingest_unit(
                &VideoUnitMetadata {
                    codec: VideoCodec::H264,
                    pts_us: 100,
                    is_keyframe: true,
                    payload_length: 3,
                },
                &[1, 2, 3],
            )
            .expect_err("non-jpeg codec should be rejected");
        assert!(
            err.to_string().contains("incompatible codec"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validates_payload_length_before_routing() {
        let path = JpegVideoPath;
        let err = path
            .ingest_unit(
                &VideoUnitMetadata {
                    codec: VideoCodec::Jpeg,
                    pts_us: 100,
                    is_keyframe: true,
                    payload_length: 5,
                },
                &[1, 2, 3],
            )
            .expect_err("length mismatch should be rejected");
        assert!(
            err.to_string().contains("payload length mismatch"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn maps_jpeg_unit_to_render_frame() {
        let path = JpegVideoPath;
        let frame = path
            .ingest_unit(
                &VideoUnitMetadata {
                    codec: VideoCodec::Jpeg,
                    pts_us: 123,
                    is_keyframe: true,
                    payload_length: 3,
                },
                &[1, 2, 3],
            )
            .expect("jpeg frame should pass baseline ingestion");
        assert_eq!(frame.timestamp_micros, 123);
        assert_eq!(frame.encoded_bytes, vec![1, 2, 3]);
    }
}
