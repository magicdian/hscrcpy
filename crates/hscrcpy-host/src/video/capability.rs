use crate::{HostError, HostResult};
use hscrcpy_contracts::{CodecName, SessionStartRequest, VideoCodec, VideoCodecDescriptor};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCodecCapability {
    pub supported_codecs: Vec<CodecName>,
}

impl HostCodecCapability {
    pub fn mvp_h264_jpeg() -> Self {
        Self {
            supported_codecs: vec![
                CodecName::new(CodecName::H264),
                CodecName::new(CodecName::JPEG),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecSelectionDiagnostics {
    pub preferred_codecs: Vec<CodecName>,
    pub route_supported_codecs: Vec<CodecName>,
    pub host_supported_codecs: Vec<CodecName>,
    pub selected_codec: Option<CodecName>,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecSelectionResult {
    pub selected_codec: VideoCodec,
    pub diagnostics: CodecSelectionDiagnostics,
}

pub fn select_codec_for_startup(
    request: &SessionStartRequest,
    route_supported_codecs: &[VideoCodecDescriptor],
    host_capability: &HostCodecCapability,
) -> HostResult<CodecSelectionResult> {
    let preferred_codecs = normalized_requested_codec_names(&request.requested_codec_order);
    let route_supported_names = dedupe_codec_names(
        route_supported_codecs
            .iter()
            .map(VideoCodecDescriptor::codec_name)
            .collect(),
    );
    let host_supported_names = dedupe_codec_names(host_capability.supported_codecs.clone());

    let mut skipped_reasons = Vec::new();
    for preferred in &preferred_codecs {
        if !host_supported_names.contains(preferred) {
            skipped_reasons.push(format!("`{preferred}` unsupported by host decoder"));
            continue;
        }
        if !route_supported_names.contains(preferred) {
            skipped_reasons.push(format!("`{preferred}` unsupported by route/device encoder"));
            continue;
        }

        let selected_codec = preferred.to_video_codec().ok_or_else(|| {
            HostError::ContractViolation(format!(
                "codec `{preferred}` is shared but not implemented for host runtime yet"
            ))
        })?;
        let selected_codec_name = preferred.clone();
        let fallback_reason = if skipped_reasons.is_empty() {
            None
        } else {
            Some(format!(
                "{}; selected `{preferred}`",
                skipped_reasons.join(", ")
            ))
        };

        return Ok(CodecSelectionResult {
            selected_codec,
            diagnostics: CodecSelectionDiagnostics {
                preferred_codecs: preferred_codecs.clone(),
                route_supported_codecs: route_supported_names.clone(),
                host_supported_codecs: host_supported_names.clone(),
                selected_codec: Some(selected_codec_name),
                fallback_reason,
            },
        });
    }

    Err(HostError::ContractViolation(format!(
        "no_shared_video_codec: preferred=[{}] route_supported=[{}] host_supported=[{}]",
        join_codec_names(&preferred_codecs),
        join_codec_names(&route_supported_names),
        join_codec_names(&host_supported_names),
    )))
}

pub fn normalized_requested_codec_order(requested_order: &[VideoCodec]) -> Vec<VideoCodec> {
    normalized_requested_codec_names(requested_order)
        .into_iter()
        .filter_map(|codec| codec.to_video_codec())
        .collect()
}

fn normalized_requested_codec_names(requested_order: &[VideoCodec]) -> Vec<CodecName> {
    if requested_order.is_empty() {
        return vec![
            CodecName::new(CodecName::H264),
            CodecName::new(CodecName::JPEG),
        ];
    }

    let mut normalized = dedupe_codec_names(
        requested_order
            .iter()
            .map(CodecName::from)
            .collect::<Vec<_>>(),
    );
    let h264 = CodecName::new(CodecName::H264);
    let jpeg = CodecName::new(CodecName::JPEG);
    if normalized.contains(&h264) && !normalized.contains(&jpeg) {
        normalized.push(jpeg);
    }
    normalized
}

fn dedupe_codec_names(codecs: Vec<CodecName>) -> Vec<CodecName> {
    let mut unique = Vec::new();
    for codec in codecs {
        if !unique.contains(&codec) {
            unique.push(codec);
        }
    }
    unique
}

fn join_codec_names(codecs: &[CodecName]) -> String {
    codecs
        .iter()
        .map(|codec| codec.as_str().to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::{normalized_requested_codec_order, select_codec_for_startup, HostCodecCapability};
    use hscrcpy_contracts::{CodecName, SessionStartRequest, VideoCodec, VideoCodecDescriptor};

    fn descriptor(codec: VideoCodec) -> VideoCodecDescriptor {
        VideoCodecDescriptor {
            codec,
            encoder_kind: "hardware".to_string(),
            max_width: 1920,
            max_height: 1080,
            max_fps: 60,
            bitrate_control: Some("vbr".to_string()),
        }
    }

    #[test]
    fn selects_h264_when_preferred_and_shared() {
        let result = select_codec_for_startup(
            &SessionStartRequest::h264_mainline(60, false),
            &[descriptor(VideoCodec::H264), descriptor(VideoCodec::Jpeg)],
            &HostCodecCapability::mvp_h264_jpeg(),
        )
        .expect("h264 should be selected");

        assert_eq!(result.selected_codec, VideoCodec::H264);
        assert_eq!(
            result.diagnostics.selected_codec,
            Some(CodecName::new(CodecName::H264))
        );
        assert!(result.diagnostics.fallback_reason.is_none());
    }

    #[test]
    fn falls_back_to_jpeg_when_preferred_codec_is_not_available_on_route() {
        let result = select_codec_for_startup(
            &SessionStartRequest::h264_mainline(60, false),
            &[descriptor(VideoCodec::Jpeg)],
            &HostCodecCapability::mvp_h264_jpeg(),
        )
        .expect("jpeg fallback should be selected");

        assert_eq!(result.selected_codec, VideoCodec::Jpeg);
        let fallback_reason = result
            .diagnostics
            .fallback_reason
            .as_deref()
            .expect("fallback reason should be present");
        assert!(fallback_reason.contains("unsupported by route/device encoder"));
        assert!(fallback_reason.contains("selected `jpeg`"));
    }

    #[test]
    fn fails_when_no_shared_codec_exists() {
        let err = select_codec_for_startup(
            &SessionStartRequest::jpeg_baseline(60, false),
            &[descriptor(VideoCodec::H264)],
            &HostCodecCapability {
                supported_codecs: vec![CodecName::new(CodecName::JPEG)],
            },
        )
        .expect_err("selection should fail without shared codecs");
        assert!(err.to_string().contains("no_shared_video_codec"));
        assert!(err.to_string().contains("preferred=[jpeg]"));
    }

    #[test]
    fn normalizes_h264_request_with_jpeg_fallback_for_host_hello() {
        assert_eq!(
            normalized_requested_codec_order(&[VideoCodec::H264]),
            vec![VideoCodec::H264, VideoCodec::Jpeg]
        );
    }
}
