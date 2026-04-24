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
        let inspection = inspect_annex_b_access_unit(payload)?;
        if metadata.is_keyframe && !inspection.has_idr_nal {
            return Err(HostError::ContractViolation(
                "h264 keyframe payload must include an IDR NAL unit".to_string(),
            ));
        }

        Ok(H264AccessUnit {
            timestamp_micros: metadata.pts_us,
            is_keyframe: metadata.is_keyframe,
            encoded_bytes: payload.to_vec(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct H264Inspection {
    has_idr_nal: bool,
}

fn inspect_annex_b_access_unit(payload: &[u8]) -> HostResult<H264Inspection> {
    let Some((first_start_code, _)) = find_annex_b_start_code(payload, 0) else {
        return Err(HostError::ContractViolation(
            "h264 payload must use Annex-B start codes".to_string(),
        ));
    };
    if payload[..first_start_code].iter().any(|byte| *byte != 0) {
        return Err(HostError::ContractViolation(format!(
            "h264 payload contains non-zero prefix before first Annex-B start code at offset {first_start_code}"
        )));
    }

    let mut cursor = first_start_code;
    let mut has_idr_nal = false;
    let mut nal_count = 0;

    while let Some((start_code_offset, start_code_len)) = find_annex_b_start_code(payload, cursor) {
        let nal_header_offset = start_code_offset + start_code_len;
        if nal_header_offset >= payload.len() {
            return Err(HostError::ContractViolation(format!(
                "h264 Annex-B start code at offset {start_code_offset} is missing a NAL header"
            )));
        }

        let next_start_code_offset = find_annex_b_start_code(payload, nal_header_offset)
            .map(|(offset, _)| offset)
            .unwrap_or(payload.len());
        if next_start_code_offset <= nal_header_offset {
            return Err(HostError::ContractViolation(format!(
                "h264 NAL unit at offset {start_code_offset} has empty payload"
            )));
        }

        let nal_header = payload[nal_header_offset];
        if nal_header & 0x80 != 0 {
            return Err(HostError::ContractViolation(format!(
                "h264 NAL header at offset {nal_header_offset} has forbidden_zero_bit set"
            )));
        }

        let nal_type = nal_header & 0x1f;
        if nal_type == 0 {
            return Err(HostError::ContractViolation(format!(
                "h264 NAL header at offset {nal_header_offset} has invalid type 0"
            )));
        }
        if nal_type == 5 {
            has_idr_nal = true;
        }
        nal_count += 1;
        cursor = next_start_code_offset;
    }

    if nal_count == 0 {
        return Err(HostError::ContractViolation(
            "h264 payload did not contain any NAL units".to_string(),
        ));
    }

    Ok(H264Inspection { has_idr_nal })
}

fn find_annex_b_start_code(payload: &[u8], from_offset: usize) -> Option<(usize, usize)> {
    if payload.len() < 3 || from_offset >= payload.len() {
        return None;
    }

    let mut offset = from_offset;
    while offset + 3 <= payload.len() {
        if payload[offset] == 0 && payload[offset + 1] == 0 {
            if payload[offset + 2] == 1 {
                return Some((offset, 3));
            }
            if offset + 3 < payload.len() && payload[offset + 2] == 0 && payload[offset + 3] == 1 {
                return Some((offset, 4));
            }
        }
        offset += 1;
    }
    None
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
    fn rejects_payload_without_annex_b_start_codes() {
        let path = H264VideoPath;
        let err = path
            .ingest_unit(
                &VideoUnitMetadata {
                    codec: VideoCodec::H264,
                    pts_us: 345,
                    is_keyframe: true,
                    payload_length: 3,
                },
                &[0x65, 0x88, 0x84],
            )
            .expect_err("non Annex-B payload should be rejected");
        assert!(
            err.to_string().contains("Annex-B start codes"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_keyframe_payload_without_idr_nal() {
        let path = H264VideoPath;
        let payload = [0, 0, 1, 0x41, 0x9a, 0x20];
        let err = path
            .ingest_unit(
                &VideoUnitMetadata {
                    codec: VideoCodec::H264,
                    pts_us: 345,
                    is_keyframe: true,
                    payload_length: payload.len(),
                },
                &payload,
            )
            .expect_err("keyframe payload without IDR should fail");
        assert!(
            err.to_string().contains("must include an IDR"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_forbidden_zero_bit_in_nal_header() {
        let path = H264VideoPath;
        let payload = [0, 0, 1, 0xE5, 0x88, 0x84];
        let err = path
            .ingest_unit(
                &VideoUnitMetadata {
                    codec: VideoCodec::H264,
                    pts_us: 345,
                    is_keyframe: true,
                    payload_length: payload.len(),
                },
                &payload,
            )
            .expect_err("forbidden_zero_bit should be rejected");
        assert!(
            err.to_string().contains("forbidden_zero_bit"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn maps_annex_b_h264_unit_to_access_unit() {
        let path = H264VideoPath;
        let payload = [
            0, 0, 0, 1, 0x67, 0x42, 0x00, 0x1f, 0x95, 0xa8, 0x14, 0x01, 0x6e, 0x40, 0, 0, 1, 0x68,
            0xce, 0x06, 0xe2, 0, 0, 1, 0x65, 0x88, 0x84,
        ];
        let unit = path
            .ingest_unit(
                &VideoUnitMetadata {
                    codec: VideoCodec::H264,
                    pts_us: 345,
                    is_keyframe: true,
                    payload_length: payload.len(),
                },
                &payload,
            )
            .expect("h264 unit should pass mainline ingestion");
        assert_eq!(unit.timestamp_micros, 345);
        assert!(unit.is_keyframe);
        assert_eq!(unit.encoded_bytes, payload.to_vec());
    }
}
