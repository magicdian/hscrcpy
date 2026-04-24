use super::runtime::{
    endpoint_port, SessionRuntime, SessionTransportFactory, TcpSessionTransportFactory,
};
use crate::companion::{CompanionLaunchRequest, CompanionManager, CompanionPlan};
use crate::hdc::HdcBridge;
use crate::video::{classify_negotiated_video_path, NegotiatedVideoPath};
use crate::{HostError, HostResult};
use hscrcpy_contracts::{
    AuthorizationUpdate, ChannelEndpoint, DeviceHello, FeatureAuthorization, HostHello,
    HostVideoLimits, InboundSessionMessage, SessionConfig, SessionControlConfig,
    SessionControlState, SessionFeature, SessionMessage, SessionReady, SessionStartRequest,
    SessionStartResponse, SessionVideoConfig, TransportKind, VideoCodec, PROTOCOL_MAJOR_MVP,
    PROTOCOL_MINOR_MVP,
};

const SESSION_CHANNEL_TARGET: &str = "127.0.0.1:27182";
const VIDEO_CHANNEL_TARGET: &str = "127.0.0.1:27183";
const HOST_MAX_WIDTH: u16 = 1920;
const HOST_MAX_HEIGHT: u16 = 1080;
const H264_IFRAME_INTERVAL_MS: u32 = 2_000;

#[derive(Debug, Clone)]
pub struct SessionBootstrap {
    pub device_id: String,
    pub request: SessionStartRequest,
}

impl SessionBootstrap {
    pub fn new(device_id: impl Into<String>, request: SessionStartRequest) -> Self {
        Self {
            device_id: device_id.into(),
            request,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionNegotiation {
    pub device_hello: DeviceHello,
    pub authorization_updates: Vec<AuthorizationUpdate>,
    pub session_ready: SessionReady,
}

pub struct SessionStartupPlan {
    pub response: SessionStartResponse,
    pub negotiation: SessionNegotiation,
    pub video_path: NegotiatedVideoPath,
    pub outbound_session_messages: Vec<SessionMessage>,
    pub runtime: SessionRuntime,
}

pub struct SessionOrchestrator<H, C> {
    hdc: H,
    companion: C,
    transport: Box<dyn SessionTransportFactory>,
}

impl<H, C> SessionOrchestrator<H, C>
where
    H: HdcBridge,
    C: CompanionManager,
{
    pub fn new(hdc: H, companion: C) -> Self {
        Self::with_transport(hdc, companion, TcpSessionTransportFactory)
    }

    pub fn with_transport<T>(hdc: H, companion: C, transport: T) -> Self
    where
        T: SessionTransportFactory + 'static,
    {
        Self {
            hdc,
            companion,
            transport: Box::new(transport),
        }
    }

    pub fn prepare(&self, bootstrap: &SessionBootstrap) -> HostResult<CompanionPlan> {
        self.hdc.ensure_device_visible(&bootstrap.device_id)?;
        let plan = self.companion.plan(&bootstrap.device_id)?;
        self.companion.apply(&bootstrap.device_id, &plan)?;
        Ok(plan)
    }

    pub fn start(&self, bootstrap: &SessionBootstrap) -> HostResult<SessionStartupPlan> {
        self.prepare(bootstrap)?;

        let preferred_codec_order =
            normalize_requested_codec_order(&bootstrap.request.requested_codec_order)?;
        let session_id = build_session_id(&bootstrap.device_id);
        let session_channel = session_channel_endpoint();
        let video_channel = video_channel_endpoint();
        self.companion.launch_session(
            &bootstrap.device_id,
            &CompanionLaunchRequest {
                session_id: session_id.clone(),
                session_channel: session_channel.clone(),
                video_channel: video_channel.clone(),
            },
        )?;

        forward_hdc_endpoint(&self.hdc, &bootstrap.device_id, &session_channel)?;
        let mut session_runtime = self.transport.open_session_channel(&session_channel)?;

        let host_hello = build_host_hello(&session_id, &bootstrap.request, &preferred_codec_order);
        session_runtime.send_message(&SessionMessage::HostHello(host_hello.clone()))?;
        let first_inbound = session_runtime.receive_message()?;

        let device_hello = match first_inbound {
            InboundSessionMessage::DeviceHello(message) => message,
            InboundSessionMessage::SessionError(error) => return Err(map_session_error(&error)),
            unexpected => {
                return Err(HostError::ContractViolation(format!(
                    "expected device_hello after host_hello, got {}",
                    unexpected.message_type()
                )))
            }
        };
        validate_device_hello(&device_hello, &session_id)?;
        let control_enabled = resolve_control_mode(
            bootstrap.request.enable_control,
            &device_hello.available_features,
        )?;

        let (authorization, authorization_updates) = wait_for_authorization(
            session_runtime.as_mut(),
            &session_id,
            control_enabled,
            device_hello.authorization.clone(),
        )?;

        let selected_codec =
            select_codec(&preferred_codec_order, &device_hello.available_video_codecs)?;
        let session_config = build_session_config(
            &session_id,
            selected_codec.clone(),
            &bootstrap.request,
            control_enabled,
        );
        session_runtime.send_message(&SessionMessage::SessionConfig(session_config.clone()))?;

        let session_ready = match session_runtime.receive_message()? {
            InboundSessionMessage::SessionReady(message) => message,
            InboundSessionMessage::SessionError(error) => return Err(map_session_error(&error)),
            unexpected => {
                return Err(HostError::ContractViolation(format!(
                    "expected session_ready after session_config, got {}",
                    unexpected.message_type()
                )))
            }
        };
        validate_session_ready(&session_ready, &session_id, &selected_codec)?;

        forward_hdc_endpoint(&self.hdc, &bootstrap.device_id, &video_channel)?;
        let video_runtime = self
            .transport
            .open_video_channel(&video_channel, selected_codec.clone())?;

        let response = SessionStartResponse {
            session_id: session_id.clone(),
            selected_codec: selected_codec.clone(),
            session_channel: session_channel.clone(),
            video_channel: video_channel.clone(),
            control: SessionControlState {
                enabled: control_enabled,
            },
            authorization,
        };
        let outbound_session_messages = vec![
            SessionMessage::HostHello(host_hello),
            SessionMessage::SessionConfig(session_config),
        ];
        let runtime = SessionRuntime::new(
            session_id.clone(),
            selected_codec.clone(),
            session_runtime,
            video_runtime,
        );

        Ok(SessionStartupPlan {
            response,
            negotiation: SessionNegotiation {
                device_hello,
                authorization_updates,
                session_ready,
            },
            video_path: classify_negotiated_video_path(&selected_codec),
            outbound_session_messages,
            runtime,
        })
    }
}

fn build_session_id(device_id: &str) -> String {
    let sanitized: String = device_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect();
    if sanitized.is_empty() {
        "host-session-unknown-device".to_string()
    } else {
        format!("host-session-{sanitized}")
    }
}

fn session_channel_endpoint() -> ChannelEndpoint {
    ChannelEndpoint {
        transport: TransportKind::HdcForward,
        target: SESSION_CHANNEL_TARGET.to_string(),
    }
}

fn video_channel_endpoint() -> ChannelEndpoint {
    ChannelEndpoint {
        transport: TransportKind::HdcForward,
        target: VIDEO_CHANNEL_TARGET.to_string(),
    }
}

fn forward_hdc_endpoint<H: HdcBridge>(
    hdc: &H,
    device_id: &str,
    endpoint: &ChannelEndpoint,
) -> HostResult<()> {
    if endpoint.transport == TransportKind::HdcForward {
        let port = endpoint_port(endpoint)?;
        hdc.forward_port(device_id, port, port)?;
    }
    Ok(())
}

fn build_host_hello(
    session_id: &str,
    request: &SessionStartRequest,
    preferred_codec_order: &[VideoCodec],
) -> HostHello {
    let mut requested_features = vec![SessionFeature::Video];
    if request.enable_control {
        requested_features.push(SessionFeature::Control);
    }

    HostHello {
        session_id: session_id.to_string(),
        protocol_major: PROTOCOL_MAJOR_MVP,
        protocol_minor: PROTOCOL_MINOR_MVP,
        host_version: env!("CARGO_PKG_VERSION").to_string(),
        requested_features,
        supported_video_codecs: host_supported_video_codecs(),
        preferred_video_codecs: preferred_codec_order.to_vec(),
        video_limits: HostVideoLimits {
            max_width: HOST_MAX_WIDTH,
            max_height: HOST_MAX_HEIGHT,
            max_fps: request.preferred_max_fps,
            bitrate_kbps: None,
        },
    }
}

fn build_session_config(
    session_id: &str,
    selected_codec: VideoCodec,
    request: &SessionStartRequest,
    control_enabled: bool,
) -> SessionConfig {
    SessionConfig {
        session_id: session_id.to_string(),
        selected_video_codec: selected_codec.clone(),
        video: SessionVideoConfig {
            max_width: HOST_MAX_WIDTH,
            max_height: HOST_MAX_HEIGHT,
            max_fps: request.preferred_max_fps,
            bitrate_kbps: None,
            iframe_interval_ms: match selected_codec {
                VideoCodec::H264 => Some(H264_IFRAME_INTERVAL_MS),
                _ => None,
            },
        },
        control: SessionControlConfig {
            enabled: control_enabled,
        },
        rotation_locked: None,
    }
}

fn validate_device_hello(device_hello: &DeviceHello, expected_session_id: &str) -> HostResult<()> {
    if device_hello.session_id != expected_session_id {
        return Err(HostError::ContractViolation(format!(
            "device_hello session_id `{}` does not match host session `{expected_session_id}`",
            device_hello.session_id
        )));
    }
    if device_hello.protocol_major != PROTOCOL_MAJOR_MVP {
        return Err(HostError::ContractViolation(format!(
            "protocol_major_mismatch: device advertised {}, host requires {}",
            device_hello.protocol_major, PROTOCOL_MAJOR_MVP
        )));
    }
    if device_hello.protocol_minor < PROTOCOL_MINOR_MVP {
        return Err(HostError::ContractViolation(format!(
            "device protocol_minor {} is older than host floor {}",
            device_hello.protocol_minor, PROTOCOL_MINOR_MVP
        )));
    }
    Ok(())
}

fn validate_session_ready(
    session_ready: &SessionReady,
    expected_session_id: &str,
    expected_codec: &VideoCodec,
) -> HostResult<()> {
    if session_ready.session_id != expected_session_id {
        return Err(HostError::ContractViolation(format!(
            "session_ready session_id `{}` does not match host session `{expected_session_id}`",
            session_ready.session_id
        )));
    }
    if &session_ready.selected_video_codec != expected_codec {
        return Err(HostError::ContractViolation(format!(
            "session_ready selected codec {:?} does not match host-configured {:?}",
            session_ready.selected_video_codec, expected_codec
        )));
    }
    Ok(())
}

fn wait_for_authorization(
    session_runtime: &mut dyn super::runtime::SessionChannelTransport,
    session_id: &str,
    control_enabled: bool,
    initial_authorization: FeatureAuthorization,
) -> HostResult<(FeatureAuthorization, Vec<AuthorizationUpdate>)> {
    let mut authorization = initial_authorization;
    let mut updates = Vec::new();

    while !authorization.required_features_granted(control_enabled) {
        let message = session_runtime.receive_message()?;
        match message {
            InboundSessionMessage::AuthorizationUpdate(update) => {
                if update.session_id != session_id {
                    return Err(HostError::ContractViolation(format!(
                        "authorization_update session_id `{}` does not match active session `{session_id}`",
                        update.session_id
                    )));
                }
                authorization = update.authorization.clone();
                updates.push(update);
            }
            InboundSessionMessage::SessionError(error) => return Err(map_session_error(&error)),
            unexpected => {
                return Err(HostError::ContractViolation(format!(
                    "expected authorization_update while waiting for grants, got {}",
                    unexpected.message_type()
                )))
            }
        }
    }

    Ok((authorization, updates))
}

fn resolve_control_mode(
    requested: bool,
    available_features: &[SessionFeature],
) -> HostResult<bool> {
    if requested && !available_features.contains(&SessionFeature::Control) {
        return Err(HostError::ContractViolation(
            "feature_unsupported: control requested but device does not advertise control support"
                .to_string(),
        ));
    }
    Ok(requested)
}

fn host_supported_video_codecs() -> Vec<VideoCodec> {
    vec![VideoCodec::H264, VideoCodec::Jpeg]
}

fn normalize_requested_codec_order(requested_order: &[VideoCodec]) -> HostResult<Vec<VideoCodec>> {
    let request_is_empty = requested_order.is_empty();
    let h264_requested = request_is_empty || requested_order.contains(&VideoCodec::H264);
    let jpeg_requested = request_is_empty || requested_order.contains(&VideoCodec::Jpeg);
    let mut normalized = Vec::new();

    if h264_requested {
        normalized.push(VideoCodec::H264);
    }
    if jpeg_requested || h264_requested {
        normalized.push(VideoCodec::Jpeg);
    }

    if normalized.is_empty() {
        return Err(HostError::ContractViolation(
            "no host-supported codec in request; expected h264 and/or jpeg".to_string(),
        ));
    }

    Ok(normalized)
}

fn select_codec(
    requested_order: &[VideoCodec],
    available_codecs: &[hscrcpy_contracts::VideoCodecDescriptor],
) -> HostResult<VideoCodec> {
    for codec in requested_order {
        if available_codecs
            .iter()
            .any(|descriptor| &descriptor.codec == codec)
        {
            return Ok(codec.clone());
        }
    }
    Err(HostError::ContractViolation(
        "no_shared_video_codec: no compatible codec between host preference and device_hello"
            .to_string(),
    ))
}

fn map_session_error(error: &hscrcpy_contracts::SessionError) -> HostError {
    HostError::ContractViolation(format!("{}: {}", error.code, error.message))
}

#[cfg(test)]
mod tests {
    use super::{
        build_session_id, normalize_requested_codec_order, resolve_control_mode, select_codec,
        SessionBootstrap, SessionOrchestrator,
    };
    use crate::companion::{
        CompanionAction, CompanionLaunchRequest, CompanionManager, CompanionPlan,
    };
    use crate::control::{ControlMessageEmitter, OutboundControlEvent, SessionMessageSink};
    use crate::hdc::HdcBridge;
    use crate::session::{SessionChannelTransport, SessionTransportFactory, VideoChannelTransport};
    use crate::video::{PreparedVideoIngress, VideoTransportPacket};
    use crate::{HostError, HostResult};
    use hscrcpy_contracts::{
        AuthorizationState, AuthorizationUpdate, ChannelEndpoint, ChannelLayout, DeviceHello,
        DisplayInfo, FeatureAuthorization, InboundSessionMessage, SessionFeature, SessionMessage,
        SessionReady, SessionStartRequest, VideoCodec, VideoCodecDescriptor,
    };
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    struct TestHdcBridge {
        forwarded_ports: Rc<RefCell<Vec<(String, u16, u16)>>>,
    }

    impl HdcBridge for TestHdcBridge {
        fn ensure_device_visible(&self, _device_id: &str) -> HostResult<()> {
            Ok(())
        }

        fn forward_port(
            &self,
            device_id: &str,
            local_port: u16,
            remote_port: u16,
        ) -> HostResult<()> {
            self.forwarded_ports.borrow_mut().push((
                device_id.to_string(),
                local_port,
                remote_port,
            ));
            Ok(())
        }

        fn exec_shell(&self, _device_id: &str, _command: &str) -> HostResult<String> {
            Ok(String::new())
        }
    }

    #[derive(Default)]
    struct TestCompanionManager {
        launched_sessions: Rc<RefCell<Vec<String>>>,
    }

    impl CompanionManager for TestCompanionManager {
        fn plan(&self, _device_id: &str) -> HostResult<CompanionPlan> {
            Ok(CompanionPlan {
                action: CompanionAction::Skip,
                target_version: "1.0.0".to_string(),
                artifact_hint: "assets/companion/hscrcpy_server.hap".to_string(),
            })
        }

        fn apply(&self, _device_id: &str, _plan: &CompanionPlan) -> HostResult<()> {
            Ok(())
        }

        fn launch_session(
            &self,
            _device_id: &str,
            request: &CompanionLaunchRequest,
        ) -> HostResult<()> {
            self.launched_sessions
                .borrow_mut()
                .push(request.session_id.clone());
            Ok(())
        }
    }

    struct TestSessionChannel {
        inbound: VecDeque<HostResult<InboundSessionMessage>>,
        sent: Rc<RefCell<Vec<SessionMessage>>>,
    }

    impl SessionChannelTransport for TestSessionChannel {
        fn send_message(&mut self, message: &SessionMessage) -> HostResult<()> {
            self.sent.borrow_mut().push(message.clone());
            Ok(())
        }

        fn receive_message(&mut self) -> HostResult<InboundSessionMessage> {
            self.inbound
                .pop_front()
                .expect("unexpected session receive in test")
        }
    }

    struct TestVideoChannel {
        packets: VecDeque<HostResult<VideoTransportPacket>>,
    }

    impl VideoChannelTransport for TestVideoChannel {
        fn receive_packet(&mut self) -> HostResult<VideoTransportPacket> {
            self.packets
                .pop_front()
                .expect("unexpected video receive in test")
        }
    }

    struct TestTransportFactory {
        session_messages: Rc<RefCell<Option<VecDeque<HostResult<InboundSessionMessage>>>>>,
        video_packets: Rc<RefCell<Option<VecDeque<HostResult<VideoTransportPacket>>>>>,
        sent_messages: Rc<RefCell<Vec<SessionMessage>>>,
        opened_video_codecs: Rc<RefCell<Vec<VideoCodec>>>,
    }

    impl SessionTransportFactory for TestTransportFactory {
        fn open_session_channel(
            &self,
            _endpoint: &ChannelEndpoint,
        ) -> HostResult<Box<dyn SessionChannelTransport>> {
            Ok(Box::new(TestSessionChannel {
                inbound: self
                    .session_messages
                    .borrow_mut()
                    .take()
                    .expect("session channel opened once"),
                sent: self.sent_messages.clone(),
            }))
        }

        fn open_video_channel(
            &self,
            _endpoint: &ChannelEndpoint,
            selected_codec: VideoCodec,
        ) -> HostResult<Box<dyn VideoChannelTransport>> {
            self.opened_video_codecs.borrow_mut().push(selected_codec);
            Ok(Box::new(TestVideoChannel {
                packets: self
                    .video_packets
                    .borrow_mut()
                    .take()
                    .expect("video channel opened once"),
            }))
        }
    }

    #[test]
    fn normalizes_h264_mainline_with_explicit_jpeg_fallback() {
        let normalized = normalize_requested_codec_order(&[VideoCodec::H264])
            .expect("h264 request should include explicit jpeg fallback");
        assert_eq!(normalized, vec![VideoCodec::H264, VideoCodec::Jpeg]);
    }

    #[test]
    fn keeps_jpeg_only_mode_when_h264_is_not_requested() {
        let normalized = normalize_requested_codec_order(&[VideoCodec::Jpeg])
            .expect("jpeg-only mode should remain valid");
        assert_eq!(normalized, vec![VideoCodec::Jpeg]);
    }

    #[test]
    fn rejects_requests_without_host_supported_codecs() {
        let err = normalize_requested_codec_order(&[VideoCodec::H265Experimental])
            .expect_err("h265-only request is not supported in MVP host");
        assert!(err.to_string().contains("no host-supported codec"));
    }

    #[test]
    fn rejects_control_request_when_device_cannot_inject_input() {
        let err = resolve_control_mode(true, &[SessionFeature::Video])
            .expect_err("control support mismatch must fail");
        assert!(err.to_string().contains("feature_unsupported"));
    }

    #[test]
    fn selects_jpeg_when_h264_is_missing() {
        let selected = select_codec(
            &[VideoCodec::H264, VideoCodec::Jpeg],
            &[VideoCodecDescriptor {
                codec: VideoCodec::Jpeg,
                encoder_kind: "software".to_string(),
                max_width: 1920,
                max_height: 1080,
                max_fps: 30,
                bitrate_control: None,
            }],
        )
        .expect("jpeg fallback should be selected");
        assert_eq!(selected, VideoCodec::Jpeg);
    }

    #[test]
    fn startup_progresses_handshake_and_opens_video_after_ready() {
        let forwarded_ports = Rc::new(RefCell::new(Vec::new()));
        let companion = TestCompanionManager::default();
        let sent_messages = Rc::new(RefCell::new(Vec::new()));
        let opened_video_codecs = Rc::new(RefCell::new(Vec::new()));
        let orchestrator = SessionOrchestrator::with_transport(
            TestHdcBridge {
                forwarded_ports: forwarded_ports.clone(),
            },
            companion,
            TestTransportFactory {
                session_messages: Rc::new(RefCell::new(Some(VecDeque::from(vec![
                    Ok(InboundSessionMessage::DeviceHello(DeviceHello {
                        session_id: build_session_id("usb-device-001"),
                        protocol_major: 1,
                        protocol_minor: 0,
                        companion_version: "1.0.0".to_string(),
                        device_name: "Harmony Device".to_string(),
                        authorization: FeatureAuthorization {
                            video_capture: AuthorizationState::Granted,
                            input_injection: AuthorizationState::NeedsUserAction,
                        },
                        available_features: vec![SessionFeature::Video, SessionFeature::Control],
                        available_video_codecs: vec![VideoCodecDescriptor {
                            codec: VideoCodec::Jpeg,
                            encoder_kind: "software".to_string(),
                            max_width: 1920,
                            max_height: 1080,
                            max_fps: 30,
                            bitrate_control: None,
                        }],
                        display: DisplayInfo {
                            width: 1920,
                            height: 1080,
                            rotation: 0,
                        },
                    })),
                    Ok(InboundSessionMessage::AuthorizationUpdate(
                        AuthorizationUpdate {
                            session_id: build_session_id("usb-device-001"),
                            authorization: FeatureAuthorization {
                                video_capture: AuthorizationState::Granted,
                                input_injection: AuthorizationState::Granted,
                            },
                            reason: Some("user accepted input injection".to_string()),
                        },
                    )),
                    Ok(InboundSessionMessage::SessionReady(SessionReady {
                        session_id: build_session_id("usb-device-001"),
                        selected_video_codec: VideoCodec::Jpeg,
                        channel_layout: ChannelLayout {
                            session: hscrcpy_contracts::ChannelBinding {
                                name: "session".to_string(),
                                state: "ready".to_string(),
                                payload_type: "utf8_json".to_string(),
                                activation: "open first".to_string(),
                            },
                            video: hscrcpy_contracts::ChannelBinding {
                                name: "video".to_string(),
                                state: "pending_open".to_string(),
                                payload_type: "binary".to_string(),
                                activation: "open after ready".to_string(),
                            },
                        },
                        display: DisplayInfo {
                            width: 1280,
                            height: 720,
                            rotation: 0,
                        },
                    })),
                ])))),
                video_packets: Rc::new(RefCell::new(Some(VecDeque::from(vec![Ok(
                    VideoTransportPacket::new(VideoCodec::Jpeg, 321, true, vec![1, 2, 3]),
                )])))),
                sent_messages: sent_messages.clone(),
                opened_video_codecs: opened_video_codecs.clone(),
            },
        );
        let bootstrap = SessionBootstrap::new(
            "usb-device-001",
            SessionStartRequest::h264_mainline(60, true),
        );

        let mut plan = orchestrator
            .start(&bootstrap)
            .expect("startup plan should succeed");

        assert_eq!(plan.response.selected_codec, VideoCodec::Jpeg);
        assert_eq!(plan.negotiation.authorization_updates.len(), 1);
        assert_eq!(
            plan.video_path,
            crate::video::NegotiatedVideoPath::JpegFallback
        );
        assert_eq!(*opened_video_codecs.borrow(), vec![VideoCodec::Jpeg]);
        assert_eq!(
            *forwarded_ports.borrow(),
            vec![
                ("usb-device-001".to_string(), 27182, 27182),
                ("usb-device-001".to_string(), 27183, 27183),
            ]
        );

        let sent = sent_messages.borrow();
        assert_eq!(sent.len(), 2);
        match &sent[0] {
            SessionMessage::HostHello(message) => {
                assert!(message
                    .requested_features
                    .contains(&SessionFeature::Control));
                assert_eq!(
                    message.preferred_video_codecs,
                    vec![VideoCodec::H264, VideoCodec::Jpeg]
                );
            }
            unexpected => panic!("first message must be host_hello, got {unexpected:?}"),
        }
        match &sent[1] {
            SessionMessage::SessionConfig(message) => {
                assert_eq!(message.selected_video_codec, VideoCodec::Jpeg);
                assert!(message.control.enabled);
            }
            unexpected => panic!("second message must be session_config, got {unexpected:?}"),
        }

        let ingress = plan
            .runtime
            .receive_video_ingress()
            .expect("prepared video ingress should be available");
        match ingress {
            PreparedVideoIngress::Jpeg(frame) => {
                assert_eq!(frame.timestamp_micros, 321);
                assert_eq!(frame.encoded_bytes, vec![1, 2, 3]);
            }
            unexpected => panic!("unexpected ingress: {unexpected:?}"),
        }
    }

    #[test]
    fn startup_rejects_non_annex_b_h264_payloads_on_video_ingress() {
        let orchestrator = SessionOrchestrator::with_transport(
            TestHdcBridge {
                forwarded_ports: Rc::new(RefCell::new(Vec::new())),
            },
            TestCompanionManager::default(),
            TestTransportFactory {
                session_messages: Rc::new(RefCell::new(Some(VecDeque::from(vec![
                    Ok(InboundSessionMessage::DeviceHello(DeviceHello {
                        session_id: build_session_id("usb-device-001"),
                        protocol_major: 1,
                        protocol_minor: 0,
                        companion_version: "1.0.0".to_string(),
                        device_name: "Harmony Device".to_string(),
                        authorization: FeatureAuthorization {
                            video_capture: AuthorizationState::Granted,
                            input_injection: AuthorizationState::Granted,
                        },
                        available_features: vec![SessionFeature::Video],
                        available_video_codecs: vec![VideoCodecDescriptor {
                            codec: VideoCodec::H264,
                            encoder_kind: "hardware".to_string(),
                            max_width: 1920,
                            max_height: 1080,
                            max_fps: 60,
                            bitrate_control: Some("vbr".to_string()),
                        }],
                        display: DisplayInfo {
                            width: 1920,
                            height: 1080,
                            rotation: 0,
                        },
                    })),
                    Ok(InboundSessionMessage::SessionReady(SessionReady {
                        session_id: build_session_id("usb-device-001"),
                        selected_video_codec: VideoCodec::H264,
                        channel_layout: ChannelLayout {
                            session: hscrcpy_contracts::ChannelBinding {
                                name: "session".to_string(),
                                state: "ready".to_string(),
                                payload_type: "utf8_json".to_string(),
                                activation: "open first".to_string(),
                            },
                            video: hscrcpy_contracts::ChannelBinding {
                                name: "video".to_string(),
                                state: "pending_open".to_string(),
                                payload_type: "binary".to_string(),
                                activation: "open after ready".to_string(),
                            },
                        },
                        display: DisplayInfo {
                            width: 1280,
                            height: 720,
                            rotation: 0,
                        },
                    })),
                ])))),
                video_packets: Rc::new(RefCell::new(Some(VecDeque::from(vec![Ok(
                    VideoTransportPacket::new(VideoCodec::H264, 777, true, vec![9, 8, 7]),
                )])))),
                sent_messages: Rc::new(RefCell::new(Vec::new())),
                opened_video_codecs: Rc::new(RefCell::new(Vec::new())),
            },
        );

        let mut plan = orchestrator
            .start(&SessionBootstrap::new(
                "usb-device-001",
                SessionStartRequest::h264_mainline(60, false),
            ))
            .expect("startup should succeed before ingest validation");
        assert_eq!(plan.response.selected_codec, VideoCodec::H264);

        let err = plan
            .runtime
            .receive_video_ingress()
            .expect_err("malformed non-Annex-B h264 packet should fail ingest");
        assert!(matches!(err, HostError::ContractViolation(_)));
        assert!(
            err.to_string().contains("Annex-B"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn runtime_remains_compatible_with_control_emitter() {
        struct Sink {
            sent: Vec<SessionMessage>,
        }

        impl SessionMessageSink for Sink {
            fn emit(&mut self, message: SessionMessage) -> HostResult<()> {
                self.sent.push(message);
                Ok(())
            }
        }

        let mut sink = Sink { sent: Vec::new() };
        let mut emitter = ControlMessageEmitter::new("session-1");
        emitter
            .emit(
                &mut sink,
                OutboundControlEvent::KeyDown {
                    key_code: "KEYCODE_BACK".to_string(),
                },
            )
            .expect("control event should emit");

        assert_eq!(sink.sent.len(), 1);
        match &sink.sent[0] {
            SessionMessage::ControlEvent(message) => assert_eq!(message.sequence, 1),
            unexpected => panic!("unexpected control message: {unexpected:?}"),
        }
    }

    #[test]
    fn startup_surfaces_session_error_codes() {
        let orchestrator = SessionOrchestrator::with_transport(
            TestHdcBridge {
                forwarded_ports: Rc::new(RefCell::new(Vec::new())),
            },
            TestCompanionManager::default(),
            TestTransportFactory {
                session_messages: Rc::new(RefCell::new(Some(VecDeque::from(vec![Ok(
                    InboundSessionMessage::SessionError(hscrcpy_contracts::SessionError {
                        session_id: "session-1".to_string(),
                        code: "protocol_major_mismatch".to_string(),
                        message: "protocol mismatch".to_string(),
                        retryable: false,
                    }),
                )])))),
                video_packets: Rc::new(RefCell::new(Some(VecDeque::new()))),
                sent_messages: Rc::new(RefCell::new(Vec::new())),
                opened_video_codecs: Rc::new(RefCell::new(Vec::new())),
            },
        );

        let err = match orchestrator.start(&SessionBootstrap::new(
            "usb-device-001",
            SessionStartRequest::h264_mainline(60, false),
        )) {
            Ok(_) => panic!("session_error should fail startup"),
            Err(err) => err,
        };
        assert!(matches!(err, HostError::ContractViolation(_)));
        assert!(err.to_string().contains("protocol_major_mismatch"));
    }
}
