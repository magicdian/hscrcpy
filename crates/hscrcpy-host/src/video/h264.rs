use super::VideoUnitMetadata;
use crate::{HostError, HostResult};
use hscrcpy_contracts::VideoCodec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct H264AccessUnit {
    pub timestamp_micros: u64,
    pub is_keyframe: bool,
    pub encoded_bytes: Vec<u8>,
}

#[derive(Debug, Default, Clone)]
pub struct H264VideoPath;

impl H264VideoPath {
    pub fn ingest_unit(
        &self,
        metadata: &VideoUnitMetadata,
        payload: &[u8],
    ) -> HostResult<H264AccessUnit> {
        if metadata.codec != VideoCodec::H264 {
            return Err(HostError::ContractViolation(format!(
                "h264 path received incompatible codec: {:?}",
                metadata.codec
            )));
        }

        metadata.validate_payload_length(payload)?;
        if payload.is_empty() {
            return Err(HostError::ContractViolation(
                "h264 access unit payload must not be empty".to_string(),
            ));
        }

        Ok(H264AccessUnit {
            timestamp_micros: metadata.pts_us,
            is_keyframe: metadata.is_keyframe,
            encoded_bytes: payload.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::H264VideoPath;
    use crate::video::VideoUnitMetadata;
    use hscrcpy_contracts::VideoCodec;

    #[test]
    fn rejects_non_h264_units() {
        let path = H264VideoPath;
        let err = path
            .ingest_unit(
                &VideoUnitMetadata {
                    codec: VideoCodec::Jpeg,
                    pts_us: 200,
                    is_keyframe: true,
                    payload_length: 3,
                },
                &[1, 2, 3],
            )
            .expect_err("non-h264 codec should be rejected");
        assert!(
            err.to_string().contains("incompatible codec"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validates_payload_length_before_routing() {
        let path = H264VideoPath;
        let err = path
            .ingest_unit(
                &VideoUnitMetadata {
                    codec: VideoCodec::H264,
                    pts_us: 200,
                    is_keyframe: false,
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
    fn maps_h264_unit_to_access_unit() {
        let path = H264VideoPath;
        let unit = path
            .ingest_unit(
                &VideoUnitMetadata {
                    codec: VideoCodec::H264,
                    pts_us: 345,
                    is_keyframe: true,
                    payload_length: 4,
                },
                &[9, 8, 7, 6],
            )
            .expect("h264 unit should pass mainline ingestion");
        assert_eq!(unit.timestamp_micros, 345);
        assert!(unit.is_keyframe);
        assert_eq!(unit.encoded_bytes, vec![9, 8, 7, 6]);
    }
}
