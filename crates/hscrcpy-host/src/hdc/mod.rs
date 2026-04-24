use crate::{HostError, HostResult};
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

const AUTO_DEVICE_ID: &str = "auto";

pub trait HdcBridge {
    fn ensure_device_visible(&self, device_id: &str) -> HostResult<()>;
    fn forward(&self, device_id: &str, spec: &HdcForwardSpec) -> HostResult<()> {
        match (&spec.local, &spec.remote) {
            (HdcForwardEndpoint::Tcp(local_port), HdcForwardEndpoint::Tcp(remote_port)) => {
                self.forward_port(device_id, *local_port, *remote_port)
            }
            _ => Err(HostError::ContractViolation(format!(
                "hdc bridge does not support forwarding {} to {}",
                spec.local, spec.remote
            ))),
        }
    }
    fn forward_port(&self, device_id: &str, local_port: u16, remote_port: u16) -> HostResult<()>;
    fn remove_forward(&self, _device_id: &str, _spec: &HdcForwardSpec) -> HostResult<()> {
        Ok(())
    }
    fn push_file(&self, _device_id: &str, local_path: &Path, remote_path: &str) -> HostResult<()> {
        Err(HostError::ContractViolation(format!(
            "hdc bridge does not support file push from {} to {remote_path}",
            local_path.display()
        )))
    }
    fn exec_shell(&self, device_id: &str, command: &str) -> HostResult<String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdcForwardSpec {
    pub local: HdcForwardEndpoint,
    pub remote: HdcForwardEndpoint,
}

impl HdcForwardSpec {
    pub const fn tcp_to_tcp(local_port: u16, remote_port: u16) -> Self {
        Self {
            local: HdcForwardEndpoint::Tcp(local_port),
            remote: HdcForwardEndpoint::Tcp(remote_port),
        }
    }

    pub fn tcp_to_localabstract(local_port: u16, socket_name: impl Into<String>) -> Self {
        Self {
            local: HdcForwardEndpoint::Tcp(local_port),
            remote: HdcForwardEndpoint::LocalAbstract(socket_name.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HdcForwardEndpoint {
    Tcp(u16),
    LocalAbstract(String),
}

impl HdcForwardEndpoint {
    fn to_hdc_arg(&self) -> HostResult<String> {
        match self {
            Self::Tcp(port) => {
                if *port == 0 {
                    return Err(HostError::ContractViolation(
                        "hdc tcp forwarding endpoint requires a non-zero port".to_string(),
                    ));
                }
                Ok(format!("tcp:{port}"))
            }
            Self::LocalAbstract(socket_name) => {
                let socket_name = socket_name.trim();
                if socket_name.is_empty() {
                    return Err(HostError::ContractViolation(
                        "hdc localabstract forwarding endpoint requires a socket name".to_string(),
                    ));
                }
                Ok(format!("localabstract:{socket_name}"))
            }
        }
    }
}

impl std::fmt::Display for HdcForwardEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tcp(port) => write!(f, "tcp:{port}"),
            Self::LocalAbstract(socket_name) => write!(f, "localabstract:{socket_name}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandOutput {
    status_code: Option<i32>,
    stdout: String,
    stderr: String,
}

trait CommandRunner {
    fn run(&self, binary: &str, args: &[String]) -> HostResult<CommandOutput>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, binary: &str, args: &[String]) -> HostResult<CommandOutput> {
        let output = Command::new(binary).args(args).output().map_err(|error| {
            HostError::HdcFailure(format!(
                "failed to execute command `{binary} {}`: {error}",
                args.join(" ")
            ))
        })?;

        Ok(CommandOutput {
            status_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

pub struct RuntimeHdcBridge {
    binary: String,
    runner: Box<dyn CommandRunner>,
    auto_resolved_target: Mutex<Option<String>>,
}

impl RuntimeHdcBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_binary(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
            runner: Box::new(SystemCommandRunner),
            auto_resolved_target: Mutex::new(None),
        }
    }

    #[cfg(test)]
    fn with_runner(binary: impl Into<String>, runner: impl CommandRunner + 'static) -> Self {
        Self {
            binary: binary.into(),
            runner: Box::new(runner),
            auto_resolved_target: Mutex::new(None),
        }
    }

    fn run_checked(
        &self,
        operation: &str,
        device_id: Option<&str>,
        args: &[String],
    ) -> HostResult<CommandOutput> {
        let output = self.runner.run(&self.binary, args)?;
        if output.status_code == Some(0) {
            return Ok(output);
        }

        let mut reason = format!(
            "operation `{operation}` failed with {}",
            match output.status_code {
                Some(code) => format!("exit code {code}"),
                None => "termination by signal".to_string(),
            }
        );
        if let Some(device_id) = device_id {
            reason.push_str(&format!(", device_id={device_id}"));
        }

        let stderr = output.stderr.trim();
        if !stderr.is_empty() {
            reason.push_str(&format!(", stderr={stderr}"));
        }
        let stdout = output.stdout.trim();
        if !stdout.is_empty() {
            reason.push_str(&format!(", stdout={stdout}"));
        }

        Err(HostError::HdcFailure(reason))
    }

    fn discover_visible_targets(&self) -> HostResult<Vec<String>> {
        let args = vec!["list".to_string(), "targets".to_string()];
        let output = self.run_checked("list targets", None, &args)?;
        Ok(parse_visible_targets(&output.stdout))
    }

    fn resolve_device_target<'a>(&self, device_id: &'a str) -> HostResult<String> {
        if device_id != AUTO_DEVICE_ID {
            return Ok(device_id.to_string());
        }

        let mut cache = self
            .auto_resolved_target
            .lock()
            .expect("auto target cache poisoned");
        if let Some(target) = cache.as_ref() {
            return Ok(target.clone());
        }

        let visible_targets = self.discover_visible_targets()?;
        if visible_targets.is_empty() {
            return Err(HostError::CompanionUnavailable(
                "no hdc targets are currently visible".to_string(),
            ));
        }

        let selected = visible_targets[0].clone();
        *cache = Some(selected.clone());
        Ok(selected)
    }
}

impl Default for RuntimeHdcBridge {
    fn default() -> Self {
        Self::with_binary("hdc")
    }
}

impl HdcBridge for RuntimeHdcBridge {
    fn ensure_device_visible(&self, device_id: &str) -> HostResult<()> {
        let device_id = validate_device_id(device_id)?;
        let visible_targets = self.discover_visible_targets()?;

        if visible_targets.is_empty() {
            return Err(HostError::CompanionUnavailable(
                "no hdc targets are currently visible".to_string(),
            ));
        }

        if device_id == AUTO_DEVICE_ID {
            let mut cache = self
                .auto_resolved_target
                .lock()
                .expect("auto target cache poisoned");
            *cache = Some(visible_targets[0].clone());
            return Ok(());
        }

        if visible_targets.iter().any(|target| target == device_id) {
            return Ok(());
        }

        Err(HostError::CompanionUnavailable(format!(
            "target `{device_id}` is not visible; discovered targets: {}",
            visible_targets.join(", ")
        )))
    }

    fn forward(&self, device_id: &str, spec: &HdcForwardSpec) -> HostResult<()> {
        let device_id = validate_device_id(device_id)?;
        if !matches!(spec.local, HdcForwardEndpoint::Tcp(_)) {
            return Err(HostError::ContractViolation(format!(
                "hdc fport requires a local tcp endpoint, got {}",
                spec.local
            )));
        }

        let local = spec.local.to_hdc_arg()?;
        let remote = spec.remote.to_hdc_arg()?;
        let resolved_target = self.resolve_device_target(device_id)?;
        let args = vec![
            "-t".to_string(),
            resolved_target.clone(),
            "fport".to_string(),
            local,
            remote,
        ];
        self.run_checked("forward endpoint", Some(&resolved_target), &args)?;
        Ok(())
    }

    fn forward_port(&self, device_id: &str, local_port: u16, remote_port: u16) -> HostResult<()> {
        self.forward(
            device_id,
            &HdcForwardSpec::tcp_to_tcp(local_port, remote_port),
        )
    }

    fn remove_forward(&self, device_id: &str, spec: &HdcForwardSpec) -> HostResult<()> {
        let device_id = validate_device_id(device_id)?;
        if !matches!(spec.local, HdcForwardEndpoint::Tcp(_)) {
            return Err(HostError::ContractViolation(format!(
                "hdc fport rm requires a local tcp endpoint, got {}",
                spec.local
            )));
        }

        let local = spec.local.to_hdc_arg()?;
        let remote = spec.remote.to_hdc_arg()?;
        let resolved_target = self.resolve_device_target(device_id)?;
        let args = vec![
            "-t".to_string(),
            resolved_target.clone(),
            "fport".to_string(),
            "rm".to_string(),
            local,
            remote,
        ];
        self.run_checked("remove forward endpoint", Some(&resolved_target), &args)?;
        Ok(())
    }

    fn push_file(&self, device_id: &str, local_path: &Path, remote_path: &str) -> HostResult<()> {
        let device_id = validate_device_id(device_id)?;
        let remote_path = remote_path.trim();
        if remote_path.is_empty() {
            return Err(HostError::ContractViolation(
                "hdc file push requires a non-empty remote path".to_string(),
            ));
        }

        let resolved_target = self.resolve_device_target(device_id)?;
        let args = vec![
            "-t".to_string(),
            resolved_target.clone(),
            "file".to_string(),
            "send".to_string(),
            local_path.display().to_string(),
            remote_path.to_string(),
        ];
        self.run_checked("file send", Some(&resolved_target), &args)?;
        Ok(())
    }

    fn exec_shell(&self, device_id: &str, command: &str) -> HostResult<String> {
        let device_id = validate_device_id(device_id)?;
        let command = command.trim();
        if command.is_empty() {
            return Err(HostError::ContractViolation(
                "shell command must not be empty".to_string(),
            ));
        }

        let resolved_target = self.resolve_device_target(device_id)?;
        let args = vec![
            "-t".to_string(),
            resolved_target.clone(),
            "shell".to_string(),
            command.to_string(),
        ];
        let output = self.run_checked("exec shell", Some(&resolved_target), &args)?;
        Ok(output
            .stdout
            .trim_end_matches(|ch| ch == '\r' || ch == '\n')
            .to_string())
    }
}

fn validate_device_id(device_id: &str) -> HostResult<&str> {
    let trimmed = device_id.trim();
    if trimmed.is_empty() {
        return Err(HostError::ContractViolation(
            "device_id must not be empty".to_string(),
        ));
    }
    Ok(trimmed)
}

fn parse_visible_targets(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_empty_target_marker(line))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

fn is_empty_target_marker(line: &str) -> bool {
    let normalized = line.trim().to_ascii_lowercase();
    normalized == "[empty]" || normalized == "empty"
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StubHdcBridge;

impl HdcBridge for StubHdcBridge {
    fn ensure_device_visible(&self, device_id: &str) -> HostResult<()> {
        RuntimeHdcBridge::default().ensure_device_visible(device_id)
    }

    fn forward_port(&self, device_id: &str, local_port: u16, remote_port: u16) -> HostResult<()> {
        RuntimeHdcBridge::default().forward_port(device_id, local_port, remote_port)
    }

    fn push_file(&self, device_id: &str, local_path: &Path, remote_path: &str) -> HostResult<()> {
        RuntimeHdcBridge::default().push_file(device_id, local_path, remote_path)
    }

    fn exec_shell(&self, device_id: &str, command: &str) -> HostResult<String> {
        RuntimeHdcBridge::default().exec_shell(device_id, command)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandOutput, CommandRunner, HdcBridge, HdcForwardSpec, HostError, HostResult,
        RuntimeHdcBridge,
    };
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::path::Path;
    use std::rc::Rc;

    #[derive(Default)]
    struct MockState {
        scripted_results: VecDeque<HostResult<CommandOutput>>,
        invocations: Vec<(String, Vec<String>)>,
    }

    #[derive(Clone, Default)]
    struct MockRunner {
        state: Rc<RefCell<MockState>>,
    }

    impl MockRunner {
        fn with_scripted_results(scripted_results: Vec<HostResult<CommandOutput>>) -> Self {
            Self {
                state: Rc::new(RefCell::new(MockState {
                    scripted_results: scripted_results.into(),
                    invocations: Vec::new(),
                })),
            }
        }

        fn invocations(&self) -> Vec<(String, Vec<String>)> {
            self.state.borrow().invocations.clone()
        }
    }

    impl CommandRunner for MockRunner {
        fn run(&self, binary: &str, args: &[String]) -> HostResult<CommandOutput> {
            let mut state = self.state.borrow_mut();
            state.invocations.push((binary.to_string(), args.to_vec()));
            state
                .scripted_results
                .pop_front()
                .expect("unexpected hdc invocation in test")
        }
    }

    fn success(stdout: &str) -> HostResult<CommandOutput> {
        Ok(CommandOutput {
            status_code: Some(0),
            stdout: stdout.to_string(),
            stderr: String::new(),
        })
    }

    fn failure(status_code: i32, stderr: &str) -> HostResult<CommandOutput> {
        Ok(CommandOutput {
            status_code: Some(status_code),
            stdout: String::new(),
            stderr: stderr.to_string(),
        })
    }

    #[test]
    fn ensure_device_visible_accepts_explicit_target() {
        let runner = MockRunner::with_scripted_results(vec![success("SERIAL_A\nSERIAL_B\n")]);
        let bridge = RuntimeHdcBridge::with_runner("hdc-test", runner.clone());

        bridge
            .ensure_device_visible("SERIAL_B")
            .expect("target should be visible");

        assert_eq!(
            runner.invocations(),
            vec![(
                "hdc-test".to_string(),
                vec!["list".to_string(), "targets".to_string()],
            )]
        );
    }

    #[test]
    fn ensure_device_visible_accepts_auto_with_any_target() {
        let runner = MockRunner::with_scripted_results(vec![success("SERIAL_A device\n")]);
        let bridge = RuntimeHdcBridge::with_runner("hdc-test", runner.clone());

        bridge
            .ensure_device_visible("auto")
            .expect("auto should accept any visible target");

        assert_eq!(
            runner.invocations(),
            vec![(
                "hdc-test".to_string(),
                vec!["list".to_string(), "targets".to_string()],
            )]
        );
    }

    #[test]
    fn ensure_device_visible_rejects_missing_target() {
        let runner = MockRunner::with_scripted_results(vec![success("SERIAL_A\nSERIAL_B\n")]);
        let bridge = RuntimeHdcBridge::with_runner("hdc-test", runner);

        let err = bridge
            .ensure_device_visible("SERIAL_C")
            .expect_err("unknown target should fail");
        assert!(
            err.to_string().contains("SERIAL_C"),
            "missing target should be in error: {err}"
        );
    }

    #[test]
    fn ensure_device_visible_rejects_empty_target_list() {
        let runner = MockRunner::with_scripted_results(vec![success("[Empty]\n")]);
        let bridge = RuntimeHdcBridge::with_runner("hdc-test", runner);

        let err = bridge
            .ensure_device_visible("auto")
            .expect_err("empty target output should fail");
        assert!(
            matches!(err, HostError::CompanionUnavailable(_)),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn forward_port_uses_device_targeted_fport_command() {
        let runner = MockRunner::with_scripted_results(vec![success("")]);
        let bridge = RuntimeHdcBridge::with_runner("hdc-test", runner.clone());

        bridge
            .forward_port("SERIAL_A", 27182, 27183)
            .expect("forwarding should succeed");

        assert_eq!(
            runner.invocations(),
            vec![(
                "hdc-test".to_string(),
                vec![
                    "-t".to_string(),
                    "SERIAL_A".to_string(),
                    "fport".to_string(),
                    "tcp:27182".to_string(),
                    "tcp:27183".to_string(),
                ],
            )]
        );
    }

    #[test]
    fn forward_supports_localabstract_remote_endpoint() {
        let runner = MockRunner::with_scripted_results(vec![success("")]);
        let bridge = RuntimeHdcBridge::with_runner("hdc-test", runner.clone());

        bridge
            .forward(
                "SERIAL_A",
                &HdcForwardSpec::tcp_to_localabstract(27182, "scrcpy_grpc_socket"),
            )
            .expect("localabstract forwarding should succeed");

        assert_eq!(
            runner.invocations(),
            vec![(
                "hdc-test".to_string(),
                vec![
                    "-t".to_string(),
                    "SERIAL_A".to_string(),
                    "fport".to_string(),
                    "tcp:27182".to_string(),
                    "localabstract:scrcpy_grpc_socket".to_string(),
                ],
            )]
        );
    }

    #[test]
    fn push_file_uses_hdc_file_send() {
        let runner = MockRunner::with_scripted_results(vec![success("")]);
        let bridge = RuntimeHdcBridge::with_runner("hdc-test", runner.clone());

        bridge
            .push_file(
                "SERIAL_A",
                Path::new("third_party/payload.so"),
                "/data/local/tmp/scrcpy_server.so",
            )
            .expect("file send should succeed");

        assert_eq!(
            runner.invocations(),
            vec![(
                "hdc-test".to_string(),
                vec![
                    "-t".to_string(),
                    "SERIAL_A".to_string(),
                    "file".to_string(),
                    "send".to_string(),
                    "third_party/payload.so".to_string(),
                    "/data/local/tmp/scrcpy_server.so".to_string(),
                ],
            )]
        );
    }

    #[test]
    fn exec_shell_uses_device_targeted_shell_command() {
        let runner = MockRunner::with_scripted_results(vec![success("ok\n")]);
        let bridge = RuntimeHdcBridge::with_runner("hdc-test", runner.clone());

        let output = bridge
            .exec_shell("SERIAL_A", "param get ro.product.model")
            .expect("shell command should succeed");
        assert_eq!(output, "ok".to_string());
        assert_eq!(
            runner.invocations(),
            vec![(
                "hdc-test".to_string(),
                vec![
                    "-t".to_string(),
                    "SERIAL_A".to_string(),
                    "shell".to_string(),
                    "param get ro.product.model".to_string(),
                ],
            )]
        );
    }

    #[test]
    fn auto_target_resolution_is_reused_for_forward_and_shell() {
        let runner = MockRunner::with_scripted_results(vec![
            success("SERIAL_A device\n"),
            success(""),
            success("ok\n"),
        ]);
        let bridge = RuntimeHdcBridge::with_runner("hdc-test", runner.clone());

        bridge
            .ensure_device_visible("auto")
            .expect("auto should resolve during discovery");
        bridge
            .forward_port("auto", 27182, 27183)
            .expect("forwarding should use resolved target");
        let output = bridge
            .exec_shell("auto", "param get ro.product.model")
            .expect("shell should use resolved target");
        assert_eq!(output, "ok".to_string());

        assert_eq!(
            runner.invocations(),
            vec![
                (
                    "hdc-test".to_string(),
                    vec!["list".to_string(), "targets".to_string()],
                ),
                (
                    "hdc-test".to_string(),
                    vec![
                        "-t".to_string(),
                        "SERIAL_A".to_string(),
                        "fport".to_string(),
                        "tcp:27182".to_string(),
                        "tcp:27183".to_string(),
                    ],
                ),
                (
                    "hdc-test".to_string(),
                    vec![
                        "-t".to_string(),
                        "SERIAL_A".to_string(),
                        "shell".to_string(),
                        "param get ro.product.model".to_string(),
                    ],
                ),
            ]
        );
    }

    #[test]
    fn forward_port_surfaces_command_failure_context() {
        let runner = MockRunner::with_scripted_results(vec![failure(1, "permission denied")]);
        let bridge = RuntimeHdcBridge::with_runner("hdc-test", runner);

        let err = bridge
            .forward_port("SERIAL_A", 27182, 27183)
            .expect_err("forwarding failure should be surfaced");
        assert!(
            err.to_string().contains("permission denied"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_invalid_inputs_before_invoking_hdc() {
        let runner = MockRunner::with_scripted_results(vec![]);
        let bridge = RuntimeHdcBridge::with_runner("hdc-test", runner);

        let err = bridge
            .exec_shell("SERIAL_A", "   ")
            .expect_err("empty command must fail");
        assert!(
            matches!(err, HostError::ContractViolation(_)),
            "unexpected error: {err}"
        );
    }
}
