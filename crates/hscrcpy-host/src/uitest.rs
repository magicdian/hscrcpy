use crate::companion::{CompanionAction, CompanionLaunchRequest, CompanionManager, CompanionPlan};
use crate::hdc::{HdcBridge, HdcForwardSpec};
use crate::{HostError, HostResult};
use hscrcpy_contracts::TransportKind;
use std::fs;
use std::path::{Path, PathBuf};

const ROUTE: &str = "uitest";
const PHASE_PAYLOAD_SELECTION: &str = "payload-selection";
const PHASE_STALE_SCAN: &str = "stale-process-scan";
const PHASE_STALE_KILL: &str = "stale-process-kill";
const PHASE_PAYLOAD_PUSH: &str = "payload-push";
const PHASE_LAUNCH: &str = "launch";
const PHASE_FORWARD: &str = "forward";
const PHASE_CLEANUP: &str = "cleanup";

pub const UITEST_REMOTE_PAYLOAD_PATH: &str = "/data/local/tmp/scrcpy_server.so";
pub const UITEST_GRPC_SOCKET_NAME: &str = "scrcpy_grpc_socket";

const DEFAULT_LOCAL_GRPC_PORT: u16 = 27_182;
const DEFAULT_FRAME_RATE: u16 = 60;
const DEFAULT_BIT_RATE: u32 = 31_457_280;
const DEFAULT_SCREEN_ID: u16 = 0;
const DEFAULT_IFRAME_INTERVAL_MS: u32 = 2_000;
const DEFAULT_REPEAT_INTERVAL_MS: u16 = 33;
const DEFAULT_RECORD_PORT_ARG: u16 = 9_958;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UitestLaunchConfig {
    pub payload_override: Option<PathBuf>,
    pub archive_root: PathBuf,
    pub remote_payload_path: String,
    pub grpc_socket_name: String,
    pub local_grpc_port: u16,
}

impl UitestLaunchConfig {
    pub fn with_payload_override(payload_override: impl Into<PathBuf>) -> Self {
        Self {
            payload_override: Some(payload_override.into()),
            ..Self::default()
        }
    }
}

impl Default for UitestLaunchConfig {
    fn default() -> Self {
        Self {
            payload_override: None,
            archive_root: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../third_party/hypium/hosScrcpy/6.1.0.210/libscrcpy"),
            remote_payload_path: UITEST_REMOTE_PAYLOAD_PATH.to_string(),
            grpc_socket_name: UITEST_GRPC_SOCKET_NAME.to_string(),
            local_grpc_port: DEFAULT_LOCAL_GRPC_PORT,
        }
    }
}

pub struct UitestCompanionManager<H> {
    hdc: H,
    config: UitestLaunchConfig,
}

impl<H> UitestCompanionManager<H>
where
    H: HdcBridge,
{
    pub fn new(hdc: H) -> Self {
        Self::with_config(hdc, UitestLaunchConfig::default())
    }

    pub fn with_config(hdc: H, config: UitestLaunchConfig) -> Self {
        Self { hdc, config }
    }
}

impl<H> CompanionManager for UitestCompanionManager<H>
where
    H: HdcBridge,
{
    fn plan(&self, _device_id: &str) -> HostResult<CompanionPlan> {
        let payload = select_official_payload(&self.config)?;
        Ok(CompanionPlan {
            action: CompanionAction::Skip,
            target_version: "route-managed".to_string(),
            artifact_hint: payload.display().to_string(),
        })
    }

    fn apply(&self, _device_id: &str, _plan: &CompanionPlan) -> HostResult<()> {
        Ok(())
    }

    fn launch_session(&self, device_id: &str, request: &CompanionLaunchRequest) -> HostResult<()> {
        if request.route.as_cli_value() != ROUTE {
            return Err(HostError::ContractViolation(format!(
                "route `{}` cannot be launched by uitest lifecycle",
                request.route
            )));
        }

        let payload = select_official_payload(&self.config)?;
        let local_grpc_port =
            resolve_local_grpc_port(request).unwrap_or(self.config.local_grpc_port);

        let stale = scan_stale_processes(&self.hdc, device_id)?;
        if !stale.is_empty() {
            kill_stale_processes(&self.hdc, device_id, &stale)?;
            let remaining = scan_stale_processes(&self.hdc, device_id)?;
            if !remaining.is_empty() {
                return Err(lifecycle_error(
                    PHASE_STALE_KILL,
                    "xdevice_scrcpy",
                    format!(
                        "stale route-owned process(es) remained alive after kill: {}",
                        format_processes(&remaining)
                    ),
                ));
            }
        }

        if let Err(error) =
            self.hdc
                .push_file(device_id, &payload, &self.config.remote_payload_path)
        {
            let cleanup = cleanup_payload(&self.hdc, device_id, &self.config);
            return Err(cleanup.err().unwrap_or_else(|| {
                lifecycle_error(
                    PHASE_PAYLOAD_PUSH,
                    &self.config.remote_payload_path,
                    error.to_string(),
                )
            }));
        }

        let launch_result = self
            .hdc
            .exec_shell(device_id, &launch_command(&self.config))
            .map_err(|error| {
                lifecycle_error(PHASE_LAUNCH, "uitest start-daemon", error.to_string())
            })
            .and_then(|_| {
                self.hdc
                    .forward(
                        device_id,
                        &HdcForwardSpec::tcp_to_localabstract(
                            local_grpc_port,
                            self.config.grpc_socket_name.clone(),
                        ),
                    )
                    .map_err(|error| {
                        lifecycle_error(
                            PHASE_FORWARD,
                            &format!("localabstract:{}", self.config.grpc_socket_name),
                            error.to_string(),
                        )
                    })
            });

        let cleanup_result = cleanup_payload(&self.hdc, device_id, &self.config);
        match (launch_result, cleanup_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(_), Err(cleanup_error)) => Err(cleanup_error),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
        }
    }
}

pub fn select_official_payload(config: &UitestLaunchConfig) -> HostResult<PathBuf> {
    if let Some(payload_override) = &config.payload_override {
        return validate_payload_path(payload_override, "override");
    }

    let mut unix_payloads = Vec::new();
    let mut fallback_payloads = Vec::new();
    collect_payload_candidates(
        &config.archive_root,
        &mut unix_payloads,
        &mut fallback_payloads,
    )?;

    unix_payloads.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    fallback_payloads.sort_by(|left, right| left.file_name().cmp(&right.file_name()));

    unix_payloads
        .pop()
        .or_else(|| fallback_payloads.pop())
        .map(|path| {
            fs::canonicalize(&path).map_err(|error| {
                payload_selection_error(&path, format!("unable to canonicalize payload: {error}"))
            })
        })
        .transpose()?
        .ok_or_else(|| {
            payload_selection_error(
                &config.archive_root,
                "no official libscrcpy_server payload was found".to_string(),
            )
        })
}

fn collect_payload_candidates(
    root: &Path,
    unix_payloads: &mut Vec<PathBuf>,
    fallback_payloads: &mut Vec<PathBuf>,
) -> HostResult<()> {
    let entries = fs::read_dir(root).map_err(|error| {
        payload_selection_error(
            root,
            format!("unable to read official payload archive: {error}"),
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            payload_selection_error(
                root,
                format!("unable to inspect official payload archive entry: {error}"),
            )
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_payload_candidates(&path, unix_payloads, fallback_payloads)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if is_unix_scrcpy_payload(file_name) {
            unix_payloads.push(path);
        } else if is_fallback_scrcpy_payload(file_name) {
            fallback_payloads.push(path);
        }
    }

    Ok(())
}

fn validate_payload_path(path: &Path, source: &str) -> HostResult<PathBuf> {
    if !path.is_file() {
        return Err(payload_selection_error(
            path,
            format!("{source} payload does not exist or is not a file"),
        ));
    }
    fs::canonicalize(path).map_err(|error| {
        payload_selection_error(
            path,
            format!("unable to canonicalize {source} payload: {error}"),
        )
    })
}

fn is_unix_scrcpy_payload(file_name: &str) -> bool {
    file_name.starts_with("libscrcpy_server_unix_") && file_name.ends_with(".z.so")
}

fn is_fallback_scrcpy_payload(file_name: &str) -> bool {
    file_name.starts_with("libscrcpy_server")
        && file_name.ends_with(".z.so")
        && !file_name.contains("_emulator")
}

fn scan_stale_processes<H: HdcBridge>(hdc: &H, device_id: &str) -> HostResult<Vec<StaleProcess>> {
    let output = hdc
        .exec_shell(device_id, "ps -ef")
        .map_err(|error| lifecycle_error(PHASE_STALE_SCAN, "ps -ef", error.to_string()))?;
    Ok(parse_stale_processes(&output))
}

fn kill_stale_processes<H: HdcBridge>(
    hdc: &H,
    device_id: &str,
    stale: &[StaleProcess],
) -> HostResult<()> {
    let pids = stale
        .iter()
        .map(|process| process.pid.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    hdc.exec_shell(device_id, &format!("kill -9 {pids}"))
        .map(|_| ())
        .map_err(|error| lifecycle_error(PHASE_STALE_KILL, &pids, error.to_string()))
}

fn cleanup_payload<H: HdcBridge>(
    hdc: &H,
    device_id: &str,
    config: &UitestLaunchConfig,
) -> HostResult<()> {
    hdc.exec_shell(
        device_id,
        &format!("rm -f {}", shell_quote(&config.remote_payload_path)),
    )
    .map(|_| ())
    .map_err(|error| {
        lifecycle_error(
            PHASE_CLEANUP,
            &config.remote_payload_path,
            error.to_string(),
        )
    })
}

fn launch_command(config: &UitestLaunchConfig) -> String {
    format!(
        "uitest start-daemon singleness --extension-name {} -scale 1 -frameRate {} -bitRate {} -p {} -screenId {} -encodeType 0 -iFrameInterval {} -repeatInterval {}",
        remote_payload_file_name(&config.remote_payload_path),
        DEFAULT_FRAME_RATE,
        DEFAULT_BIT_RATE,
        DEFAULT_RECORD_PORT_ARG,
        DEFAULT_SCREEN_ID,
        DEFAULT_IFRAME_INTERVAL_MS,
        DEFAULT_REPEAT_INTERVAL_MS,
    )
}

fn remote_payload_file_name(remote_payload_path: &str) -> &str {
    remote_payload_path
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(remote_payload_path)
}

fn resolve_local_grpc_port(request: &CompanionLaunchRequest) -> Option<u16> {
    if request.session_channel.transport != TransportKind::HdcForward {
        return None;
    }
    request
        .session_channel
        .target
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse().ok())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StaleProcess {
    pid: u32,
    line: String,
}

fn parse_stale_processes(output: &str) -> Vec<StaleProcess> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| is_route_owned_stale_process(line))
        .filter_map(|line| {
            parse_pid(line).map(|pid| StaleProcess {
                pid,
                line: line.to_string(),
            })
        })
        .collect()
}

fn is_route_owned_stale_process(line: &str) -> bool {
    line.contains("xdevice_scrcpy")
        || (line.contains("uitest")
            && line.contains("start-daemon")
            && line.contains("scrcpy_server.so"))
}

fn parse_pid(line: &str) -> Option<u32> {
    line.split_whitespace()
        .take(4)
        .find_map(|field| field.parse::<u32>().ok())
}

fn format_processes(processes: &[StaleProcess]) -> String {
    processes
        .iter()
        .map(|process| format!("pid={} line={}", process.pid, process.line))
        .collect::<Vec<_>>()
        .join("; ")
}

fn payload_selection_error(target: &Path, reason: String) -> HostError {
    HostError::PayloadSelection(format!(
        "route `{ROUTE}` phase `{PHASE_PAYLOAD_SELECTION}` target `{}`: {reason}",
        target.display()
    ))
}

fn lifecycle_error(
    phase: &'static str,
    command_target: impl AsRef<str>,
    reason: impl Into<String>,
) -> HostError {
    HostError::HdcFailure(format!(
        "route `{ROUTE}` phase `{phase}` target `{}`: {}",
        command_target.as_ref(),
        reason.into()
    ))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_stale_processes, select_official_payload, UitestCompanionManager, UitestLaunchConfig,
        UITEST_GRPC_SOCKET_NAME, UITEST_REMOTE_PAYLOAD_PATH,
    };
    use crate::companion::{CompanionLaunchRequest, CompanionManager};
    use crate::hdc::{HdcBridge, HdcForwardSpec};
    use crate::route::HostRoute;
    use crate::{HostError, HostResult};
    use hscrcpy_contracts::{ChannelEndpoint, TransportKind};
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum HdcCall {
        Shell(String),
        Push(PathBuf, String),
        Forward(HdcForwardSpec),
    }

    #[derive(Default)]
    struct RecordingHdcState {
        shell_outputs: VecDeque<HostResult<String>>,
        push_result: Option<HostResult<()>>,
        forward_result: Option<HostResult<()>>,
        calls: Vec<HdcCall>,
    }

    #[derive(Clone, Default)]
    struct RecordingHdc {
        state: Rc<RefCell<RecordingHdcState>>,
    }

    impl RecordingHdc {
        fn with_shell_outputs(outputs: Vec<HostResult<String>>) -> Self {
            Self {
                state: Rc::new(RefCell::new(RecordingHdcState {
                    shell_outputs: outputs.into(),
                    push_result: Some(Ok(())),
                    forward_result: Some(Ok(())),
                    calls: Vec::new(),
                })),
            }
        }

        fn calls(&self) -> Vec<HdcCall> {
            self.state.borrow().calls.clone()
        }

        fn fail_forward(self, reason: &str) -> Self {
            self.state.borrow_mut().forward_result =
                Some(Err(HostError::HdcFailure(reason.to_string())));
            self
        }

        fn fail_push(self, reason: &str) -> Self {
            self.state.borrow_mut().push_result =
                Some(Err(HostError::HdcFailure(reason.to_string())));
            self
        }
    }

    impl HdcBridge for RecordingHdc {
        fn ensure_device_visible(&self, _device_id: &str) -> HostResult<()> {
            Ok(())
        }

        fn forward(&self, _device_id: &str, spec: &HdcForwardSpec) -> HostResult<()> {
            let mut state = self.state.borrow_mut();
            state.calls.push(HdcCall::Forward(spec.clone()));
            state.forward_result.clone().unwrap_or(Ok(()))
        }

        fn forward_port(
            &self,
            _device_id: &str,
            _local_port: u16,
            _remote_port: u16,
        ) -> HostResult<()> {
            Ok(())
        }

        fn push_file(
            &self,
            _device_id: &str,
            local_path: &Path,
            remote_path: &str,
        ) -> HostResult<()> {
            let mut state = self.state.borrow_mut();
            state.calls.push(HdcCall::Push(
                local_path.to_path_buf(),
                remote_path.to_string(),
            ));
            state.push_result.clone().unwrap_or(Ok(()))
        }

        fn exec_shell(&self, _device_id: &str, command: &str) -> HostResult<String> {
            let mut state = self.state.borrow_mut();
            state.calls.push(HdcCall::Shell(command.to_string()));
            state
                .shell_outputs
                .pop_front()
                .expect("unexpected shell invocation")
        }
    }

    fn fixture_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "hscrcpy-uitest-fixture-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("fixture dir should be created");
        dir
    }

    fn touch(path: &Path) {
        fs::write(path, b"fixture").expect("fixture file should be written");
    }

    fn config_for_archive(archive_root: PathBuf) -> UitestLaunchConfig {
        UitestLaunchConfig {
            archive_root,
            ..UitestLaunchConfig::default()
        }
    }

    fn launch_request() -> CompanionLaunchRequest {
        CompanionLaunchRequest {
            route: HostRoute::Uitest,
            session_id: "session-1".to_string(),
            session_channel: ChannelEndpoint {
                transport: TransportKind::HdcForward,
                target: "127.0.0.1:27182".to_string(),
            },
            video_channel: ChannelEndpoint {
                transport: TransportKind::HdcForward,
                target: "127.0.0.1:27183".to_string(),
            },
        }
    }

    #[test]
    fn selector_prefers_latest_unix_scrcpy_payload() {
        let dir = fixture_dir();
        touch(&dir.join("libscrcpy_server1.z.so"));
        touch(&dir.join("libscrcpy_server_unix_6.3.1-20260113.z.so"));
        touch(&dir.join("libscrcpy_server_unix_6.5-20260313.z.so"));

        let selected = select_official_payload(&config_for_archive(dir.clone()))
            .expect("payload should be selected");

        assert_eq!(
            selected.file_name().and_then(|name| name.to_str()),
            Some("libscrcpy_server_unix_6.5-20260313.z.so")
        );
        fs::remove_dir_all(dir).expect("fixture dir should be removed");
    }

    #[test]
    fn selector_searches_payload_archive_recursively() {
        let dir = fixture_dir();
        let nested = dir.join("hosScrcpy/6.1.0.210/libscrcpy");
        fs::create_dir_all(&nested).expect("nested fixture dir should be created");
        touch(&nested.join("libscrcpy_server_unix_6.5-20260313.z.so"));

        let selected = select_official_payload(&config_for_archive(dir.clone()))
            .expect("nested payload should be selected");

        assert_eq!(
            selected.file_name().and_then(|name| name.to_str()),
            Some("libscrcpy_server_unix_6.5-20260313.z.so")
        );
        fs::remove_dir_all(dir).expect("fixture dir should be removed");
    }

    #[test]
    fn selector_fails_when_payload_archive_is_missing() {
        let dir = fixture_dir().join("missing");
        let err = select_official_payload(&config_for_archive(dir))
            .expect_err("missing archive should fail");
        assert!(matches!(err, HostError::PayloadSelection(_)));
        assert!(err.to_string().contains("payload-selection"));
    }

    #[test]
    fn parse_stale_processes_finds_xdevice_and_scrcpy_uitest_lines() {
        let output = "\
root 101 1 0 xdevice_scrcpy
root 202 1 0 uitest start-daemon singleness --extension-name scrcpy_server.so
root 303 1 0 uitest start-daemon singleness --extension-name uitest_agent.so
";

        let stale = parse_stale_processes(output);

        assert_eq!(
            stale.iter().map(|process| process.pid).collect::<Vec<_>>(),
            vec![101, 202]
        );
    }

    #[test]
    fn launch_kills_stale_process_pushes_forwards_and_cleans_payload() {
        let dir = fixture_dir();
        let payload = dir.join("libscrcpy_server_unix_6.5-20260313.z.so");
        touch(&payload);
        let hdc = RecordingHdc::with_shell_outputs(vec![
            Ok("root 101 1 0 xdevice_scrcpy\n".to_string()),
            Ok(String::new()),
            Ok(String::new()),
            Ok("uitest launched\n".to_string()),
            Ok(String::new()),
        ]);
        let manager = UitestCompanionManager::with_config(
            hdc.clone(),
            UitestLaunchConfig::with_payload_override(&payload),
        );

        manager
            .launch_session("device-1", &launch_request())
            .expect("launch lifecycle should succeed");

        let calls = hdc.calls();
        assert_eq!(calls[0], HdcCall::Shell("ps -ef".to_string()));
        assert_eq!(calls[1], HdcCall::Shell("kill -9 101".to_string()));
        assert_eq!(calls[2], HdcCall::Shell("ps -ef".to_string()));
        assert!(matches!(
            &calls[3],
            HdcCall::Push(_, remote) if remote == UITEST_REMOTE_PAYLOAD_PATH
        ));
        assert!(matches!(
            &calls[4],
            HdcCall::Shell(command)
                if command.contains("uitest start-daemon singleness")
                    && command.contains("--extension-name scrcpy_server.so")
        ));
        assert_eq!(
            calls[5],
            HdcCall::Forward(HdcForwardSpec::tcp_to_localabstract(
                27182,
                UITEST_GRPC_SOCKET_NAME
            ))
        );
        assert_eq!(
            calls[6],
            HdcCall::Shell(format!("rm -f '{UITEST_REMOTE_PAYLOAD_PATH}'"))
        );
        fs::remove_dir_all(dir).expect("fixture dir should be removed");
    }

    #[test]
    fn launch_fails_when_stale_process_remains_alive() {
        let dir = fixture_dir();
        let payload = dir.join("libscrcpy_server_unix_6.5-20260313.z.so");
        touch(&payload);
        let hdc = RecordingHdc::with_shell_outputs(vec![
            Ok("root 101 1 0 xdevice_scrcpy\n".to_string()),
            Ok(String::new()),
            Ok("root 101 1 0 xdevice_scrcpy\n".to_string()),
        ]);
        let manager = UitestCompanionManager::with_config(
            hdc.clone(),
            UitestLaunchConfig::with_payload_override(&payload),
        );

        let err = manager
            .launch_session("device-1", &launch_request())
            .expect_err("remaining stale process should fail launch");

        assert!(err.to_string().contains("stale-process-kill"));
        assert_eq!(hdc.calls().len(), 3);
        fs::remove_dir_all(dir).expect("fixture dir should be removed");
    }

    #[test]
    fn cleanup_runs_and_dominates_after_launch_failure() {
        let dir = fixture_dir();
        let payload = dir.join("libscrcpy_server_unix_6.5-20260313.z.so");
        touch(&payload);
        let hdc = RecordingHdc::with_shell_outputs(vec![
            Ok(String::new()),
            Err(HostError::HdcFailure("launch denied".to_string())),
            Err(HostError::HdcFailure("cleanup denied".to_string())),
        ]);
        let manager = UitestCompanionManager::with_config(
            hdc.clone(),
            UitestLaunchConfig::with_payload_override(&payload),
        );

        let err = manager
            .launch_session("device-1", &launch_request())
            .expect_err("cleanup failure after launch failure should fail startup");

        assert!(err.to_string().contains("cleanup"));
        assert!(err.to_string().contains("cleanup denied"));
        assert!(matches!(
            hdc.calls().last(),
            Some(HdcCall::Shell(command)) if command.starts_with("rm -f")
        ));
        fs::remove_dir_all(dir).expect("fixture dir should be removed");
    }

    #[test]
    fn missing_payload_fails_before_hdc_invocation() {
        let hdc = RecordingHdc::with_shell_outputs(Vec::new());
        let manager = UitestCompanionManager::with_config(
            hdc.clone(),
            UitestLaunchConfig::with_payload_override("/tmp/does-not-exist-scrcpy.so"),
        );

        let err = manager
            .launch_session("device-1", &launch_request())
            .expect_err("missing payload should fail before device launch");

        assert!(matches!(err, HostError::PayloadSelection(_)));
        assert!(hdc.calls().is_empty());
    }

    #[test]
    fn push_failure_reports_route_phase_target_and_device_output_then_cleans_payload() {
        let dir = fixture_dir();
        let payload = dir.join("libscrcpy_server_unix_6.5-20260313.z.so");
        touch(&payload);
        let hdc = RecordingHdc::with_shell_outputs(vec![
            Ok(String::new()),
            Ok("cleanup ok\n".to_string()),
        ])
        .fail_push(
            "operation `file send` failed, stderr=permission denied, stdout=Xpm check failed",
        );
        let manager = UitestCompanionManager::with_config(
            hdc.clone(),
            UitestLaunchConfig::with_payload_override(&payload),
        );

        let err = manager
            .launch_session("device-1", &launch_request())
            .expect_err("push failure should fail selected uitest route");

        let message = err.to_string();
        assert!(
            message.contains("route `uitest`"),
            "unexpected error: {message}"
        );
        assert!(
            message.contains("phase `payload-push`"),
            "unexpected error: {message}"
        );
        assert!(
            message.contains(&format!("target `{UITEST_REMOTE_PAYLOAD_PATH}`")),
            "unexpected error: {message}"
        );
        assert!(
            message.contains("permission denied"),
            "unexpected error: {message}"
        );
        assert!(
            message.contains("Xpm check failed"),
            "unexpected error: {message}"
        );
        assert!(matches!(
            hdc.calls().last(),
            Some(HdcCall::Shell(command)) if command == &format!("rm -f '{UITEST_REMOTE_PAYLOAD_PATH}'")
        ));
        assert!(
            hdc.calls().iter().all(|call| {
                !matches!(call, HdcCall::Shell(command) if command.contains("aa start"))
            }),
            "uitest push failure must not attempt the HAP launch route"
        );
        fs::remove_dir_all(dir).expect("fixture dir should be removed");
    }

    #[test]
    fn forward_failure_still_cleans_payload() {
        let dir = fixture_dir();
        let payload = dir.join("libscrcpy_server_unix_6.5-20260313.z.so");
        touch(&payload);
        let hdc = RecordingHdc::with_shell_outputs(vec![
            Ok(String::new()),
            Ok("uitest launched\n".to_string()),
            Ok(String::new()),
        ])
        .fail_forward("fport denied");
        let manager = UitestCompanionManager::with_config(
            hdc.clone(),
            UitestLaunchConfig::with_payload_override(&payload),
        );

        let err = manager
            .launch_session("device-1", &launch_request())
            .expect_err("forward failure should be surfaced");

        assert!(err.to_string().contains("forward"));
        assert!(matches!(
            hdc.calls().last(),
            Some(HdcCall::Shell(command)) if command.starts_with("rm -f")
        ));
        fs::remove_dir_all(dir).expect("fixture dir should be removed");
    }
}
