use crate::control::SessionMessageSink;
use crate::video::{PreparedVideoIngress, VideoTransportPacket};
use crate::{HostError, HostResult};
use hscrcpy_contracts::{
    AuthorizationState, AuthorizationUpdate, ChannelBinding, ChannelEndpoint, ChannelLayout,
    ControlEvent, DeviceAction, DeviceHello, DisplayInfo, FeatureAuthorization, HostHello,
    InboundSessionMessage, PointerButton, SessionConfig, SessionError, SessionFeature,
    SessionMessage, SessionReady, StopSession, VideoCodec, VideoCodecDescriptor,
};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::str::FromStr;

const VIDEO_PACKET_HEADER_LEN: usize = 14;

pub trait SessionChannelTransport {
    fn send_message(&mut self, message: &SessionMessage) -> HostResult<()>;
    fn receive_message(&mut self) -> HostResult<InboundSessionMessage>;
}

pub trait VideoChannelTransport {
    fn receive_packet(&mut self) -> HostResult<VideoTransportPacket>;
}

pub trait SessionTransportFactory {
    fn open_session_channel(
        &self,
        endpoint: &ChannelEndpoint,
    ) -> HostResult<Box<dyn SessionChannelTransport>>;
    fn open_video_channel(
        &self,
        endpoint: &ChannelEndpoint,
        selected_codec: VideoCodec,
    ) -> HostResult<Box<dyn VideoChannelTransport>>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TcpSessionTransportFactory;

impl SessionTransportFactory for TcpSessionTransportFactory {
    fn open_session_channel(
        &self,
        endpoint: &ChannelEndpoint,
    ) -> HostResult<Box<dyn SessionChannelTransport>> {
        let stream = connect_stream(endpoint)?;
        Ok(Box::new(TcpSessionChannel::new(stream)?))
    }

    fn open_video_channel(
        &self,
        endpoint: &ChannelEndpoint,
        selected_codec: VideoCodec,
    ) -> HostResult<Box<dyn VideoChannelTransport>> {
        let stream = connect_stream(endpoint)?;
        Ok(Box::new(TcpVideoChannel::new(stream, selected_codec)?))
    }
}

pub struct SessionRuntime {
    session_id: String,
    selected_codec: VideoCodec,
    session_channel: Box<dyn SessionChannelTransport>,
    video_channel: Box<dyn VideoChannelTransport>,
}

impl SessionRuntime {
    pub fn new(
        session_id: impl Into<String>,
        selected_codec: VideoCodec,
        session_channel: Box<dyn SessionChannelTransport>,
        video_channel: Box<dyn VideoChannelTransport>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            selected_codec,
            session_channel,
            video_channel,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn selected_codec(&self) -> &VideoCodec {
        &self.selected_codec
    }

    pub fn receive_video_ingress(&mut self) -> HostResult<PreparedVideoIngress> {
        let packet = self.video_channel.receive_packet()?;
        PreparedVideoIngress::from_packet(packet)
    }

    pub fn stop(&mut self, reason: Option<String>) -> HostResult<()> {
        self.session_channel
            .send_message(&SessionMessage::StopSession(StopSession {
                session_id: self.session_id.clone(),
                reason,
            }))
    }
}

impl SessionMessageSink for SessionRuntime {
    fn emit(&mut self, message: SessionMessage) -> HostResult<()> {
        self.session_channel.send_message(&message)
    }
}

fn connect_stream(endpoint: &ChannelEndpoint) -> HostResult<TcpStream> {
    let addr = socket_addr(endpoint)?;
    let stream = TcpStream::connect(addr).map_err(|error| {
        HostError::TransportFailure(format!(
            "failed to connect to {} over {:?}: {error}",
            endpoint.target, endpoint.transport
        ))
    })?;
    stream.set_nodelay(true).map_err(|error| {
        HostError::TransportFailure(format!("failed to set TCP_NODELAY: {error}"))
    })?;
    Ok(stream)
}

pub fn endpoint_port(endpoint: &ChannelEndpoint) -> HostResult<u16> {
    Ok(socket_addr(endpoint)?.port())
}

fn socket_addr(endpoint: &ChannelEndpoint) -> HostResult<SocketAddr> {
    SocketAddr::from_str(&endpoint.target).map_err(|error| {
        HostError::ContractViolation(format!(
            "channel endpoint target `{}` is not a socket address: {error}",
            endpoint.target
        ))
    })
}

struct TcpSessionChannel {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl TcpSessionChannel {
    fn new(stream: TcpStream) -> HostResult<Self> {
        let writer = stream.try_clone().map_err(|error| {
            HostError::TransportFailure(format!("failed to clone TCP stream: {error}"))
        })?;
        Ok(Self {
            reader: BufReader::new(stream),
            writer,
        })
    }
}

impl SessionChannelTransport for TcpSessionChannel {
    fn send_message(&mut self, message: &SessionMessage) -> HostResult<()> {
        let encoded = encode_session_message(message);
        self.writer
            .write_all(encoded.as_bytes())
            .and_then(|_| self.writer.write_all(b"\n"))
            .and_then(|_| self.writer.flush())
            .map_err(|error| {
                HostError::TransportFailure(format!("failed to write session message: {error}"))
            })
    }

    fn receive_message(&mut self) -> HostResult<InboundSessionMessage> {
        let mut line = String::new();
        let bytes = self.reader.read_line(&mut line).map_err(|error| {
            HostError::TransportFailure(format!("failed to read session message: {error}"))
        })?;
        if bytes == 0 {
            return Err(HostError::TransportFailure(
                "session channel closed while waiting for handshake data".to_string(),
            ));
        }
        decode_inbound_session_message(line.trim())
    }
}

struct TcpVideoChannel {
    stream: TcpStream,
    selected_codec: VideoCodec,
}

impl TcpVideoChannel {
    fn new(stream: TcpStream, selected_codec: VideoCodec) -> HostResult<Self> {
        stream.set_nodelay(true).map_err(|error| {
            HostError::TransportFailure(format!("failed to set TCP_NODELAY: {error}"))
        })?;
        Ok(Self {
            stream,
            selected_codec,
        })
    }
}

impl VideoChannelTransport for TcpVideoChannel {
    fn receive_packet(&mut self) -> HostResult<VideoTransportPacket> {
        let mut header = [0_u8; VIDEO_PACKET_HEADER_LEN];
        self.stream.read_exact(&mut header).map_err(|error| {
            HostError::TransportFailure(format!("failed to read video packet header: {error}"))
        })?;

        let codec = decode_video_codec(header[0])?;
        if codec != self.selected_codec {
            return Err(HostError::ContractViolation(format!(
                "video packet codec {:?} does not match negotiated session codec {:?}",
                codec, self.selected_codec
            )));
        }

        let pts_us = u64::from_be_bytes(header[1..9].try_into().expect("slice length"));
        let is_keyframe = match header[9] {
            0 => false,
            1 => true,
            flag => {
                return Err(HostError::ContractViolation(format!(
                    "video packet keyframe flag must be 0 or 1, got {flag}"
                )))
            }
        };
        let payload_length = u32::from_be_bytes(header[10..14].try_into().expect("slice length"));
        if payload_length == 0 {
            return Err(HostError::ContractViolation(
                "video packet payload_length must be positive".to_string(),
            ));
        }

        let mut payload = vec![0_u8; payload_length as usize];
        self.stream.read_exact(&mut payload).map_err(|error| {
            HostError::TransportFailure(format!("failed to read video packet payload: {error}"))
        })?;

        Ok(VideoTransportPacket::new(
            codec,
            pts_us,
            is_keyframe,
            payload,
        ))
    }
}

fn encode_session_message(message: &SessionMessage) -> String {
    match message {
        SessionMessage::HostHello(host_hello) => encode_host_hello(host_hello),
        SessionMessage::SessionConfig(config) => encode_session_config(config),
        SessionMessage::StopSession(stop) => encode_stop_session(stop),
        SessionMessage::ControlEvent(event) => encode_control_event(event),
    }
}

fn encode_host_hello(message: &HostHello) -> String {
    let requested_features = message
        .requested_features
        .iter()
        .map(encode_session_feature)
        .collect::<Vec<_>>()
        .join(",");
    let supported_video_codecs = message
        .supported_video_codecs
        .iter()
        .map(encode_video_codec)
        .collect::<Vec<_>>()
        .join(",");
    let preferred_video_codecs = message
        .preferred_video_codecs
        .iter()
        .map(encode_video_codec)
        .collect::<Vec<_>>()
        .join(",");

    let mut json = format!(
        "{{\"type\":\"host_hello\",\"session_id\":{},\"protocol_major\":{},\"protocol_minor\":{},\"host_version\":{},\"requested_features\":[{}],\"supported_video_codecs\":[{}],\"preferred_video_codecs\":[{}],\"video_limits\":{{\"max_width\":{},\"max_height\":{},\"max_fps\":{}",
        quote_string(&message.session_id),
        message.protocol_major,
        message.protocol_minor,
        quote_string(&message.host_version),
        requested_features,
        supported_video_codecs,
        preferred_video_codecs,
        message.video_limits.max_width,
        message.video_limits.max_height,
        message.video_limits.max_fps
    );
    if let Some(bitrate_kbps) = message.video_limits.bitrate_kbps {
        json.push_str(&format!(",\"bitrate_kbps\":{bitrate_kbps}"));
    }
    json.push_str("}}");
    json
}

fn encode_session_config(message: &SessionConfig) -> String {
    let mut json = format!(
        "{{\"type\":\"session_config\",\"session_id\":{},\"selected_video_codec\":{},\"video\":{{\"max_width\":{},\"max_height\":{},\"max_fps\":{}",
        quote_string(&message.session_id),
        encode_video_codec(&message.selected_video_codec),
        message.video.max_width,
        message.video.max_height,
        message.video.max_fps
    );
    if let Some(bitrate_kbps) = message.video.bitrate_kbps {
        json.push_str(&format!(",\"bitrate_kbps\":{bitrate_kbps}"));
    }
    if let Some(iframe_interval_ms) = message.video.iframe_interval_ms {
        json.push_str(&format!(",\"iframe_interval_ms\":{iframe_interval_ms}"));
    }
    json.push_str(&format!(
        "}},\"control\":{{\"enabled\":{}}}",
        if message.control.enabled {
            "true"
        } else {
            "false"
        }
    ));
    if let Some(rotation_locked) = message.rotation_locked {
        json.push_str(&format!(
            ",\"rotation\":{{\"locked\":{}}}",
            if rotation_locked { "true" } else { "false" }
        ));
    }
    json.push('}');
    json
}

fn encode_stop_session(message: &StopSession) -> String {
    let mut json = format!(
        "{{\"type\":\"stop_session\",\"session_id\":{}",
        quote_string(&message.session_id)
    );
    if let Some(reason) = &message.reason {
        json.push_str(&format!(",\"reason\":{}", quote_string(reason)));
    }
    json.push('}');
    json
}

fn encode_control_event(message: &ControlEvent) -> String {
    let mut json = format!(
        "{{\"type\":\"control_event\",\"session_id\":{},\"event_type\":{},\"sequence\":{}",
        quote_string(&message.session_id),
        quote_string(control_event_type_name(message)),
        message.sequence
    );
    if let Some(position_norm) = message.position_norm {
        json.push_str(&format!(
            ",\"position_norm\":{{\"x\":{},\"y\":{}}}",
            position_norm.x(),
            position_norm.y()
        ));
    }
    if let Some(pointer_id) = message.pointer_id {
        json.push_str(&format!(",\"pointer_id\":{pointer_id}"));
    }
    if let Some(button) = &message.button {
        json.push_str(&format!(
            ",\"button\":{}",
            quote_string(pointer_button_name(button))
        ));
    }
    if let Some(scroll_delta_x) = message.scroll_delta_x {
        json.push_str(&format!(",\"scroll_delta_x\":{scroll_delta_x}"));
    }
    if let Some(scroll_delta_y) = message.scroll_delta_y {
        json.push_str(&format!(",\"scroll_delta_y\":{scroll_delta_y}"));
    }
    if let Some(key_code) = &message.key_code {
        json.push_str(&format!(",\"key_code\":{}", quote_string(key_code)));
    }
    if let Some(text) = &message.text {
        json.push_str(&format!(",\"text\":{}", quote_string(text)));
    }
    if let Some(device_action) = &message.device_action {
        json.push_str(&format!(
            ",\"device_action\":{}",
            quote_string(device_action_name(device_action))
        ));
    }
    json.push('}');
    json
}

fn control_event_type_name(message: &ControlEvent) -> &'static str {
    use hscrcpy_contracts::ControlEventType;

    match message.event_type {
        ControlEventType::PointerDown => "pointer_down",
        ControlEventType::PointerMove => "pointer_move",
        ControlEventType::PointerUp => "pointer_up",
        ControlEventType::Scroll => "scroll",
        ControlEventType::KeyDown => "key_down",
        ControlEventType::KeyUp => "key_up",
        ControlEventType::DeviceAction => "device_action",
    }
}

fn pointer_button_name(button: &PointerButton) -> &'static str {
    match button {
        PointerButton::Primary => "primary",
        PointerButton::Secondary => "secondary",
        PointerButton::Middle => "middle",
    }
}

fn device_action_name(action: &DeviceAction) -> &str {
    match action {
        DeviceAction::Back => "back",
        DeviceAction::Home => "home",
        DeviceAction::Named(other) => other.as_str(),
    }
}

fn encode_session_feature(feature: &SessionFeature) -> String {
    let value = match feature {
        SessionFeature::Video => "video",
        SessionFeature::Control => "control",
    };
    quote_string(value)
}

fn encode_video_codec(codec: &VideoCodec) -> String {
    quote_string(video_codec_name(codec))
}

fn video_codec_name(codec: &VideoCodec) -> &'static str {
    match codec {
        VideoCodec::H264 => "h264",
        VideoCodec::Jpeg => "jpeg",
        VideoCodec::H265Experimental => "h265",
    }
}

fn quote_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped.push('"');
    escaped
}

fn decode_inbound_session_message(input: &str) -> HostResult<InboundSessionMessage> {
    let value = JsonParser::new(input).parse()?;
    let object = value.as_object()?;
    let message_type = object.required_string("type")?;

    match message_type {
        "device_hello" => Ok(InboundSessionMessage::DeviceHello(parse_device_hello(
            object,
        )?)),
        "authorization_update" => Ok(InboundSessionMessage::AuthorizationUpdate(
            parse_authorization_update(object)?,
        )),
        "session_ready" => Ok(InboundSessionMessage::SessionReady(parse_session_ready(
            object,
        )?)),
        "session_error" => Ok(InboundSessionMessage::SessionError(parse_session_error(
            object,
        )?)),
        other => Err(HostError::ContractViolation(format!(
            "unsupported inbound session message type `{other}`"
        ))),
    }
}

fn parse_device_hello(object: &JsonObject) -> HostResult<DeviceHello> {
    Ok(DeviceHello {
        session_id: object.required_string("session_id")?.to_string(),
        protocol_major: object.required_u16("protocol_major")?,
        protocol_minor: object.required_u16("protocol_minor")?,
        companion_version: object.required_string("companion_version")?.to_string(),
        device_name: object.required_string("device_name")?.to_string(),
        authorization: parse_authorization(object.required_object("authorization")?)?,
        available_features: parse_features(object.required_array("available_features")?)?,
        available_video_codecs: parse_video_codec_descriptors(
            object.required_array("available_video_codecs")?,
        )?,
        display: parse_display_info(object.required_object("display")?)?,
    })
}

fn parse_authorization_update(object: &JsonObject) -> HostResult<AuthorizationUpdate> {
    Ok(AuthorizationUpdate {
        session_id: object.required_string("session_id")?.to_string(),
        authorization: parse_authorization(object.required_object("authorization")?)?,
        reason: object.optional_string("reason")?.map(ToString::to_string),
    })
}

fn parse_session_ready(object: &JsonObject) -> HostResult<SessionReady> {
    Ok(SessionReady {
        session_id: object.required_string("session_id")?.to_string(),
        selected_video_codec: parse_video_codec(object.required_string("selected_video_codec")?)?,
        channel_layout: parse_channel_layout(object.required_object("channel_layout")?)?,
        display: parse_display_info(object.required_object("display")?)?,
    })
}

fn parse_session_error(object: &JsonObject) -> HostResult<SessionError> {
    Ok(SessionError {
        session_id: object.required_string("session_id")?.to_string(),
        code: object.required_string("code")?.to_string(),
        message: object.required_string("message")?.to_string(),
        retryable: object.required_bool("retryable")?,
    })
}

fn parse_authorization(object: &JsonObject) -> HostResult<FeatureAuthorization> {
    Ok(FeatureAuthorization {
        video_capture: parse_authorization_state(object.required_string("video_capture")?)?,
        input_injection: parse_authorization_state(object.required_string("input_injection")?)?,
    })
}

fn parse_authorization_state(value: &str) -> HostResult<AuthorizationState> {
    match value {
        "granted" => Ok(AuthorizationState::Granted),
        "needs_user_action" => Ok(AuthorizationState::NeedsUserAction),
        "denied" => Ok(AuthorizationState::Denied),
        "unsupported" => Ok(AuthorizationState::Unsupported),
        other => Err(HostError::ContractViolation(format!(
            "unsupported authorization state `{other}`"
        ))),
    }
}

fn parse_features(values: &[JsonValue]) -> HostResult<Vec<SessionFeature>> {
    values
        .iter()
        .map(|value| parse_session_feature(value.as_string()?))
        .collect()
}

fn parse_session_feature(value: &str) -> HostResult<SessionFeature> {
    match value {
        "video" => Ok(SessionFeature::Video),
        "control" => Ok(SessionFeature::Control),
        other => Err(HostError::ContractViolation(format!(
            "unsupported session feature `{other}`"
        ))),
    }
}

fn parse_video_codec_descriptors(values: &[JsonValue]) -> HostResult<Vec<VideoCodecDescriptor>> {
    values
        .iter()
        .map(|value| {
            let object = value.as_object()?;
            Ok(VideoCodecDescriptor {
                codec: parse_video_codec(object.required_string("codec")?)?,
                encoder_kind: object.required_string("encoder_kind")?.to_string(),
                max_width: object.required_u16("max_width")?,
                max_height: object.required_u16("max_height")?,
                max_fps: object.required_u16("max_fps")?,
                bitrate_control: object
                    .optional_string("bitrate_control")?
                    .map(ToString::to_string),
            })
        })
        .collect()
}

fn parse_display_info(object: &JsonObject) -> HostResult<DisplayInfo> {
    Ok(DisplayInfo {
        width: object.required_u16("width")?,
        height: object.required_u16("height")?,
        rotation: object.required_u16("rotation")?,
    })
}

fn parse_channel_layout(object: &JsonObject) -> HostResult<ChannelLayout> {
    Ok(ChannelLayout {
        session: parse_channel_binding(object.required_object("session")?)?,
        video: parse_channel_binding(object.required_object("video")?)?,
    })
}

fn parse_channel_binding(object: &JsonObject) -> HostResult<ChannelBinding> {
    Ok(ChannelBinding {
        name: object.required_string("name")?.to_string(),
        state: object.required_string("state")?.to_string(),
        payload_type: object.required_string("payload_type")?.to_string(),
        activation: object.required_string("activation")?.to_string(),
    })
}

fn parse_video_codec(value: &str) -> HostResult<VideoCodec> {
    match value {
        "h264" => Ok(VideoCodec::H264),
        "jpeg" => Ok(VideoCodec::Jpeg),
        "h265" => Ok(VideoCodec::H265Experimental),
        other => Err(HostError::ContractViolation(format!(
            "unsupported video codec `{other}`"
        ))),
    }
}

fn decode_video_codec(raw: u8) -> HostResult<VideoCodec> {
    match raw {
        1 => Ok(VideoCodec::H264),
        2 => Ok(VideoCodec::Jpeg),
        3 => Ok(VideoCodec::H265Experimental),
        other => Err(HostError::ContractViolation(format!(
            "unsupported video packet codec tag `{other}`"
        ))),
    }
}

#[cfg(test)]
pub(crate) fn encode_video_packet(
    codec: &VideoCodec,
    pts_us: u64,
    is_keyframe: bool,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(VIDEO_PACKET_HEADER_LEN + payload.len());
    bytes.push(match codec {
        VideoCodec::H264 => 1,
        VideoCodec::Jpeg => 2,
        VideoCodec::H265Experimental => 3,
    });
    bytes.extend_from_slice(&pts_us.to_be_bytes());
    bytes.push(u8::from(is_keyframe));
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

type JsonObject = BTreeMap<String, JsonValue>;

#[derive(Debug, Clone, PartialEq)]
enum JsonValue {
    String(String),
    Number(String),
    Bool(bool),
    Object(JsonObject),
    Array(Vec<JsonValue>),
    Null,
}

impl JsonValue {
    fn as_object(&self) -> HostResult<&JsonObject> {
        match self {
            Self::Object(object) => Ok(object),
            other => Err(HostError::ContractViolation(format!(
                "expected JSON object, got {other:?}"
            ))),
        }
    }

    fn as_string(&self) -> HostResult<&str> {
        match self {
            Self::String(value) => Ok(value),
            other => Err(HostError::ContractViolation(format!(
                "expected JSON string, got {other:?}"
            ))),
        }
    }
}

trait JsonObjectExt {
    fn required_value(&self, key: &str) -> HostResult<&JsonValue>;
    fn required_string(&self, key: &str) -> HostResult<&str>;
    fn optional_string(&self, key: &str) -> HostResult<Option<&str>>;
    fn required_u16(&self, key: &str) -> HostResult<u16>;
    fn required_bool(&self, key: &str) -> HostResult<bool>;
    fn required_object(&self, key: &str) -> HostResult<&JsonObject>;
    fn required_array(&self, key: &str) -> HostResult<&[JsonValue]>;
}

impl JsonObjectExt for JsonObject {
    fn required_value(&self, key: &str) -> HostResult<&JsonValue> {
        self.get(key).ok_or_else(|| {
            HostError::ContractViolation(format!("missing required JSON field `{key}`"))
        })
    }

    fn required_string(&self, key: &str) -> HostResult<&str> {
        self.required_value(key)?.as_string()
    }

    fn optional_string(&self, key: &str) -> HostResult<Option<&str>> {
        match self.get(key) {
            Some(JsonValue::String(value)) => Ok(Some(value)),
            Some(JsonValue::Null) => Ok(None),
            Some(other) => Err(HostError::ContractViolation(format!(
                "expected JSON string for `{key}`, got {other:?}"
            ))),
            None => Ok(None),
        }
    }

    fn required_u16(&self, key: &str) -> HostResult<u16> {
        match self.required_value(key)? {
            JsonValue::Number(value) => value.parse().map_err(|error| {
                HostError::ContractViolation(format!("failed to parse `{key}` as u16: {error}"))
            }),
            other => Err(HostError::ContractViolation(format!(
                "expected JSON number for `{key}`, got {other:?}"
            ))),
        }
    }

    fn required_bool(&self, key: &str) -> HostResult<bool> {
        match self.required_value(key)? {
            JsonValue::Bool(value) => Ok(*value),
            other => Err(HostError::ContractViolation(format!(
                "expected JSON bool for `{key}`, got {other:?}"
            ))),
        }
    }

    fn required_object(&self, key: &str) -> HostResult<&JsonObject> {
        self.required_value(key)?.as_object()
    }

    fn required_array(&self, key: &str) -> HostResult<&[JsonValue]> {
        match self.required_value(key)? {
            JsonValue::Array(values) => Ok(values),
            other => Err(HostError::ContractViolation(format!(
                "expected JSON array for `{key}`, got {other:?}"
            ))),
        }
    }
}

struct JsonParser<'a> {
    input: &'a [u8],
    index: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            index: 0,
        }
    }

    fn parse(mut self) -> HostResult<JsonValue> {
        self.skip_ws();
        let value = self.parse_value()?;
        self.skip_ws();
        if self.index != self.input.len() {
            return Err(HostError::ContractViolation(
                "unexpected trailing JSON content".to_string(),
            ));
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> HostResult<JsonValue> {
        self.skip_ws();
        match self.peek_byte() {
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b't') | Some(b'f') => self.parse_bool().map(JsonValue::Bool),
            Some(b'n') => {
                self.expect_bytes(b"null")?;
                Ok(JsonValue::Null)
            }
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number().map(JsonValue::Number),
            Some(other) => Err(HostError::ContractViolation(format!(
                "unexpected JSON token byte `{}`",
                other as char
            ))),
            None => Err(HostError::ContractViolation(
                "unexpected end of JSON input".to_string(),
            )),
        }
    }

    fn parse_object(&mut self) -> HostResult<JsonValue> {
        self.expect_byte(b'{')?;
        let mut object = BTreeMap::new();
        loop {
            self.skip_ws();
            if self.consume_byte_if(b'}') {
                break;
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect_byte(b':')?;
            let value = self.parse_value()?;
            object.insert(key, value);
            self.skip_ws();
            if self.consume_byte_if(b'}') {
                break;
            }
            self.expect_byte(b',')?;
        }
        Ok(JsonValue::Object(object))
    }

    fn parse_array(&mut self) -> HostResult<JsonValue> {
        self.expect_byte(b'[')?;
        let mut values = Vec::new();
        loop {
            self.skip_ws();
            if self.consume_byte_if(b']') {
                break;
            }
            values.push(self.parse_value()?);
            self.skip_ws();
            if self.consume_byte_if(b']') {
                break;
            }
            self.expect_byte(b',')?;
        }
        Ok(JsonValue::Array(values))
    }

    fn parse_string(&mut self) -> HostResult<String> {
        self.expect_byte(b'"')?;
        let mut value = String::new();
        loop {
            let byte = self.next_byte().ok_or_else(|| {
                HostError::ContractViolation("unterminated JSON string".to_string())
            })?;
            match byte {
                b'"' => break,
                b'\\' => {
                    let escaped = self.next_byte().ok_or_else(|| {
                        HostError::ContractViolation(
                            "unterminated JSON escape sequence".to_string(),
                        )
                    })?;
                    value.push(match escaped {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{0008}',
                        b'f' => '\u{000C}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        other => {
                            return Err(HostError::ContractViolation(format!(
                                "unsupported JSON escape byte `{}`",
                                other as char
                            )))
                        }
                    });
                }
                other => value.push(other as char),
            }
        }
        Ok(value)
    }

    fn parse_bool(&mut self) -> HostResult<bool> {
        if self.starts_with(b"true") {
            self.expect_bytes(b"true")?;
            Ok(true)
        } else if self.starts_with(b"false") {
            self.expect_bytes(b"false")?;
            Ok(false)
        } else {
            Err(HostError::ContractViolation(
                "expected JSON boolean literal".to_string(),
            ))
        }
    }

    fn parse_number(&mut self) -> HostResult<String> {
        let start = self.index;
        if self.consume_byte_if(b'-') {
            // Negative numbers are not expected in current inbound payloads, but the parser
            // accepts them for completeness.
        }
        self.consume_digits();
        if self.consume_byte_if(b'.') {
            self.consume_digits();
        }
        if matches!(self.peek_byte(), Some(b'e') | Some(b'E')) {
            self.index += 1;
            if matches!(self.peek_byte(), Some(b'+') | Some(b'-')) {
                self.index += 1;
            }
            self.consume_digits();
        }

        std::str::from_utf8(&self.input[start..self.index])
            .map(|value| value.to_string())
            .map_err(|error| {
                HostError::ContractViolation(format!("failed to decode JSON number: {error}"))
            })
    }

    fn consume_digits(&mut self) {
        while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
            self.index += 1;
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek_byte(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.index += 1;
        }
    }

    fn expect_byte(&mut self, expected: u8) -> HostResult<()> {
        let actual = self.next_byte().ok_or_else(|| {
            HostError::ContractViolation(format!(
                "expected JSON byte `{}` but reached end of input",
                expected as char
            ))
        })?;
        if actual == expected {
            Ok(())
        } else {
            Err(HostError::ContractViolation(format!(
                "expected JSON byte `{}`, got `{}`",
                expected as char, actual as char
            )))
        }
    }

    fn expect_bytes(&mut self, expected: &[u8]) -> HostResult<()> {
        for byte in expected {
            self.expect_byte(*byte)?;
        }
        Ok(())
    }

    fn consume_byte_if(&mut self, expected: u8) -> bool {
        if self.peek_byte() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn starts_with(&self, prefix: &[u8]) -> bool {
        self.input[self.index..].starts_with(prefix)
    }

    fn peek_byte(&self) -> Option<u8> {
        self.input.get(self.index).copied()
    }

    fn next_byte(&mut self) -> Option<u8> {
        let byte = self.peek_byte()?;
        self.index += 1;
        Some(byte)
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_inbound_session_message, encode_video_packet, endpoint_port};
    use crate::video::{PreparedVideoIngress, VideoTransportPacket};
    use hscrcpy_contracts::{
        ChannelEndpoint, InboundSessionMessage, SessionFeature, TransportKind, VideoCodec,
    };

    #[test]
    fn parses_device_hello_payload() {
        let payload = r#"{
            "type":"device_hello",
            "session_id":"session-1",
            "protocol_major":1,
            "protocol_minor":0,
            "companion_version":"1.0.0",
            "device_name":"Harmony Device",
            "authorization":{"video_capture":"granted","input_injection":"needs_user_action"},
            "available_features":["video","control"],
            "available_video_codecs":[
                {"codec":"h264","encoder_kind":"hardware","max_width":1920,"max_height":1080,"max_fps":60,"bitrate_control":"vbr"},
                {"codec":"jpeg","encoder_kind":"software","max_width":1920,"max_height":1080,"max_fps":30}
            ],
            "display":{"width":1920,"height":1080,"rotation":0}
        }"#;

        let message =
            decode_inbound_session_message(payload).expect("device hello should parse cleanly");
        match message {
            InboundSessionMessage::DeviceHello(device_hello) => {
                assert_eq!(device_hello.session_id, "session-1");
                assert_eq!(device_hello.available_features.len(), 2);
                assert!(device_hello
                    .available_features
                    .contains(&SessionFeature::Control));
                assert_eq!(device_hello.available_video_codecs.len(), 2);
                assert_eq!(
                    device_hello.available_video_codecs[0].codec,
                    VideoCodec::H264
                );
            }
            unexpected => panic!("unexpected message: {unexpected:?}"),
        }
    }

    #[test]
    fn parses_session_ready_payload() {
        let payload = r#"{
            "type":"session_ready",
            "session_id":"session-1",
            "selected_video_codec":"jpeg",
            "channel_layout":{
                "session":{"name":"session","state":"ready","payload_type":"utf8_json","activation":"open first"},
                "video":{"name":"video","state":"pending_open","payload_type":"binary","activation":"open after ready"}
            },
            "display":{"width":1280,"height":720,"rotation":90}
        }"#;

        let message =
            decode_inbound_session_message(payload).expect("session ready should parse cleanly");
        match message {
            InboundSessionMessage::SessionReady(session_ready) => {
                assert_eq!(session_ready.selected_video_codec, VideoCodec::Jpeg);
                assert_eq!(session_ready.channel_layout.video.payload_type, "binary");
                assert_eq!(session_ready.display.rotation, 90);
            }
            unexpected => panic!("unexpected message: {unexpected:?}"),
        }
    }

    #[test]
    fn decodes_provisional_video_packet_into_h264_ingress() {
        let payload = encode_video_packet(
            &VideoCodec::H264,
            444,
            true,
            &[0, 0, 1, 0x67, 0x42, 0x00, 0x1f, 0, 0, 1, 0x65, 0x88, 0x84],
        );
        let packet =
            VideoTransportPacket::decode(&payload).expect("provisional packet should decode");
        let ingress =
            PreparedVideoIngress::from_packet(packet).expect("decoded packet should prepare");

        match ingress {
            PreparedVideoIngress::H264(access_unit) => {
                assert_eq!(access_unit.timestamp_micros, 444);
                assert!(access_unit.is_keyframe);
                assert_eq!(
                    access_unit.encoded_bytes,
                    vec![0, 0, 1, 0x67, 0x42, 0x00, 0x1f, 0, 0, 1, 0x65, 0x88, 0x84]
                );
            }
            unexpected => panic!("unexpected ingress: {unexpected:?}"),
        }
    }

    #[test]
    fn parses_endpoint_port_from_socket_target() {
        let endpoint = ChannelEndpoint {
            transport: TransportKind::HdcForward,
            target: "127.0.0.1:27182".to_string(),
        };

        assert_eq!(endpoint_port(&endpoint).expect("port should parse"), 27182);
    }
}
