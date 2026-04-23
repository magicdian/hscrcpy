use crate::{ChannelEndpoint, ChannelLayout, VideoCodec};
use std::error::Error;
use std::fmt;

pub const PROTOCOL_MAJOR_MVP: u16 = 1;
pub const PROTOCOL_MINOR_MVP: u16 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionFeature {
    Video,
    Control,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationState {
    Granted,
    NeedsUserAction,
    Denied,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureAuthorization {
    pub video_capture: AuthorizationState,
    pub input_injection: AuthorizationState,
}

impl FeatureAuthorization {
    pub fn required_features_granted(&self, control_enabled: bool) -> bool {
        self.video_capture == AuthorizationState::Granted
            && (!control_enabled || self.input_injection == AuthorizationState::Granted)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStartRequest {
    pub requested_codec_order: Vec<VideoCodec>,
    pub preferred_max_fps: u16,
    pub enable_control: bool,
}

impl SessionStartRequest {
    pub fn h264_mainline(preferred_max_fps: u16, enable_control: bool) -> Self {
        Self {
            requested_codec_order: vec![VideoCodec::H264, VideoCodec::Jpeg],
            preferred_max_fps,
            enable_control,
        }
    }

    pub fn jpeg_baseline(preferred_max_fps: u16, enable_control: bool) -> Self {
        Self {
            requested_codec_order: vec![VideoCodec::Jpeg],
            preferred_max_fps,
            enable_control,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionControlState {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStartResponse {
    pub session_id: String,
    pub selected_codec: VideoCodec,
    pub session_channel: ChannelEndpoint,
    pub video_channel: ChannelEndpoint,
    pub control: SessionControlState,
    pub authorization: FeatureAuthorization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostVideoLimits {
    pub max_width: u16,
    pub max_height: u16,
    pub max_fps: u16,
    pub bitrate_kbps: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayInfo {
    pub width: u16,
    pub height: u16,
    pub rotation: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoCodecDescriptor {
    pub codec: VideoCodec,
    pub encoder_kind: String,
    pub max_width: u16,
    pub max_height: u16,
    pub max_fps: u16,
    pub bitrate_control: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostHello {
    pub session_id: String,
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub host_version: String,
    pub requested_features: Vec<SessionFeature>,
    pub supported_video_codecs: Vec<VideoCodec>,
    pub preferred_video_codecs: Vec<VideoCodec>,
    pub video_limits: HostVideoLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionVideoConfig {
    pub max_width: u16,
    pub max_height: u16,
    pub max_fps: u16,
    pub bitrate_kbps: Option<u32>,
    pub iframe_interval_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionControlConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    pub session_id: String,
    pub selected_video_codec: VideoCodec,
    pub video: SessionVideoConfig,
    pub control: SessionControlConfig,
    pub rotation_locked: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopSession {
    pub session_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceHello {
    pub session_id: String,
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub companion_version: String,
    pub device_name: String,
    pub authorization: FeatureAuthorization,
    pub available_features: Vec<SessionFeature>,
    pub available_video_codecs: Vec<VideoCodecDescriptor>,
    pub display: DisplayInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationUpdate {
    pub session_id: String,
    pub authorization: FeatureAuthorization,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionReady {
    pub session_id: String,
    pub selected_video_codec: VideoCodec,
    pub channel_layout: ChannelLayout,
    pub display: DisplayInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionError {
    pub session_id: String,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedPosition {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedPositionError {
    XOutOfRange,
    YOutOfRange,
}

impl fmt::Display for NormalizedPositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::XOutOfRange => write!(f, "position_norm.x must be within [0.0, 1.0]"),
            Self::YOutOfRange => write!(f, "position_norm.y must be within [0.0, 1.0]"),
        }
    }
}

impl Error for NormalizedPositionError {}

impl NormalizedPosition {
    pub fn new(x: f32, y: f32) -> Result<Self, NormalizedPositionError> {
        if !(0.0..=1.0).contains(&x) {
            return Err(NormalizedPositionError::XOutOfRange);
        }
        if !(0.0..=1.0).contains(&y) {
            return Err(NormalizedPositionError::YOutOfRange);
        }
        Ok(Self { x, y })
    }

    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> f32 {
        self.y
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlEventType {
    PointerDown,
    PointerMove,
    PointerUp,
    Scroll,
    KeyDown,
    KeyUp,
    DeviceAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceAction {
    Back,
    Home,
    Named(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlEvent {
    pub session_id: String,
    pub event_type: ControlEventType,
    pub sequence: u64,
    pub position_norm: Option<NormalizedPosition>,
    pub pointer_id: Option<u32>,
    pub button: Option<PointerButton>,
    pub scroll_delta_x: Option<i32>,
    pub scroll_delta_y: Option<i32>,
    pub key_code: Option<String>,
    pub text: Option<String>,
    pub device_action: Option<DeviceAction>,
}

impl ControlEvent {
    pub fn pointer_down(
        session_id: impl Into<String>,
        sequence: u64,
        position_norm: NormalizedPosition,
        pointer_id: u32,
        button: PointerButton,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            event_type: ControlEventType::PointerDown,
            sequence,
            position_norm: Some(position_norm),
            pointer_id: Some(pointer_id),
            button: Some(button),
            scroll_delta_x: None,
            scroll_delta_y: None,
            key_code: None,
            text: None,
            device_action: None,
        }
    }

    pub fn pointer_move(
        session_id: impl Into<String>,
        sequence: u64,
        position_norm: NormalizedPosition,
        pointer_id: u32,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            event_type: ControlEventType::PointerMove,
            sequence,
            position_norm: Some(position_norm),
            pointer_id: Some(pointer_id),
            button: None,
            scroll_delta_x: None,
            scroll_delta_y: None,
            key_code: None,
            text: None,
            device_action: None,
        }
    }

    pub fn pointer_up(
        session_id: impl Into<String>,
        sequence: u64,
        position_norm: NormalizedPosition,
        pointer_id: u32,
        button: PointerButton,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            event_type: ControlEventType::PointerUp,
            sequence,
            position_norm: Some(position_norm),
            pointer_id: Some(pointer_id),
            button: Some(button),
            scroll_delta_x: None,
            scroll_delta_y: None,
            key_code: None,
            text: None,
            device_action: None,
        }
    }

    pub fn scroll(
        session_id: impl Into<String>,
        sequence: u64,
        position_norm: NormalizedPosition,
        scroll_delta_x: i32,
        scroll_delta_y: i32,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            event_type: ControlEventType::Scroll,
            sequence,
            position_norm: Some(position_norm),
            pointer_id: None,
            button: None,
            scroll_delta_x: Some(scroll_delta_x),
            scroll_delta_y: Some(scroll_delta_y),
            key_code: None,
            text: None,
            device_action: None,
        }
    }

    pub fn key_down(
        session_id: impl Into<String>,
        sequence: u64,
        key_code: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            event_type: ControlEventType::KeyDown,
            sequence,
            position_norm: None,
            pointer_id: None,
            button: None,
            scroll_delta_x: None,
            scroll_delta_y: None,
            key_code: Some(key_code.into()),
            text: None,
            device_action: None,
        }
    }

    pub fn key_up(
        session_id: impl Into<String>,
        sequence: u64,
        key_code: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            event_type: ControlEventType::KeyUp,
            sequence,
            position_norm: None,
            pointer_id: None,
            button: None,
            scroll_delta_x: None,
            scroll_delta_y: None,
            key_code: Some(key_code.into()),
            text: None,
            device_action: None,
        }
    }

    pub fn device_action(
        session_id: impl Into<String>,
        sequence: u64,
        device_action: DeviceAction,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            event_type: ControlEventType::DeviceAction,
            sequence,
            position_norm: None,
            pointer_id: None,
            button: None,
            scroll_delta_x: None,
            scroll_delta_y: None,
            key_code: None,
            text: None,
            device_action: Some(device_action),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionMessage {
    HostHello(HostHello),
    SessionConfig(SessionConfig),
    StopSession(StopSession),
    ControlEvent(ControlEvent),
}

impl SessionMessage {
    pub fn message_type(&self) -> &'static str {
        match self {
            Self::HostHello(_) => "host_hello",
            Self::SessionConfig(_) => "session_config",
            Self::StopSession(_) => "stop_session",
            Self::ControlEvent(_) => "control_event",
        }
    }

    pub fn session_id(&self) -> &str {
        match self {
            Self::HostHello(message) => &message.session_id,
            Self::SessionConfig(message) => &message.session_id,
            Self::StopSession(message) => &message.session_id,
            Self::ControlEvent(message) => &message.session_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundSessionMessage {
    DeviceHello(DeviceHello),
    AuthorizationUpdate(AuthorizationUpdate),
    SessionReady(SessionReady),
    SessionError(SessionError),
}

impl InboundSessionMessage {
    pub fn message_type(&self) -> &'static str {
        match self {
            Self::DeviceHello(_) => "device_hello",
            Self::AuthorizationUpdate(_) => "authorization_update",
            Self::SessionReady(_) => "session_ready",
            Self::SessionError(_) => "session_error",
        }
    }

    pub fn session_id(&self) -> &str {
        match self {
            Self::DeviceHello(message) => &message.session_id,
            Self::AuthorizationUpdate(message) => &message.session_id,
            Self::SessionReady(message) => &message.session_id,
            Self::SessionError(message) => &message.session_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuthorizationState, FeatureAuthorization, NormalizedPosition, NormalizedPositionError,
        SessionStartRequest,
    };
    use crate::VideoCodec;

    #[test]
    fn builds_h264_mainline_request() {
        let request = SessionStartRequest::h264_mainline(60, false);
        assert_eq!(
            request.requested_codec_order,
            vec![VideoCodec::H264, VideoCodec::Jpeg]
        );
        assert_eq!(request.preferred_max_fps, 60);
        assert!(!request.enable_control);
    }

    #[test]
    fn builds_jpeg_baseline_request() {
        let request = SessionStartRequest::jpeg_baseline(30, true);
        assert_eq!(request.requested_codec_order, vec![VideoCodec::Jpeg]);
        assert_eq!(request.preferred_max_fps, 30);
        assert!(request.enable_control);
    }

    #[test]
    fn requires_input_injection_only_when_control_is_enabled() {
        let authorization = FeatureAuthorization {
            video_capture: AuthorizationState::Granted,
            input_injection: AuthorizationState::Denied,
        };

        assert!(authorization.required_features_granted(false));
        assert!(!authorization.required_features_granted(true));
    }

    #[test]
    fn rejects_out_of_range_normalized_coordinates() {
        let x_err = NormalizedPosition::new(-0.01, 0.3).expect_err("x should be validated");
        assert_eq!(x_err, NormalizedPositionError::XOutOfRange);

        let y_err = NormalizedPosition::new(0.3, 1.01).expect_err("y should be validated");
        assert_eq!(y_err, NormalizedPositionError::YOutOfRange);
    }
}
