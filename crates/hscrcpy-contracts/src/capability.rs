#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    Jpeg,
    H265Experimental,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CodecName(String);

impl CodecName {
    pub const H264: &'static str = "h264";
    pub const H265: &'static str = "h265";
    pub const H266: &'static str = "h266";
    pub const VP9: &'static str = "vp9";
    pub const AV1: &'static str = "av1";
    pub const JPEG: &'static str = "jpeg";

    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into().to_ascii_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn known_registry() -> Vec<Self> {
        vec![
            Self::new(Self::H264),
            Self::new(Self::H265),
            Self::new(Self::H266),
            Self::new(Self::VP9),
            Self::new(Self::AV1),
            Self::new(Self::JPEG),
        ]
    }

    pub fn to_video_codec(&self) -> Option<VideoCodec> {
        match self.as_str() {
            Self::H264 => Some(VideoCodec::H264),
            Self::H265 => Some(VideoCodec::H265Experimental),
            Self::JPEG => Some(VideoCodec::Jpeg),
            _ => None,
        }
    }
}

impl std::fmt::Display for CodecName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for CodecName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for CodecName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&VideoCodec> for CodecName {
    fn from(value: &VideoCodec) -> Self {
        match value {
            VideoCodec::H264 => Self::new(Self::H264),
            VideoCodec::Jpeg => Self::new(Self::JPEG),
            VideoCodec::H265Experimental => Self::new(Self::H265),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecDescriptor {
    pub codec_name: CodecName,
    pub encoder_kind: String,
    pub max_width: u16,
    pub max_height: u16,
    pub max_fps: u16,
    pub bitrate_control: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteVideoEncoderCapability {
    pub supported_codecs: Vec<CodecDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostVideoDecoderCapability {
    pub supported_codecs: Vec<CodecName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReport {
    pub device_id: String,
    pub supported_video_codecs: Vec<VideoCodec>,
    pub supports_control: bool,
    pub supports_audio: bool,
}

impl CapabilityReport {
    pub fn supported_codec_names(&self) -> Vec<CodecName> {
        self.supported_video_codecs
            .iter()
            .map(CodecName::from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{CodecName, VideoCodec};

    #[test]
    fn includes_known_codec_registry_names() {
        let names = CodecName::known_registry();
        assert!(names.contains(&CodecName::new(CodecName::H264)));
        assert!(names.contains(&CodecName::new(CodecName::H265)));
        assert!(names.contains(&CodecName::new(CodecName::H266)));
        assert!(names.contains(&CodecName::new(CodecName::VP9)));
        assert!(names.contains(&CodecName::new(CodecName::AV1)));
        assert!(names.contains(&CodecName::new(CodecName::JPEG)));
    }

    #[test]
    fn converts_known_codec_names_to_legacy_video_codec() {
        assert_eq!(
            CodecName::new(CodecName::H264).to_video_codec(),
            Some(VideoCodec::H264)
        );
        assert_eq!(
            CodecName::new(CodecName::JPEG).to_video_codec(),
            Some(VideoCodec::Jpeg)
        );
        assert_eq!(CodecName::new(CodecName::AV1).to_video_codec(), None);
    }
}
