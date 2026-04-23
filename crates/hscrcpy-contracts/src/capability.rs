#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    Jpeg,
    H265Experimental,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReport {
    pub device_id: String,
    pub supported_video_codecs: Vec<VideoCodec>,
    pub supports_control: bool,
    pub supports_audio: bool,
}
