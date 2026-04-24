use crate::cancellation::CancellationToken;
use crate::{HostError, HostResult};
use hscrcpy_contracts::VideoCodec;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tonic::transport::{Channel, Endpoint};

pub const DEFAULT_MAX_RECEIVE_MESSAGE_BYTES: usize = 10 * 1024 * 1024;
const GRPC_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
const GRPC_CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(100);
const GRPC_STREAM_START_POLL_TIMEOUT: Duration = Duration::from_millis(100);
const GRPC_STREAM_POLL_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialScrcpyClientConfig {
    pub host: String,
    pub port: u16,
    pub max_receive_message_bytes: usize,
    pub insecure: bool,
}

impl OfficialScrcpyClientConfig {
    pub fn forwarded_local_tcp(local_port: u16) -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: local_port,
            max_receive_message_bytes: DEFAULT_MAX_RECEIVE_MESSAGE_BYTES,
            insecure: true,
        }
    }

    pub fn target(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    fn uri(&self) -> String {
        let scheme = if self.insecure { "http" } else { "https" };
        format!("{scheme}://{}", self.target())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialScrcpyVideoMessage {
    pub codec: VideoCodec,
    pub payload_key: String,
    pub payload: Vec<u8>,
    pub reply_type: i32,
    pub data: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialScrcpyControlResult {
    pub result: i32,
}

pub trait OfficialScrcpyTransport {
    type StartStream: Iterator<Item = HostResult<protocol::ReplyMessage>>;

    fn on_start(&mut self, config: &OfficialScrcpyClientConfig) -> HostResult<Self::StartStream>;
    fn on_end(
        &mut self,
        config: &OfficialScrcpyClientConfig,
    ) -> HostResult<protocol::ReplyEndMessage>;
    fn on_request_idr_frame(
        &mut self,
        config: &OfficialScrcpyClientConfig,
    ) -> HostResult<protocol::ReplyEndMessage>;
}

pub struct OfficialScrcpyClient<T> {
    config: OfficialScrcpyClientConfig,
    transport: T,
}

impl<T> OfficialScrcpyClient<T> {
    pub fn new(config: OfficialScrcpyClientConfig, transport: T) -> Self {
        Self { config, transport }
    }

    pub fn config(&self) -> &OfficialScrcpyClientConfig {
        &self.config
    }
}

impl OfficialScrcpyClient<TonicGrpcTransport> {
    pub fn for_forwarded_local_tcp(local_port: u16) -> Self {
        Self::new(
            OfficialScrcpyClientConfig::forwarded_local_tcp(local_port),
            TonicGrpcTransport,
        )
    }

    pub fn start_with_cancellation<C>(
        &mut self,
        cancellation: &C,
    ) -> HostResult<OfficialScrcpyStream>
    where
        C: CancellationToken + ?Sized,
    {
        let stream = self
            .transport
            .on_start_with_cancellation(&self.config, cancellation)?;
        Ok(OfficialScrcpyStream::new(Box::new(
            stream.map(|message| message.and_then(convert_reply_message)),
        )))
    }
}

impl<T> OfficialScrcpyClient<T>
where
    T: OfficialScrcpyTransport,
    T::StartStream: 'static,
{
    pub fn start(&mut self) -> HostResult<OfficialScrcpyStream> {
        let stream = self.transport.on_start(&self.config)?;
        Ok(OfficialScrcpyStream::new(Box::new(
            stream.map(|message| message.and_then(convert_reply_message)),
        )))
    }

    pub fn stop(&mut self) -> HostResult<OfficialScrcpyControlResult> {
        self.transport
            .on_end(&self.config)
            .map(OfficialScrcpyControlResult::from)
    }

    pub fn request_idr_frame(&mut self) -> HostResult<OfficialScrcpyControlResult> {
        self.transport
            .on_request_idr_frame(&self.config)
            .map(OfficialScrcpyControlResult::from)
    }
}

pub struct OfficialScrcpyStream {
    inner: Box<dyn Iterator<Item = HostResult<OfficialScrcpyVideoMessage>>>,
}

impl OfficialScrcpyStream {
    fn new(inner: Box<dyn Iterator<Item = HostResult<OfficialScrcpyVideoMessage>>>) -> Self {
        Self { inner }
    }
}

impl Iterator for OfficialScrcpyStream {
    type Item = HostResult<OfficialScrcpyVideoMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TonicGrpcTransport;

impl OfficialScrcpyTransport for TonicGrpcTransport {
    type StartStream = TonicGrpcStartStream;

    fn on_start(&mut self, config: &OfficialScrcpyClientConfig) -> HostResult<Self::StartStream> {
        TonicGrpcConnection::open_streaming(config, protocol::ON_START_PATH)
    }

    fn on_end(
        &mut self,
        config: &OfficialScrcpyClientConfig,
    ) -> HostResult<protocol::ReplyEndMessage> {
        TonicGrpcConnection::unary(
            config,
            protocol::ON_END_PATH,
            |mut client, request| async move { client.on_end(request).await },
        )
    }

    fn on_request_idr_frame(
        &mut self,
        config: &OfficialScrcpyClientConfig,
    ) -> HostResult<protocol::ReplyEndMessage> {
        TonicGrpcConnection::unary(
            config,
            protocol::ON_REQUEST_IDR_FRAME_PATH,
            |mut client, request| async move { client.on_request_idr_frame(request).await },
        )
    }
}

impl TonicGrpcTransport {
    fn on_start_with_cancellation<C>(
        &mut self,
        config: &OfficialScrcpyClientConfig,
        cancellation: &C,
    ) -> HostResult<TonicGrpcStartStream>
    where
        C: CancellationToken + ?Sized,
    {
        TonicGrpcConnection::open_streaming_with_cancellation(
            config,
            protocol::ON_START_PATH,
            cancellation,
        )
    }
}

struct TonicGrpcConnection {
    runtime: Runtime,
    client: protocol::scrcpy_service_client::ScrcpyServiceClient<Channel>,
}

impl TonicGrpcConnection {
    fn connect(config: &OfficialScrcpyClientConfig, method: &str) -> HostResult<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .map_err(|error| grpc_transport_error(config, method, "runtime", error))?;

        let endpoint = Endpoint::from_shared(config.uri())
            .map_err(|error| grpc_transport_error(config, method, "build-endpoint", error))?
            .connect_timeout(GRPC_CONNECT_TIMEOUT);

        let start = Instant::now();
        let channel = loop {
            match runtime.block_on(endpoint.clone().connect()) {
                Ok(channel) => break channel,
                Err(error) if start.elapsed() < GRPC_CONNECT_TIMEOUT => {
                    let last_error = error;
                    std::thread::sleep(GRPC_CONNECT_RETRY_INTERVAL);
                    if start.elapsed() >= GRPC_CONNECT_TIMEOUT {
                        return Err(grpc_transport_error(
                            config,
                            method,
                            "grpc-connect",
                            last_error,
                        ));
                    }
                }
                Err(error) => {
                    return Err(grpc_transport_error(config, method, "grpc-connect", error))
                }
            }
        };

        let client = protocol::scrcpy_service_client::ScrcpyServiceClient::new(channel)
            .max_decoding_message_size(config.max_receive_message_bytes)
            .max_encoding_message_size(config.max_receive_message_bytes);

        Ok(Self { runtime, client })
    }

    fn open_streaming(
        config: &OfficialScrcpyClientConfig,
        method: &str,
    ) -> HostResult<TonicGrpcStartStream> {
        Self::open_streaming_with_cancellation(config, method, &crate::cancellation::NoCancellation)
    }

    fn open_streaming_with_cancellation<C>(
        config: &OfficialScrcpyClientConfig,
        method: &str,
        cancellation: &C,
    ) -> HostResult<TonicGrpcStartStream>
    where
        C: CancellationToken + ?Sized,
    {
        if cancellation.is_cancelled() {
            return Err(HostError::ShutdownRequested(format!(
                "host shutdown requested before {method}"
            )));
        }
        let TonicGrpcConnection {
            runtime,
            mut client,
        } = Self::connect(config, method)?;
        if cancellation.is_cancelled() {
            return Err(HostError::ShutdownRequested(format!(
                "host shutdown requested before grpc-start target `{}` method `{method}`",
                config.target()
            )));
        }
        let response = runtime.block_on(async {
            let start = client.on_start(tonic::Request::new(protocol::Empty {}));
            tokio::pin!(start);
            loop {
                tokio::select! {
                    response = &mut start => {
                        return response
                            .map_err(|error| grpc_status_error(config, method, "grpc-start", error));
                    }
                    _ = tokio::time::sleep(GRPC_STREAM_START_POLL_TIMEOUT) => {
                        if cancellation.is_cancelled() {
                            return Err(HostError::ShutdownRequested(format!(
                                "host shutdown requested during grpc-start target `{}` method `{method}`",
                                config.target()
                            )));
                        }
                    }
                }
            }
        })?;

        Ok(TonicGrpcStartStream {
            inner: response.into_inner(),
            runtime,
            method: method.to_string(),
            target: config.target(),
            max_receive_message_bytes: config.max_receive_message_bytes,
            complete: false,
        })
    }

    fn unary<F, Fut>(
        config: &OfficialScrcpyClientConfig,
        method: &str,
        call: F,
    ) -> HostResult<protocol::ReplyEndMessage>
    where
        F: FnOnce(
            protocol::scrcpy_service_client::ScrcpyServiceClient<Channel>,
            tonic::Request<protocol::Empty>,
        ) -> Fut,
        Fut: std::future::Future<
            Output = Result<tonic::Response<protocol::ReplyEndMessage>, tonic::Status>,
        >,
    {
        let TonicGrpcConnection { runtime, client } = Self::connect(config, method)?;
        let request = tonic::Request::new(protocol::Empty {});
        let response = runtime
            .block_on(call(client, request))
            .map_err(|error| grpc_status_error(config, method, "grpc-unary", error))?;
        Ok(response.into_inner())
    }
}

pub struct TonicGrpcStartStream {
    inner: tonic::Streaming<protocol::ReplyMessage>,
    runtime: Runtime,
    method: String,
    target: String,
    max_receive_message_bytes: usize,
    complete: bool,
}

impl Iterator for TonicGrpcStartStream {
    type Item = HostResult<protocol::ReplyMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.complete {
            return None;
        }

        match self.runtime.block_on(async {
            tokio::time::timeout(GRPC_STREAM_POLL_TIMEOUT, self.inner.message()).await
        }) {
            Err(_) => Some(Err(HostError::ReceiveTimeout(format!(
                "official scrcpy gRPC route=uitest phase=grpc-stream target `{}` method `{}` max_receive_message_bytes={} timed out after {}ms",
                self.target,
                self.method,
                self.max_receive_message_bytes,
                GRPC_STREAM_POLL_TIMEOUT.as_millis()
            )))),
            Ok(Ok(Some(message))) => Some(Ok(message)),
            Ok(Ok(None)) => {
                self.complete = true;
                None
            }
            Ok(Err(error)) => {
                self.complete = true;
                Some(Err(HostError::TransportFailure(format!(
                    "official scrcpy gRPC route=uitest phase=grpc-stream target `{}` method `{}` max_receive_message_bytes={}: {error}",
                    self.target, self.method, self.max_receive_message_bytes
                ))))
            }
        }
    }
}

fn grpc_transport_error(
    config: &OfficialScrcpyClientConfig,
    method: &str,
    phase: &str,
    error: impl std::fmt::Display,
) -> HostError {
    HostError::TransportFailure(format!(
        "official scrcpy gRPC route=uitest phase={phase} target `{}` method `{method}` max_receive_message_bytes={}: {error}",
        config.target(),
        config.max_receive_message_bytes
    ))
}

fn grpc_status_error(
    config: &OfficialScrcpyClientConfig,
    method: &str,
    phase: &str,
    error: tonic::Status,
) -> HostError {
    HostError::TransportFailure(format!(
        "official scrcpy gRPC route=uitest phase={phase} target `{}` method `{method}` max_receive_message_bytes={} grpc-status={} grpc-message={}",
        config.target(),
        config.max_receive_message_bytes,
        error.code(),
        error.message()
    ))
}

impl From<protocol::ReplyEndMessage> for OfficialScrcpyControlResult {
    fn from(value: protocol::ReplyEndMessage) -> Self {
        Self {
            result: value.result,
        }
    }
}

pub fn convert_reply_message(
    message: protocol::ReplyMessage,
) -> HostResult<OfficialScrcpyVideoMessage> {
    let bytes_candidates = message
        .payload
        .iter()
        .filter_map(|(key, value)| match value.values.as_ref() {
            Some(protocol::param_value::Values::ValBytes(bytes)) => Some((key, bytes)),
            _ => None,
        })
        .collect::<Vec<_>>();

    match bytes_candidates.as_slice() {
        [] => Err(HostError::ContractViolation(format!(
            "official scrcpy ReplyMessage has no bytes payload candidate: reply_type={} data_present={} payload_keys=[{}]",
            message.reply_type,
            !message.data.is_empty(),
            message.payload.keys().cloned().collect::<Vec<_>>().join(",")
        ))),
        [(key, bytes)] if bytes.is_empty() => Err(HostError::ContractViolation(format!(
            "official scrcpy ReplyMessage bytes payload `{}` is empty: reply_type={}",
            key, message.reply_type
        ))),
        [(key, bytes)] => Ok(OfficialScrcpyVideoMessage {
            codec: VideoCodec::H264,
            payload_key: (*key).clone(),
            payload: (*bytes).clone(),
            reply_type: message.reply_type,
            data: (!message.data.is_empty()).then_some(message.data),
        }),
        candidates => Err(HostError::ContractViolation(format!(
            "official scrcpy ReplyMessage has ambiguous bytes payload candidates: reply_type={} keys=[{}]",
            message.reply_type,
            candidates
                .iter()
                .map(|(key, _)| key.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ))),
    }
}

pub mod protocol {
    pub const SERVICE_NAME: &str = "ScrcpyService";
    pub const ON_START_PATH: &str = "/ScrcpyService/onStart";
    pub const ON_END_PATH: &str = "/ScrcpyService/onEnd";
    pub const ON_REQUEST_IDR_FRAME_PATH: &str = "/ScrcpyService/onRequestIDRFrame";

    include!(concat!(env!("OUT_DIR"), "/scrcpy.rs"));
}

#[cfg(test)]
mod tests {
    use super::{
        convert_reply_message, OfficialScrcpyClient, OfficialScrcpyClientConfig,
        OfficialScrcpyControlResult, OfficialScrcpyTransport, DEFAULT_MAX_RECEIVE_MESSAGE_BYTES,
    };
    use crate::cancellation::CancellationToken;
    use crate::official_scrcpy::protocol::{
        param_value, ParamValue, ReplyEndMessage, ReplyMessage,
    };
    use crate::render::{BringupRenderSurface, RenderSessionDescriptor};
    use crate::video::OfficialScrcpyIngressAdapter;
    use crate::{HostError, HostResult};
    use hscrcpy_contracts::VideoCodec;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Cancelled;

    impl CancellationToken for Cancelled {
        fn is_cancelled(&self) -> bool {
            true
        }
    }

    #[derive(Clone, Default)]
    struct RecordingTransport {
        state: Rc<RefCell<RecordingTransportState>>,
    }

    struct RecordingTransportState {
        seen_targets: Vec<String>,
        seen_receive_limits: Vec<usize>,
        start_messages: Vec<HostResult<ReplyMessage>>,
        stop_result: HostResult<ReplyEndMessage>,
        idr_result: HostResult<ReplyEndMessage>,
    }

    impl Default for RecordingTransportState {
        fn default() -> Self {
            Self {
                seen_targets: Vec::new(),
                seen_receive_limits: Vec::new(),
                start_messages: Vec::new(),
                stop_result: Ok(ReplyEndMessage::default()),
                idr_result: Ok(ReplyEndMessage::default()),
            }
        }
    }

    impl RecordingTransport {
        fn with_start_messages(messages: Vec<HostResult<ReplyMessage>>) -> Self {
            let transport = Self::default();
            transport.state.borrow_mut().start_messages = messages;
            transport.state.borrow_mut().stop_result = Ok(ReplyEndMessage { result: 7 });
            transport.state.borrow_mut().idr_result = Ok(ReplyEndMessage { result: 1 });
            transport
        }

        fn seen_receive_limits(&self) -> Vec<usize> {
            self.state.borrow().seen_receive_limits.clone()
        }
    }

    impl OfficialScrcpyTransport for RecordingTransport {
        type StartStream = std::vec::IntoIter<HostResult<ReplyMessage>>;

        fn on_start(
            &mut self,
            config: &OfficialScrcpyClientConfig,
        ) -> HostResult<Self::StartStream> {
            let mut state = self.state.borrow_mut();
            state.seen_targets.push(config.target());
            state
                .seen_receive_limits
                .push(config.max_receive_message_bytes);
            Ok(std::mem::take(&mut state.start_messages).into_iter())
        }

        fn on_end(&mut self, config: &OfficialScrcpyClientConfig) -> HostResult<ReplyEndMessage> {
            let mut state = self.state.borrow_mut();
            state.seen_targets.push(config.target());
            state
                .seen_receive_limits
                .push(config.max_receive_message_bytes);
            state.stop_result.clone()
        }

        fn on_request_idr_frame(
            &mut self,
            config: &OfficialScrcpyClientConfig,
        ) -> HostResult<ReplyEndMessage> {
            let mut state = self.state.borrow_mut();
            state.seen_targets.push(config.target());
            state
                .seen_receive_limits
                .push(config.max_receive_message_bytes);
            state.idr_result.clone()
        }
    }

    fn generated_value(value: param_value::Values) -> ParamValue {
        ParamValue {
            values: Some(value),
        }
    }

    fn reply_with_payload(payload: BTreeMap<String, ParamValue>) -> ReplyMessage {
        ReplyMessage {
            data: "first-frame".to_string(),
            reply_type: 3,
            payload,
        }
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("hscrcpy-official-{name}-{unique}"));
            fs::create_dir_all(&path).expect("test temp dir should create");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn constructs_forwarded_local_tcp_client_with_ten_mib_receive_limit() {
        let client = OfficialScrcpyClient::for_forwarded_local_tcp(27182);

        assert_eq!(client.config().target(), "127.0.0.1:27182");
        assert_eq!(
            client.config().max_receive_message_bytes,
            DEFAULT_MAX_RECEIVE_MESSAGE_BYTES
        );
        assert!(client.config().insecure);
    }

    #[test]
    fn converts_generated_single_bytes_payload_candidate_to_h264_stream_message() {
        let mut payload = BTreeMap::new();
        payload.insert(
            "ReplyMessage.payload".to_string(),
            generated_value(param_value::Values::ValBytes(vec![
                0, 0, 1, 0x65, 0x88, 0x84,
            ])),
        );
        payload.insert(
            "frame_count".to_string(),
            generated_value(param_value::Values::ValInt(1)),
        );

        let converted =
            convert_reply_message(reply_with_payload(payload)).expect("bytes payload should map");

        assert_eq!(converted.codec, VideoCodec::H264);
        assert_eq!(converted.payload_key, "ReplyMessage.payload");
        assert_eq!(converted.payload, vec![0, 0, 1, 0x65, 0x88, 0x84]);
        assert_eq!(converted.reply_type, 3);
        assert_eq!(converted.data.as_deref(), Some("first-frame"));
    }

    #[test]
    fn generated_reply_end_maps_to_route_neutral_control_result() {
        let result = OfficialScrcpyControlResult::from(ReplyEndMessage { result: 17 });

        assert_eq!(result, OfficialScrcpyControlResult { result: 17 });
    }

    #[test]
    fn rejects_reply_message_without_bytes_payload_candidate() {
        let mut payload = BTreeMap::new();
        payload.insert(
            "status".to_string(),
            generated_value(param_value::Values::ValString("ok".to_string())),
        );

        let err = convert_reply_message(reply_with_payload(payload))
            .expect_err("non-bytes payload shape must fail");

        assert!(matches!(err, HostError::ContractViolation(_)));
        assert!(err.to_string().contains("no bytes payload candidate"));
        assert!(err.to_string().contains("status"));
    }

    #[test]
    fn rejects_ambiguous_bytes_payload_candidates() {
        let mut payload = BTreeMap::new();
        payload.insert(
            "first".to_string(),
            generated_value(param_value::Values::ValBytes(vec![1])),
        );
        payload.insert(
            "second".to_string(),
            generated_value(param_value::Values::ValBytes(vec![2])),
        );

        let err = convert_reply_message(reply_with_payload(payload))
            .expect_err("ambiguous bytes payload shape must fail");

        assert!(matches!(err, HostError::ContractViolation(_)));
        assert!(err.to_string().contains("ambiguous bytes payload"));
        assert!(err.to_string().contains("first"));
        assert!(err.to_string().contains("second"));
    }

    #[test]
    fn start_stop_and_idr_delegate_through_transport_with_receive_limit() {
        let mut payload = BTreeMap::new();
        payload.insert(
            "payload".to_string(),
            generated_value(param_value::Values::ValBytes(vec![9, 8, 7])),
        );
        let transport =
            RecordingTransport::with_start_messages(vec![Ok(reply_with_payload(payload))]);
        let observer = transport.clone();
        let mut client = OfficialScrcpyClient::new(
            OfficialScrcpyClientConfig::forwarded_local_tcp(27182),
            transport,
        );

        let mut stream = client.start().expect("start should create stream");
        let message = stream
            .next()
            .expect("stream should have a first item")
            .expect("stream item should convert");
        assert_eq!(message.payload, vec![9, 8, 7]);
        drop(stream);

        assert_eq!(
            client.stop().expect("stop should delegate"),
            OfficialScrcpyControlResult { result: 7 }
        );
        assert_eq!(
            client
                .request_idr_frame()
                .expect("request idr should delegate"),
            OfficialScrcpyControlResult { result: 1 }
        );
        assert_eq!(
            observer.seen_receive_limits(),
            vec![
                DEFAULT_MAX_RECEIVE_MESSAGE_BYTES,
                DEFAULT_MAX_RECEIVE_MESSAGE_BYTES,
                DEFAULT_MAX_RECEIVE_MESSAGE_BYTES
            ]
        );
    }

    #[test]
    fn start_with_cancellation_returns_before_connect_when_cancelled() {
        let mut client = OfficialScrcpyClient::for_forwarded_local_tcp(27182);

        let err = match client.start_with_cancellation(&Cancelled) {
            Ok(_) => panic!("cancelled start should not try to connect"),
            Err(error) => error,
        };

        assert!(matches!(err, HostError::ShutdownRequested(_)));
        assert!(
            err.to_string()
                .contains("host shutdown requested before /ScrcpyService/onStart"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn mock_official_stream_feeds_existing_h264_render_surface() {
        let access_unit = vec![0, 0, 1, 0x65, 0x88, 0x84];
        let mut payload = BTreeMap::new();
        payload.insert(
            "video_payload".to_string(),
            generated_value(param_value::Values::ValBytes(access_unit.clone())),
        );
        let transport =
            RecordingTransport::with_start_messages(vec![Ok(reply_with_payload(payload))]);
        let mut client = OfficialScrcpyClient::new(
            OfficialScrcpyClientConfig::forwarded_local_tcp(27182),
            transport,
        );

        let message = client
            .start()
            .expect("mock official stream should start")
            .next()
            .expect("mock stream should yield one message")
            .expect("mock message should convert");
        let ingress = OfficialScrcpyIngressAdapter::default()
            .ingest_message(message)
            .expect("official h264 message should normalize");

        let temp = TestDir::new("render");
        let mut surface = BringupRenderSurface::new(temp.path()).with_h264_artifact_recording();
        surface
            .initialize_session(RenderSessionDescriptor {
                session_id: "official-h264".to_string(),
                selected_codec: VideoCodec::H264,
                display_width: 1280,
                display_height: 720,
            })
            .expect("render surface should initialize");

        let artifact = surface
            .present_video_ingress(&ingress)
            .expect("route-neutral ingress should render");

        assert_eq!(artifact.codec, VideoCodec::H264);
        assert_eq!(artifact.timestamp_micros, 0);
        assert_eq!(surface.stats().h264_access_units, 1);
        assert_eq!(
            fs::read(
                artifact
                    .artifact_path
                    .as_ref()
                    .expect("h264 artifact path should exist")
            )
            .expect("frame artifact should exist"),
            access_unit
        );
        let session_dir = surface.session_dir().expect("session dir should exist");
        assert!(
            fs::read(session_dir.join("stream.h264"))
                .expect("stream artifact should exist")
                .ends_with(&access_unit),
            "stream.h264 should contain the official access unit"
        );
    }
}
