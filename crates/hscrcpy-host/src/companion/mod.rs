use crate::hdc::HdcBridge;
use crate::{HostError, HostResult};
use hscrcpy_contracts::{ChannelEndpoint, PROTOCOL_MAJOR_MVP, PROTOCOL_MINOR_MVP};

const BUNDLE_INSPECT_NOT_INSTALLED_MARKERS: [&str; 4] = [
    "not found",
    "not installed",
    "bundle does not exist",
    "no bundle info",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionDeployState {
    Missing,
    Ready,
    UpgradeRequired,
    ProtocolIncompatible,
    CompanionTooNew,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionAction {
    Install,
    Update,
    Reinstall,
    Skip,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionPlan {
    pub action: CompanionAction,
    pub target_version: String,
    pub artifact_hint: String,
}

impl CompanionPlan {
    pub fn deploy_state(&self) -> CompanionDeployState {
        match self.action {
            CompanionAction::Install => CompanionDeployState::Missing,
            CompanionAction::Skip => CompanionDeployState::Ready,
            CompanionAction::Update => CompanionDeployState::UpgradeRequired,
            CompanionAction::Reinstall => CompanionDeployState::ProtocolIncompatible,
            CompanionAction::Block => CompanionDeployState::CompanionTooNew,
        }
    }
}

pub trait CompanionManager {
    fn plan(&self, device_id: &str) -> HostResult<CompanionPlan>;
    fn apply(&self, device_id: &str, plan: &CompanionPlan) -> HostResult<()>;
    fn launch_session(&self, device_id: &str, request: &CompanionLaunchRequest) -> HostResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledCompanionManifest {
    pub companion_id: String,
    pub artifact_path: String,
    pub version_name: String,
    pub version_code: u64,
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub sha256: String,
    pub supported_abis: Vec<String>,
    pub launch_ability: String,
}

impl BundledCompanionManifest {
    pub fn mvp() -> Self {
        Self {
            companion_id: "cn.magicdian.hscrcpy.server".to_string(),
            artifact_path: "assets/companion/hscrcpy_server.hap".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1_000_000,
            protocol_major: PROTOCOL_MAJOR_MVP,
            protocol_minor: PROTOCOL_MINOR_MVP,
            sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            supported_abis: Vec::new(),
            launch_ability: "EntryAbility".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionLaunchRequest {
    pub session_id: String,
    pub session_channel: ChannelEndpoint,
    pub video_channel: ChannelEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCompanionMetadata {
    pub companion_id: String,
    pub version_name: String,
    pub version_code: u64,
    pub protocol_major: u16,
    pub protocol_minor: u16,
}

pub trait CompanionDeployRuntime {
    fn inspect_installed(
        &self,
        device_id: &str,
        companion_id: &str,
    ) -> HostResult<Option<InstalledCompanionMetadata>>;
    fn install(&self, device_id: &str, manifest: &BundledCompanionManifest) -> HostResult<()>;
    fn uninstall(&self, device_id: &str, companion_id: &str) -> HostResult<()>;
    fn launch_session(
        &self,
        device_id: &str,
        manifest: &BundledCompanionManifest,
        request: &CompanionLaunchRequest,
    ) -> HostResult<()>;
}

pub struct BundledCompanionManager<R> {
    manifest: BundledCompanionManifest,
    runtime: R,
}

impl<R> BundledCompanionManager<R> {
    pub fn new(manifest: BundledCompanionManifest, runtime: R) -> Self {
        Self { manifest, runtime }
    }
}

impl<R> CompanionManager for BundledCompanionManager<R>
where
    R: CompanionDeployRuntime,
{
    fn plan(&self, device_id: &str) -> HostResult<CompanionPlan> {
        validate_bundled_manifest(&self.manifest)?;
        let installed = self
            .runtime
            .inspect_installed(device_id, &self.manifest.companion_id)?;
        let deploy_state = resolve_deploy_state(&self.manifest, installed.as_ref());
        Ok(CompanionPlan {
            action: action_for_state(deploy_state),
            target_version: self.manifest.version_name.clone(),
            artifact_hint: self.manifest.artifact_path.clone(),
        })
    }

    fn apply(&self, device_id: &str, plan: &CompanionPlan) -> HostResult<()> {
        match plan.action {
            CompanionAction::Skip => Ok(()),
            CompanionAction::Install | CompanionAction::Update => {
                self.runtime.install(device_id, &self.manifest)
            }
            CompanionAction::Reinstall => {
                self.runtime
                    .uninstall(device_id, &self.manifest.companion_id)?;
                self.runtime.install(device_id, &self.manifest)
            }
            CompanionAction::Block => Err(HostError::ContractViolation(
                "companion_too_new: device companion is newer than host-compatible bundled build"
                    .to_string(),
            )),
        }
    }

    fn launch_session(&self, device_id: &str, request: &CompanionLaunchRequest) -> HostResult<()> {
        self.runtime
            .launch_session(device_id, &self.manifest, request)
    }
}

fn action_for_state(state: CompanionDeployState) -> CompanionAction {
    match state {
        CompanionDeployState::Missing => CompanionAction::Install,
        CompanionDeployState::Ready => CompanionAction::Skip,
        CompanionDeployState::UpgradeRequired => CompanionAction::Update,
        CompanionDeployState::ProtocolIncompatible => CompanionAction::Reinstall,
        CompanionDeployState::CompanionTooNew => CompanionAction::Block,
    }
}

fn validate_bundled_manifest(manifest: &BundledCompanionManifest) -> HostResult<()> {
    if manifest.protocol_major != PROTOCOL_MAJOR_MVP {
        return Err(HostError::ContractViolation(format!(
            "bundled companion protocol_major {} does not match host {}",
            manifest.protocol_major, PROTOCOL_MAJOR_MVP
        )));
    }
    if manifest.protocol_minor < PROTOCOL_MINOR_MVP {
        return Err(HostError::ContractViolation(format!(
            "bundled companion protocol_minor {} is older than host floor {}",
            manifest.protocol_minor, PROTOCOL_MINOR_MVP
        )));
    }
    Ok(())
}

fn resolve_deploy_state(
    manifest: &BundledCompanionManifest,
    installed: Option<&InstalledCompanionMetadata>,
) -> CompanionDeployState {
    let Some(installed) = installed else {
        return CompanionDeployState::Missing;
    };

    if installed.protocol_major > manifest.protocol_major {
        return CompanionDeployState::CompanionTooNew;
    }
    if installed.protocol_major < manifest.protocol_major {
        return CompanionDeployState::ProtocolIncompatible;
    }

    if installed.version_code > manifest.version_code {
        return CompanionDeployState::CompanionTooNew;
    }
    if installed.version_code < manifest.version_code {
        return CompanionDeployState::UpgradeRequired;
    }

    if installed.protocol_minor > manifest.protocol_minor {
        return CompanionDeployState::CompanionTooNew;
    }
    if installed.protocol_minor < manifest.protocol_minor {
        return CompanionDeployState::UpgradeRequired;
    }

    CompanionDeployState::Ready
}

pub struct HdcCompanionRuntime<H> {
    hdc: H,
}

impl<H> HdcCompanionRuntime<H> {
    pub fn new(hdc: H) -> Self {
        Self { hdc }
    }
}

impl<H> CompanionDeployRuntime for HdcCompanionRuntime<H>
where
    H: HdcBridge,
{
    fn inspect_installed(
        &self,
        device_id: &str,
        companion_id: &str,
    ) -> HostResult<Option<InstalledCompanionMetadata>> {
        let command = format!("bm dump -n {}", shell_quote(companion_id));
        let output = self.hdc.exec_shell(device_id, &command)?;
        parse_bm_dump_output(companion_id, &output)
    }

    fn install(&self, device_id: &str, manifest: &BundledCompanionManifest) -> HostResult<()> {
        // Artifact staging/transfer is delegated to the HDC layer behind exec_shell.
        let command = format!("bm install -p {}", shell_quote(&manifest.artifact_path));
        self.hdc.exec_shell(device_id, &command)?;
        Ok(())
    }

    fn uninstall(&self, device_id: &str, companion_id: &str) -> HostResult<()> {
        let command = format!("bm uninstall -n {}", shell_quote(companion_id));
        self.hdc.exec_shell(device_id, &command)?;
        Ok(())
    }

    fn launch_session(
        &self,
        device_id: &str,
        manifest: &BundledCompanionManifest,
        request: &CompanionLaunchRequest,
    ) -> HostResult<()> {
        if request.session_id.trim().is_empty() {
            return Err(HostError::ContractViolation(
                "session launch requires a non-empty session_id".to_string(),
            ));
        }

        // The exact HarmonyOS launch verb is still provisional, so the host only
        // owns bundle/ability activation here and leaves port/session arguments
        // as a later extension point behind this abstraction.
        let command = format!(
            "aa start -b {} -a {}",
            shell_quote(&manifest.companion_id),
            shell_quote(&manifest.launch_ability)
        );
        self.hdc.exec_shell(device_id, &command)?;
        Ok(())
    }
}

pub struct HdcCompanionManager<H>
where
    H: HdcBridge,
{
    inner: BundledCompanionManager<HdcCompanionRuntime<H>>,
}

impl<H> HdcCompanionManager<H>
where
    H: HdcBridge,
{
    pub fn new(hdc: H) -> Self {
        Self {
            inner: BundledCompanionManager::new(
                BundledCompanionManifest::mvp(),
                HdcCompanionRuntime::new(hdc),
            ),
        }
    }
}

impl<H> CompanionManager for HdcCompanionManager<H>
where
    H: HdcBridge,
{
    fn plan(&self, device_id: &str) -> HostResult<CompanionPlan> {
        self.inner.plan(device_id)
    }

    fn apply(&self, device_id: &str, plan: &CompanionPlan) -> HostResult<()> {
        self.inner.apply(device_id, plan)
    }

    fn launch_session(&self, device_id: &str, request: &CompanionLaunchRequest) -> HostResult<()> {
        self.inner.launch_session(device_id, request)
    }
}

struct NoopCompanionRuntime;

impl CompanionDeployRuntime for NoopCompanionRuntime {
    fn inspect_installed(
        &self,
        _device_id: &str,
        _companion_id: &str,
    ) -> HostResult<Option<InstalledCompanionMetadata>> {
        Ok(None)
    }

    fn install(&self, _device_id: &str, _manifest: &BundledCompanionManifest) -> HostResult<()> {
        Ok(())
    }

    fn uninstall(&self, _device_id: &str, _companion_id: &str) -> HostResult<()> {
        Ok(())
    }

    fn launch_session(
        &self,
        _device_id: &str,
        _manifest: &BundledCompanionManifest,
        _request: &CompanionLaunchRequest,
    ) -> HostResult<()> {
        Ok(())
    }
}

pub struct StubCompanionManager;

impl CompanionManager for StubCompanionManager {
    fn plan(&self, device_id: &str) -> HostResult<CompanionPlan> {
        let manager =
            BundledCompanionManager::new(BundledCompanionManifest::mvp(), NoopCompanionRuntime);
        manager.plan(device_id)
    }

    fn apply(&self, device_id: &str, plan: &CompanionPlan) -> HostResult<()> {
        let manager =
            BundledCompanionManager::new(BundledCompanionManifest::mvp(), NoopCompanionRuntime);
        manager.apply(device_id, plan)
    }

    fn launch_session(&self, device_id: &str, request: &CompanionLaunchRequest) -> HostResult<()> {
        let manager =
            BundledCompanionManager::new(BundledCompanionManifest::mvp(), NoopCompanionRuntime);
        manager.launch_session(device_id, request)
    }
}

fn parse_bm_dump_output(
    companion_id: &str,
    output: &str,
) -> HostResult<Option<InstalledCompanionMetadata>> {
    let normalized = output.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || BUNDLE_INSPECT_NOT_INSTALLED_MARKERS
            .iter()
            .any(|marker| normalized.contains(marker))
    {
        return Ok(None);
    }

    let version_code =
        extract_top_level_u64(output, &["versionCode", "version_code"]).ok_or_else(|| {
            HostError::CompanionUnavailable(
                "unable to parse installed companion version_code from `bm dump`".to_string(),
            )
        })?;
    let version_name = extract_top_level_string(output, &["versionName", "version_name"])
        .or_else(|| extract_string(output, &["versionName", "version_name"]))
        .unwrap_or_else(|| "unknown".to_string());
    let protocol_major = extract_top_level_u16(output, &["protocolMajor", "protocol_major"])
        .or_else(|| extract_u16(output, &["protocolMajor", "protocol_major"]))
        .unwrap_or(PROTOCOL_MAJOR_MVP);
    let protocol_minor = extract_top_level_u16(output, &["protocolMinor", "protocol_minor"])
        .or_else(|| extract_u16(output, &["protocolMinor", "protocol_minor"]))
        .unwrap_or(PROTOCOL_MINOR_MVP);

    Ok(Some(InstalledCompanionMetadata {
        companion_id: companion_id.to_string(),
        version_name,
        version_code,
        protocol_major,
        protocol_minor,
    }))
}

fn extract_top_level_u64(output: &str, keys: &[&str]) -> Option<u64> {
    extract_top_level_scalar(output, keys).and_then(|value| value.parse().ok())
}

fn extract_top_level_u16(output: &str, keys: &[&str]) -> Option<u16> {
    extract_top_level_scalar(output, keys).and_then(|value| value.parse().ok())
}

fn extract_top_level_string(output: &str, keys: &[&str]) -> Option<String> {
    extract_top_level_scalar(output, keys).map(ToString::to_string)
}

fn extract_top_level_scalar<'a>(output: &'a str, keys: &[&str]) -> Option<&'a str> {
    let mut depth = 0_i32;

    for line in output.lines() {
        let trimmed = line.trim();
        if depth == 1 {
            for key in keys {
                if let Some(value) = extract_value_from_line(trimmed, key) {
                    return Some(value);
                }
            }
        }

        depth += structural_delta(trimmed);
    }
    None
}

fn structural_delta(line: &str) -> i32 {
    let opens = line.chars().filter(|ch| matches!(ch, '{' | '[')).count() as i32;
    let closes = line.chars().filter(|ch| matches!(ch, '}' | ']')).count() as i32;
    opens - closes
}

fn extract_u16(output: &str, keys: &[&str]) -> Option<u16> {
    extract_scalar(output, keys).and_then(|value| value.parse().ok())
}

fn extract_string(output: &str, keys: &[&str]) -> Option<String> {
    extract_scalar(output, keys).map(ToString::to_string)
}

fn extract_scalar<'a>(output: &'a str, keys: &[&str]) -> Option<&'a str> {
    for line in output.lines() {
        let trimmed = line.trim();
        for key in keys {
            if let Some(value) = extract_value_from_line(trimmed, key) {
                return Some(value);
            }
        }
    }
    None
}

fn extract_value_from_line<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    if !line.contains(key) {
        return None;
    }

    let key_position = line.find(key)?;
    let key_end = key_position + key.len();
    let tail = line.get(key_end..)?.trim_start_matches([' ', '"', '\'']);
    let tail = tail.strip_prefix(':').or_else(|| tail.strip_prefix('='))?;
    let value = tail.trim();
    let value = value.trim_matches([',', '"', '\'', ' ', '}']);
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_bm_dump_output, BundledCompanionManager, BundledCompanionManifest, CompanionAction,
        CompanionDeployRuntime, CompanionDeployState, CompanionLaunchRequest, CompanionManager,
        CompanionPlan, InstalledCompanionMetadata,
    };
    use crate::{HostError, HostResult};
    use std::cell::RefCell;

    #[derive(Default)]
    struct TestRuntime {
        installed: Option<InstalledCompanionMetadata>,
        actions: RefCell<Vec<String>>,
    }

    impl TestRuntime {
        fn with_installed(installed: InstalledCompanionMetadata) -> Self {
            Self {
                installed: Some(installed),
                actions: RefCell::new(Vec::new()),
            }
        }
    }

    impl CompanionDeployRuntime for TestRuntime {
        fn inspect_installed(
            &self,
            _device_id: &str,
            _companion_id: &str,
        ) -> HostResult<Option<InstalledCompanionMetadata>> {
            Ok(self.installed.clone())
        }

        fn install(
            &self,
            _device_id: &str,
            _manifest: &BundledCompanionManifest,
        ) -> HostResult<()> {
            self.actions.borrow_mut().push("install".to_string());
            Ok(())
        }

        fn uninstall(&self, _device_id: &str, _companion_id: &str) -> HostResult<()> {
            self.actions.borrow_mut().push("uninstall".to_string());
            Ok(())
        }

        fn launch_session(
            &self,
            _device_id: &str,
            _manifest: &BundledCompanionManifest,
            _request: &CompanionLaunchRequest,
        ) -> HostResult<()> {
            self.actions.borrow_mut().push("launch".to_string());
            Ok(())
        }
    }

    fn manifest() -> BundledCompanionManifest {
        BundledCompanionManifest {
            companion_id: "cn.magicdian.hscrcpy.server".to_string(),
            artifact_path: "assets/companion/hscrcpy_server.hap".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1_000_000,
            protocol_major: 1,
            protocol_minor: 0,
            sha256: "00".repeat(32),
            supported_abis: Vec::new(),
            launch_ability: "EntryAbility".to_string(),
        }
    }

    fn installed(
        version_code: u64,
        protocol_major: u16,
        protocol_minor: u16,
    ) -> InstalledCompanionMetadata {
        InstalledCompanionMetadata {
            companion_id: "cn.magicdian.hscrcpy.server".to_string(),
            version_name: "1.0.0".to_string(),
            version_code,
            protocol_major,
            protocol_minor,
        }
    }

    #[test]
    fn plans_missing_state_as_install() {
        let manager = BundledCompanionManager::new(manifest(), TestRuntime::default());
        let plan = manager.plan("device-1").expect("planning should succeed");
        assert_eq!(plan.deploy_state(), CompanionDeployState::Missing);
        assert_eq!(plan.action, CompanionAction::Install);
    }

    #[test]
    fn plans_ready_state_as_skip() {
        let runtime = TestRuntime::with_installed(installed(1_000_000, 1, 0));
        let manager = BundledCompanionManager::new(manifest(), runtime);
        let plan = manager.plan("device-1").expect("planning should succeed");
        assert_eq!(plan.deploy_state(), CompanionDeployState::Ready);
        assert_eq!(plan.action, CompanionAction::Skip);
    }

    #[test]
    fn plans_upgrade_required_as_update() {
        let runtime = TestRuntime::with_installed(installed(999_999, 1, 0));
        let manager = BundledCompanionManager::new(manifest(), runtime);
        let plan = manager.plan("device-1").expect("planning should succeed");
        assert_eq!(plan.deploy_state(), CompanionDeployState::UpgradeRequired);
        assert_eq!(plan.action, CompanionAction::Update);
    }

    #[test]
    fn plans_protocol_incompatible_as_reinstall() {
        let runtime = TestRuntime::with_installed(installed(1_000_000, 0, 9));
        let manager = BundledCompanionManager::new(manifest(), runtime);
        let plan = manager.plan("device-1").expect("planning should succeed");
        assert_eq!(
            plan.deploy_state(),
            CompanionDeployState::ProtocolIncompatible
        );
        assert_eq!(plan.action, CompanionAction::Reinstall);
    }

    #[test]
    fn plans_companion_too_new_as_block() {
        let runtime = TestRuntime::with_installed(installed(1_000_001, 1, 1));
        let manager = BundledCompanionManager::new(manifest(), runtime);
        let plan = manager.plan("device-1").expect("planning should succeed");
        assert_eq!(plan.deploy_state(), CompanionDeployState::CompanionTooNew);
        assert_eq!(plan.action, CompanionAction::Block);
    }

    #[test]
    fn apply_reinstall_runs_uninstall_then_install() {
        let runtime = TestRuntime::with_installed(installed(1_000_000, 0, 9));
        let manager = BundledCompanionManager::new(manifest(), runtime);
        let plan = CompanionPlan {
            action: CompanionAction::Reinstall,
            target_version: "1.0.0".to_string(),
            artifact_hint: "assets/companion/hscrcpy_server.hap".to_string(),
        };

        manager
            .apply("device-1", &plan)
            .expect("reinstall should succeed");
        assert_eq!(
            manager.runtime.actions.borrow().as_slice(),
            &["uninstall".to_string(), "install".to_string()]
        );
    }

    #[test]
    fn apply_block_returns_contract_violation() {
        let manager = BundledCompanionManager::new(manifest(), TestRuntime::default());
        let plan = CompanionPlan {
            action: CompanionAction::Block,
            target_version: "1.0.0".to_string(),
            artifact_hint: "assets/companion/hscrcpy_server.hap".to_string(),
        };
        let err = manager
            .apply("device-1", &plan)
            .expect_err("blocked plan should fail");
        assert!(
            matches!(err, HostError::ContractViolation(_)),
            "unexpected error: {err}"
        );
        assert!(
            err.to_string().contains("companion_too_new"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parses_installed_metadata_from_bm_output() {
        let raw = r#"
        {
          "versionCode": 1000000,
          "versionName": "1.0.0",
          "protocolMajor": 1,
          "protocolMinor": 0
        }
        "#;
        let parsed = parse_bm_dump_output("cn.magicdian.hscrcpy.server", raw)
            .expect("parse should succeed")
            .expect("metadata should exist");
        assert_eq!(parsed.version_code, 1_000_000);
        assert_eq!(parsed.protocol_major, 1);
        assert_eq!(parsed.protocol_minor, 0);
    }

    #[test]
    fn prefers_top_level_version_fields_from_bm_output() {
        let raw = r#"
        cn.magicdian.hscrcpy.server:
        {
          "applicationInfo": {
            "appQuickFix": {
              "versionCode": 0,
              "versionName": ""
            },
            "versionCode": 42,
            "versionName": "nested"
          },
          "versionCode": 1000000,
          "versionName": "1.0.0"
        }
        "#;
        let parsed = parse_bm_dump_output("cn.magicdian.hscrcpy.server", raw)
            .expect("parse should succeed")
            .expect("metadata should exist");
        assert_eq!(parsed.version_code, 1_000_000);
        assert_eq!(parsed.version_name, "1.0.0");
    }

    #[test]
    fn treats_not_installed_markers_as_missing() {
        let raw = "bundle not found";
        let parsed =
            parse_bm_dump_output("cn.magicdian.hscrcpy.server", raw).expect("parse should succeed");
        assert!(parsed.is_none());
    }
}
