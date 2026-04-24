use super::runtime::{
    endpoint_port, SessionRuntime, SessionTransportFactory, TcpSessionTransportFactory,
};
use crate::companion::{
    CompanionLaunchRequest, CompanionManager, CompanionPlan, RouteCompanionManager,
};
use crate::hdc::{HdcBridge, RuntimeHdcBridge};
use crate::official_scrcpy::OfficialScrcpyClient;
use crate::route::HostRoute;
use crate::uitest::{
    cleanup_uitest_runtime, collect_uitest_runtime_diagnostics, UitestLaunchConfig,
    UITEST_GRPC_SOCKET_NAME,
};
use crate::video::{
    classify_negotiated_video_path, normalized_requested_codec_order, select_codec_for_startup,
    CodecSelectionDiagnostics, HostCodecCapability, NegotiatedVideoPath,
    OfficialScrcpyIngressAdapter, PreparedVideoIngress,
};
use crate::{host_log, HostError, HostResult};
use hscrcpy_contracts::{
    AuthorizationState, AuthorizationUpdate, ChannelBinding, ChannelEndpoint, ChannelLayout,
    DeviceHello, DisplayInfo, FeatureAuthorization, HostHello, HostVideoLimits,
    InboundSessionMessage, SessionConfig, SessionControlConfig, SessionControlState,
    SessionFeature, SessionMessage, SessionReady, SessionStartRequest, SessionStartResponse,
    SessionVideoConfig, TransportKind, VideoCodec, VideoCodecDescriptor, PROTOCOL_MAJOR_MVP,
    PROTOCOL_MINOR_MVP,
};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

const SESSION_CHANNEL_TARGET: &str = "127.0.0.1:27182";
const VIDEO_CHANNEL_TARGET: &str = "127.0.0.1:27183";
const HOST_MAX_WIDTH: u16 = 1920;
const HOST_MAX_HEIGHT: u16 = 1080;
const H264_IFRAME_INTERVAL_MS: u32 = 2_000;
const UITEST_GRPC_START_ATTEMPTS: usize = 5;
const UITEST_GRPC_START_RETRY_MS: u64 = 150;

#[derive(Debug, Clone)]
pub struct SessionBootstrap {
    pub device_id: String,
    pub request: SessionStartRequest,
    pub route: HostRoute,
}

impl SessionBootstrap {
    pub fn new(
        device_id: impl Into<String>,
        request: SessionStartRequest,
        route: HostRoute,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            request,
            route,
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
    pub route: HostRoute,
    pub response: SessionStartResponse,
    pub negotiation: SessionNegotiation,
    pub codec_selection: CodecSelectionDiagnostics,
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
        let companion_plan = self.prepare(bootstrap)?;

        let host_codec_capability = host_codec_capability();
        let preferred_codec_order = normalize_requested_codec_order_for_host_hello(
            &bootstrap.request.requested_codec_order,
            &host_codec_capability,
        )?;
        let session_id = build_session_id(&bootstrap.device_id);
        let session_channel = if bootstrap.route == HostRoute::Uitest {
            uitest_grpc_channel_endpoint()?
        } else {
            session_channel_endpoint()
        };
        let video_channel = video_channel_endpoint();
        let launch_request = CompanionLaunchRequest {
            route: bootstrap.route,
            session_id: session_id.clone(),
            session_channel: session_channel.clone(),
            video_channel: video_channel.clone(),
        };
        if let Err(error) = self
            .companion
            .launch_session(&bootstrap.device_id, &launch_request)
        {
            let error = if bootstrap.route == HostRoute::Uitest {
                append_uitest_runtime_diagnostics(
                    error,
                    collect_uitest_runtime_diagnostics(&self.hdc, &bootstrap.device_id),
                )
            } else {
                error
            };
            return Err(error);
        }

        if bootstrap.route == HostRoute::Uitest {
            let startup_result = start_uitest_official_stream(
                &bootstrap.device_id,
                &bootstrap.request,
                &session_channel,
                &companion_plan.artifact_hint,
            );
            if let Err(error) = startup_result {
                let error = if matches!(error, HostError::TransportFailure(_)) {
                    append_uitest_runtime_diagnostics(
                        error,
                        collect_uitest_runtime_diagnostics(&self.hdc, &bootstrap.device_id),
                    )
                } else {
                    error
                };
                if let Ok(local_port) = endpoint_port(&session_channel) {
                    let _ = cleanup_uitest_runtime(
                        &self.hdc,
                        &bootstrap.device_id,
                        local_port,
                        UITEST_GRPC_SOCKET_NAME,
                    );
                }
                return Err(error);
            }
            return startup_result;
        }

        forward_hdc_endpoint(
            &self.hdc,
            &bootstrap.device_id,
            bootstrap.route,
            &session_channel,
        )?;
        let mut session_runtime = self.transport.open_session_channel(&session_channel)?;

        let host_hello = build_host_hello(&session_id, &bootstrap.request, &preferred_codec_order);
        session_runtime.send_message(&SessionMessage::HostHello(host_hello.clone()))?;
        let first_inbound = session_runtime.receive_message()?;

        let device_hello = match first_inbound {
            InboundSessionMessage::DeviceHello(message) => message,
            InboundSessionMessage::SessionError(error) => {
                return Err(map_session_error(bootstrap.route, &error))
            }
            unexpected => {
                return Err(HostError::ContractViolation(format!(
                    "route `{}` expected device_hello after host_hello, got {}",
                    bootstrap.route,
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
            bootstrap.route,
            control_enabled,
            device_hello.authorization.clone(),
        )?;

        let codec_selection = select_codec_for_startup(
            &bootstrap.request,
            &device_hello.available_video_codecs,
            &host_codec_capability,
        )?;
        let selected_codec = codec_selection.selected_codec.clone();
        let session_config = build_session_config(
            &session_id,
            selected_codec.clone(),
            &bootstrap.request,
            control_enabled,
        );
        session_runtime.send_message(&SessionMessage::SessionConfig(session_config.clone()))?;

        let session_ready = match session_runtime.receive_message()? {
            InboundSessionMessage::SessionReady(message) => message,
            InboundSessionMessage::SessionError(error) => {
                return Err(map_session_error(bootstrap.route, &error))
            }
            unexpected => {
                return Err(HostError::ContractViolation(format!(
                    "route `{}` expected session_ready after session_config, got {}",
                    bootstrap.route,
                    unexpected.message_type()
                )))
            }
        };
        validate_session_ready(&session_ready, &session_id, &selected_codec)?;

        forward_hdc_endpoint(
            &self.hdc,
            &bootstrap.device_id,
            bootstrap.route,
            &video_channel,
        )?;
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
            route: bootstrap.route,
            response,
            negotiation: SessionNegotiation {
                device_hello,
                authorization_updates,
                session_ready,
            },
            codec_selection: codec_selection.diagnostics,
            video_path: classify_negotiated_video_path(&selected_codec),
            outbound_session_messages,
            runtime,
        })
    }
}

impl SessionOrchestrator<RuntimeHdcBridge, RouteCompanionManager<RuntimeHdcBridge>> {
    pub fn for_route(route: HostRoute) -> Self {
        Self::for_route_with_uitest_config(route, UitestLaunchConfig::default())
    }

    pub fn for_route_with_uitest_config(
        route: HostRoute,
        uitest_config: UitestLaunchConfig,
    ) -> Self {
        Self::new(
            RuntimeHdcBridge::new(),
            RouteCompanionManager::with_uitest_config(
                route,
                RuntimeHdcBridge::new(),
                uitest_config,
            ),
        )
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

fn uitest_grpc_channel_endpoint() -> HostResult<ChannelEndpoint> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| {
        HostError::TransportFailure(format!(
            "route `uitest` phase `local-port-selection` failed to reserve local gRPC port: {error}"
        ))
    })?;
    let port = listener.local_addr().map_err(|error| {
        HostError::TransportFailure(format!(
            "route `uitest` phase `local-port-selection` failed to read reserved local gRPC port: {error}"
        ))
    })?.port();
    drop(listener);
    Ok(ChannelEndpoint {
        transport: TransportKind::HdcForward,
        target: format!("127.0.0.1:{port}"),
    })
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
    route: HostRoute,
    endpoint: &ChannelEndpoint,
) -> HostResult<()> {
    if route == HostRoute::Uitest {
        return Ok(());
    }
    if endpoint.transport == TransportKind::HdcForward {
        let port = endpoint_port(endpoint)?;
        hdc.forward_port(device_id, port, port)?;
    }
    Ok(())
}

fn append_uitest_runtime_diagnostics(error: HostError, diagnostics: String) -> HostError {
    match error {
        HostError::TransportFailure(message) => HostError::TransportFailure(format!(
            "{message}; uitest_runtime_diagnostics {diagnostics}"
        )),
        HostError::HdcFailure(message) => HostError::HdcFailure(format!(
            "{message}; uitest_runtime_diagnostics {diagnostics}"
        )),
        other => other,
    }
}

fn start_uitest_official_stream(
    device_id: &str,
    request: &SessionStartRequest,
    session_channel: &ChannelEndpoint,
    selected_payload: &str,
) -> HostResult<SessionStartupPlan> {
    let local_port = endpoint_port(session_channel)?;
    let session_id = build_session_id(device_id);
    let route_supported_codecs = vec![official_h264_descriptor()];
    let host_codec_capability = host_codec_capability();
    let codec_selection =
        select_codec_for_startup(request, &route_supported_codecs, &host_codec_capability)?;
    if codec_selection.selected_codec != VideoCodec::H264 {
        return Err(HostError::ContractViolation(format!(
            "route `uitest` phase `codec-selection` selected unsupported codec {}; official scrcpy stream currently supports h264 only",
            video_codec_name(&codec_selection.selected_codec)
        )));
    }

    let target = OfficialScrcpyClient::for_forwarded_local_tcp(local_port)
        .config()
        .target();
    let mut last_start_error = None;
    let mut stream = None;
    for attempt in 1..=UITEST_GRPC_START_ATTEMPTS {
        let mut start_client = OfficialScrcpyClient::for_forwarded_local_tcp(local_port);
        match start_client.start() {
            Ok(start_stream) => {
                stream = Some(start_stream);
                break;
            }
            Err(error) => {
                last_start_error = Some(error);
                if attempt < UITEST_GRPC_START_ATTEMPTS {
                    thread::sleep(Duration::from_millis(UITEST_GRPC_START_RETRY_MS));
                }
            }
        }
    }
    let stream = stream.ok_or_else(|| {
        let error = last_start_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "ScrcpyService/onStart did not return a stream".to_string());
        HostError::TransportFailure(format!(
            "route `uitest` phase `grpc-start` target `{target}` method `/ScrcpyService/onStart` selected_payload `{selected_payload}` selected_codec=h264 attempts={UITEST_GRPC_START_ATTEMPTS}: {error}"
        ))
    })?;
    wake_official_scrcpy_display(device_id, selected_payload);
    if request.request_official_idr_on_start {
        request_official_scrcpy_idr(local_port, device_id, selected_payload);
    }

    let authorization = FeatureAuthorization {
        video_capture: AuthorizationState::Granted,
        input_injection: AuthorizationState::Unsupported,
    };
    let display = DisplayInfo {
        width: HOST_MAX_WIDTH,
        height: HOST_MAX_HEIGHT,
        rotation: 0,
    };
    let channel_layout = ChannelLayout {
        session: ChannelBinding {
            name: "official-grpc-control".to_string(),
            state: "ready".to_string(),
            payload_type: "grpc_unary".to_string(),
            activation: "onEnd during host stop".to_string(),
        },
        video: ChannelBinding {
            name: "official-grpc-stream".to_string(),
            state: "streaming".to_string(),
            payload_type: "h264_annex_b".to_string(),
            activation: "ScrcpyService/onStart".to_string(),
        },
    };
    let device_hello = DeviceHello {
        session_id: session_id.clone(),
        protocol_major: PROTOCOL_MAJOR_MVP,
        protocol_minor: PROTOCOL_MINOR_MVP,
        companion_version: "official-uitest-scrcpy".to_string(),
        device_name: device_id.to_string(),
        authorization: authorization.clone(),
        available_features: vec![SessionFeature::Video],
        available_video_codecs: route_supported_codecs,
        display: display.clone(),
    };
    let session_ready = SessionReady {
        session_id: session_id.clone(),
        selected_video_codec: VideoCodec::H264,
        channel_layout,
        display,
    };
    let response = SessionStartResponse {
        session_id: session_id.clone(),
        selected_codec: VideoCodec::H264,
        session_channel: session_channel.clone(),
        video_channel: session_channel.clone(),
        control: SessionControlState { enabled: false },
        authorization,
    };
    let runtime = SessionRuntime::new(
        session_id.clone(),
        VideoCodec::H264,
        Box::new(OfficialScrcpyControlChannel {
            device_id: device_id.to_string(),
            local_port,
            target: target.clone(),
            selected_payload: selected_payload.to_string(),
            grpc_socket_name: UITEST_GRPC_SOCKET_NAME.to_string(),
        }),
        Box::new(OfficialScrcpyVideoChannel {
            stream,
            adapter: OfficialScrcpyIngressAdapter::new(0, official_pts_step_us(request)),
        }),
    );

    Ok(SessionStartupPlan {
        route: HostRoute::Uitest,
        response,
        negotiation: SessionNegotiation {
            device_hello,
            authorization_updates: Vec::new(),
            session_ready,
        },
        codec_selection: codec_selection.diagnostics,
        video_path: NegotiatedVideoPath::H264Mainline,
        outbound_session_messages: Vec::new(),
        runtime,
    })
}

struct OfficialScrcpyControlChannel {
    device_id: String,
    local_port: u16,
    target: String,
    selected_payload: String,
    grpc_socket_name: String,
}

impl super::runtime::SessionChannelTransport for OfficialScrcpyControlChannel {
    fn send_message(&mut self, message: &SessionMessage) -> HostResult<()> {
        match message {
            SessionMessage::StopSession(_) => {
                let mut client = OfficialScrcpyClient::for_forwarded_local_tcp(self.local_port);
                let stop_result = client.stop().map(|_| ()).map_err(|error| {
                    HostError::TransportFailure(format!(
                        "route `uitest` phase `grpc-stop` target `{}` method `/ScrcpyService/onEnd` selected_payload `{}` selected_codec=h264: {error}",
                        self.target, self.selected_payload
                    ))
                });
                let cleanup_result = cleanup_uitest_runtime(
                    &RuntimeHdcBridge::new(),
                    &self.device_id,
                    self.local_port,
                    &self.grpc_socket_name,
                );
                stop_result.and(cleanup_result)
            }
            unexpected => Err(HostError::ContractViolation(format!(
                "route `uitest` official control channel only supports stop_session mapped to ScrcpyService/onEnd, got {unexpected:?}"
            ))),
        }
    }

    fn receive_message(&mut self) -> HostResult<InboundSessionMessage> {
        Err(HostError::ContractViolation(
            "route `uitest` official control channel does not expose host-device session messages"
                .to_string(),
        ))
    }
}

struct OfficialScrcpyVideoChannel {
    stream: crate::official_scrcpy::OfficialScrcpyStream,
    adapter: OfficialScrcpyIngressAdapter,
}

impl super::runtime::VideoChannelTransport for OfficialScrcpyVideoChannel {
    fn receive_video_ingress(&mut self) -> HostResult<PreparedVideoIngress> {
        let message = match self.stream.next().ok_or_else(|| {
            HostError::TransportFailure(
                "route `uitest` phase `grpc-stream` official ScrcpyService/onStart stream ended before host stop"
                    .to_string(),
            )
        })? {
            Ok(message) => message,
            Err(HostError::ReceiveTimeout(message)) => {
                return Err(HostError::ReceiveTimeout(message));
            }
            Err(error) => return Err(error),
        };
        let ingress = self.adapter.ingest_message(message)?;
        Ok(ingress)
    }
}

fn wake_official_scrcpy_display(device_id: &str, selected_payload: &str) {
    match RuntimeHdcBridge::new().exec_shell(device_id, "power-shell wakeup") {
        Ok(output) => host_log::debug(
            "uitest",
            "official_scrcpy_wakeup",
            format!(
                "device={} payload={} result={}",
                device_id,
                selected_payload,
                output.trim()
            ),
        ),
        Err(error) => host_log::warn(
            "uitest",
            "official_scrcpy_wakeup_failed",
            format!(
                "device={} payload={} error={}",
                device_id, selected_payload, error
            ),
        ),
    }
}

fn request_official_scrcpy_idr(local_port: u16, device_id: &str, selected_payload: &str) {
    let mut client = OfficialScrcpyClient::for_forwarded_local_tcp(local_port);
    match client.request_idr_frame() {
        Ok(result) => host_log::info(
            "uitest",
            "official_scrcpy_request_idr",
            format!(
                "device={} payload={} method=/ScrcpyService/onRequestIDRFrame result={}",
                device_id, selected_payload, result.result
            ),
        ),
        Err(error) => host_log::warn(
            "uitest",
            "official_scrcpy_request_idr_failed",
            format!(
                "device={} payload={} method=/ScrcpyService/onRequestIDRFrame error={}",
                device_id, selected_payload, error
            ),
        ),
    }
}

fn official_h264_descriptor() -> VideoCodecDescriptor {
    VideoCodecDescriptor {
        codec: VideoCodec::H264,
        encoder_kind: "official-uitest-platform-avc".to_string(),
        max_width: HOST_MAX_WIDTH,
        max_height: HOST_MAX_HEIGHT,
        max_fps: 120,
        bitrate_control: Some("official".to_string()),
    }
}

fn official_pts_step_us(request: &SessionStartRequest) -> u64 {
    if request.preferred_max_fps == 0 {
        0
    } else {
        1_000_000 / u64::from(request.preferred_max_fps)
    }
}

fn video_codec_name(codec: &VideoCodec) -> &'static str {
    match codec {
        VideoCodec::H264 => "h264",
        VideoCodec::Jpeg => "jpeg",
        VideoCodec::H265Experimental => "h265",
    }
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
    route: HostRoute,
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
            InboundSessionMessage::SessionError(error) => {
                return Err(map_session_error(route, &error))
            }
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
    host_codec_capability()
        .supported_codecs
        .into_iter()
        .filter_map(|codec| codec.to_video_codec())
        .collect()
}

fn host_codec_capability() -> HostCodecCapability {
    HostCodecCapability::mvp_h264_jpeg()
}

fn normalize_requested_codec_order_for_host_hello(
    requested_order: &[VideoCodec],
    host_capability: &HostCodecCapability,
) -> HostResult<Vec<VideoCodec>> {
    let normalized = normalized_requested_codec_order(requested_order);
    let host_supported: Vec<VideoCodec> = host_capability
        .supported_codecs
        .iter()
        .filter_map(|codec| codec.to_video_codec())
        .collect();
    let preferred: Vec<VideoCodec> = normalized
        .into_iter()
        .filter(|codec| host_supported.contains(codec))
        .collect();

    if preferred.is_empty() {
        return Err(HostError::ContractViolation(
            "no host-supported codec in request; expected h264 and/or jpeg".to_string(),
        ));
    }

    Ok(preferred)
}

fn map_session_error(route: HostRoute, error: &hscrcpy_contracts::SessionError) -> HostError {
    HostError::ContractViolation(format!("route `{route}` {}: {}", error.code, error.message))
}

#[cfg(test)]
mod tests {
    use super::{
        build_session_id, normalize_requested_codec_order_for_host_hello, resolve_control_mode,
        SessionBootstrap, SessionOrchestrator,
    };
    use crate::companion::{
        CompanionAction, CompanionLaunchRequest, CompanionManager, CompanionPlan,
    };
    use crate::control::{ControlMessageEmitter, OutboundControlEvent, SessionMessageSink};
    use crate::hdc::HdcBridge;
    use crate::route::HostRoute;
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

    struct TestCompanionManager {
        launched_sessions: Rc<RefCell<Vec<CompanionLaunchRequest>>>,
        artifact_hint: String,
    }

    impl Default for TestCompanionManager {
        fn default() -> Self {
            Self {
                launched_sessions: Rc::new(RefCell::new(Vec::new())),
                artifact_hint: "assets/companion/hscrcpy_server.hap".to_string(),
            }
        }
    }

    impl TestCompanionManager {
        fn with_artifact_hint(artifact_hint: impl Into<String>) -> Self {
            Self {
                artifact_hint: artifact_hint.into(),
                ..Self::default()
            }
        }
    }

    impl CompanionManager for TestCompanionManager {
        fn plan(&self, _device_id: &str) -> HostResult<CompanionPlan> {
            Ok(CompanionPlan {
                action: CompanionAction::Skip,
                target_version: "1.0.0".to_string(),
                artifact_hint: self.artifact_hint.clone(),
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
            self.launched_sessions.borrow_mut().push(request.clone());
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
        fn receive_video_ingress(&mut self) -> HostResult<PreparedVideoIngress> {
            let packet = self
                .packets
                .pop_front()
                .expect("unexpected video receive in test")?;
            PreparedVideoIngress::from_packet(packet)
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
    fn rejects_control_request_when_device_cannot_inject_input() {
        let err = resolve_control_mode(true, &[SessionFeature::Video])
            .expect_err("control support mismatch must fail");
        assert!(err.to_string().contains("feature_unsupported"));
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
            HostRoute::HscrcpyServer,
        );

        let mut plan = orchestrator
            .start(&bootstrap)
            .expect("startup plan should succeed");

        assert_eq!(plan.response.selected_codec, VideoCodec::Jpeg);
        assert_eq!(
            plan.codec_selection
                .selected_codec
                .as_ref()
                .map(|codec| codec.as_str()),
            Some("jpeg")
        );
        assert!(plan
            .codec_selection
            .fallback_reason
            .as_deref()
            .expect("jpeg selection should include fallback reason")
            .contains("unsupported by route/device encoder"));
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
                HostRoute::HscrcpyServer,
            ))
            .expect("startup should succeed before ingest validation");
        assert_eq!(plan.response.selected_codec, VideoCodec::H264);
        assert!(plan.codec_selection.fallback_reason.is_none());

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
    fn uitest_startup_rejects_unsupported_codec_without_hap_fallback() {
        let forwarded_ports = Rc::new(RefCell::new(Vec::new()));
        let companion = TestCompanionManager::with_artifact_hint(
            "third_party/hypium/hosScrcpy/6.1.0.210/libscrcpy/libscrcpy_server_unix_6.5-20260313.z.so",
        );
        let launched_sessions = companion.launched_sessions.clone();
        let sent_messages = Rc::new(RefCell::new(Vec::new()));
        let opened_video_codecs = Rc::new(RefCell::new(Vec::new()));
        let orchestrator = SessionOrchestrator::with_transport(
            TestHdcBridge {
                forwarded_ports: forwarded_ports.clone(),
            },
            companion,
            TestTransportFactory {
                session_messages: Rc::new(RefCell::new(Some(VecDeque::new()))),
                video_packets: Rc::new(RefCell::new(Some(VecDeque::new()))),
                sent_messages: sent_messages.clone(),
                opened_video_codecs: opened_video_codecs.clone(),
            },
        );

        let err = match orchestrator.start(&SessionBootstrap::new(
            "usb-device-001",
            SessionStartRequest::jpeg_baseline(60, false),
            HostRoute::Uitest,
        )) {
            Ok(_) => panic!("uitest route should reject unsupported jpeg request"),
            Err(err) => err,
        };

        assert!(matches!(err, HostError::ContractViolation(_)));
        let message = err.to_string();
        assert!(
            message.contains("no_shared_video_codec"),
            "unexpected error: {message}"
        );
        assert_eq!(launched_sessions.borrow().len(), 1);
        assert_eq!(launched_sessions.borrow()[0].route, HostRoute::Uitest);
        assert!(
            forwarded_ports.borrow().is_empty(),
            "uitest startup must not forward HAP session/video ports"
        );
        assert!(
            sent_messages.borrow().is_empty(),
            "uitest startup must not enter HAP JSON handshake"
        );
        assert!(
            opened_video_codecs.borrow().is_empty(),
            "uitest startup must not open HAP video channel"
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
            HostRoute::HscrcpyServer,
        )) {
            Ok(_) => panic!("session_error should fail startup"),
            Err(err) => err,
        };
        assert!(matches!(err, HostError::ContractViolation(_)));
        assert!(err.to_string().contains("protocol_major_mismatch"));
    }

    #[test]
    fn rejects_h265_only_request_before_host_hello_is_sent() {
        let sent_messages = Rc::new(RefCell::new(Vec::new()));
        let orchestrator = SessionOrchestrator::with_transport(
            TestHdcBridge {
                forwarded_ports: Rc::new(RefCell::new(Vec::new())),
            },
            TestCompanionManager::default(),
            TestTransportFactory {
                session_messages: Rc::new(RefCell::new(Some(VecDeque::new()))),
                video_packets: Rc::new(RefCell::new(Some(VecDeque::new()))),
                sent_messages: sent_messages.clone(),
                opened_video_codecs: Rc::new(RefCell::new(Vec::new())),
            },
        );

        let err = match orchestrator.start(&SessionBootstrap::new(
            "usb-device-001",
            SessionStartRequest {
                requested_codec_order: vec![VideoCodec::H265Experimental],
                preferred_max_fps: 60,
                enable_control: false,
                request_official_idr_on_start: false,
            },
            HostRoute::HscrcpyServer,
        )) {
            Ok(_) => panic!("h265-only request should fail before negotiation"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("no host-supported codec"));
        assert!(
            sent_messages.borrow().is_empty(),
            "host_hello must not be sent for unsupported requests"
        );
    }

    #[test]
    fn host_hello_preferred_codecs_are_subset_of_supported_codecs() {
        let preferred = normalize_requested_codec_order_for_host_hello(
            &[VideoCodec::H265Experimental, VideoCodec::H264],
            &super::host_codec_capability(),
        )
        .expect("mixed request should keep host-supported codecs");
        assert_eq!(preferred, vec![VideoCodec::H264, VideoCodec::Jpeg]);
    }
}
