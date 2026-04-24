use crate::hdc::HdcBridge;
use crate::video::h264::{self, H264AccessUnit};
use crate::{HostError, HostResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DEFAULT_REMOTE_SNAPSHOT_PATH: &str = "/data/local/tmp/hscrcpy_startup_snapshot.jpeg";
const SNAPSHOT_DISPLAY_BIN: &str = "snapshot_display";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupSnapshotCapture {
    pub local_path: PathBuf,
    pub remote_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticSnapshotIdrConfig {
    pub ffmpeg_bin: String,
    pub input_path: PathBuf,
    pub display_width: u32,
    pub display_height: u32,
    pub framerate: u16,
}

pub fn capture_startup_snapshot<H: HdcBridge>(
    hdc: &H,
    device_id: &str,
    local_path: impl AsRef<Path>,
) -> HostResult<StartupSnapshotCapture> {
    capture_startup_snapshot_with_remote(
        hdc,
        device_id,
        local_path.as_ref(),
        DEFAULT_REMOTE_SNAPSHOT_PATH,
    )
}

pub fn capture_startup_snapshot_with_remote<H: HdcBridge>(
    hdc: &H,
    device_id: &str,
    local_path: &Path,
    remote_path: &str,
) -> HostResult<StartupSnapshotCapture> {
    if remote_path.trim().is_empty() {
        return Err(HostError::ContractViolation(
            "startup snapshot remote path must not be empty".to_string(),
        ));
    }
    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            HostError::HdcFailure(format!(
                "startup snapshot phase=local-dir path={} error={error}",
                parent.display()
            ))
        })?;
    }

    let remote = remote_path.trim();
    remove_stale_local_file(local_path)?;
    hdc.exec_shell(device_id, &snapshot_display_command(remote))
        .map_err(|error| {
            HostError::HdcFailure(format!(
                "startup snapshot phase=capture device={device_id} remote={remote}: {error}"
            ))
        })?;
    receive_snapshot_file(hdc, device_id, remote, local_path)?;
    let _ = hdc.exec_shell(device_id, &format!("rm -f {}", shell_quote(remote)));

    Ok(StartupSnapshotCapture {
        local_path: local_path.to_path_buf(),
        remote_path: remote.to_string(),
    })
}

pub fn encode_snapshot_as_h264_idr(
    config: &SyntheticSnapshotIdrConfig,
) -> HostResult<H264AccessUnit> {
    validate_encode_config(config)?;
    let args = build_ffmpeg_snapshot_idr_args(config);
    let output = Command::new(&config.ffmpeg_bin)
        .args(&args)
        .output()
        .map_err(|error| {
            HostError::RenderFailure(format!(
                "startup snapshot phase=encode command={} error={error}",
                config.ffmpeg_bin
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(HostError::RenderFailure(format!(
            "startup snapshot phase=encode command={} status={} stderr={}",
            config.ffmpeg_bin,
            output
                .status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string()),
            compact_output(&stderr)
        )));
    }
    if output.stdout.is_empty() {
        return Err(HostError::RenderFailure(
            "startup snapshot phase=encode produced empty h264 payload".to_string(),
        ));
    }
    let profile = h264::access_unit_profile(&output.stdout)?;
    if !profile.has_idr {
        return Err(HostError::RenderFailure(format!(
            "startup snapshot phase=encode h264 output is not an IDR access unit: nals={} types={}",
            profile.nal_count,
            profile.nal_types_csv()
        )));
    }

    Ok(H264AccessUnit {
        timestamp_micros: 0,
        is_keyframe: true,
        encoded_bytes: output.stdout,
    })
}

pub fn build_ffmpeg_snapshot_idr_args(config: &SyntheticSnapshotIdrConfig) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        config.input_path.display().to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        "-vf".to_string(),
        snapshot_scale_filter(config),
        "-r".to_string(),
        config.framerate.to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "ultrafast".to_string(),
        "-tune".to_string(),
        "zerolatency".to_string(),
        "-x264-params".to_string(),
        "keyint=1:min-keyint=1:scenecut=0".to_string(),
        "-f".to_string(),
        "h264".to_string(),
        "pipe:1".to_string(),
    ]
}

pub fn snapshot_display_command(remote_path: &str) -> String {
    format!(
        "rm -f {remote}; {bin} -f {remote}; test -f {remote}",
        remote = shell_quote(remote_path),
        bin = SNAPSHOT_DISPLAY_BIN
    )
}

fn receive_snapshot_file<H: HdcBridge>(
    hdc: &H,
    device_id: &str,
    remote_path: &str,
    local_path: &Path,
) -> HostResult<()> {
    let local_parent = local_path.parent().unwrap_or_else(|| Path::new("."));
    hdc.recv_file(device_id, remote_path, local_parent)
        .map_err(|error| {
            HostError::HdcFailure(format!(
                "startup snapshot phase=recv device={device_id} remote={remote_path} local_dir={}: {error}",
                local_parent.display()
            ))
        })?;

    if local_path.is_file() {
        return Ok(());
    }

    let remote_file_name = remote_path
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .ok_or_else(|| {
            HostError::ContractViolation(format!(
                "startup snapshot remote path has no file name: {remote_path}"
            ))
        })?;
    let received_path = local_parent.join(remote_file_name);
    if !received_path.is_file() {
        let remote_diag = hdc
            .exec_shell(
                device_id,
                &format!(
                    "ls -l {} 2>&1; file {} 2>&1",
                    shell_quote(remote_path),
                    shell_quote(remote_path)
                ),
            )
            .unwrap_or_else(|error| format!("remote diagnostic failed: {error}"));
        let local_diag = local_directory_listing(local_parent);
        return Err(HostError::HdcFailure(format!(
            "startup snapshot phase=recv local artifact missing after hdc file recv: expected {} or {} remote_diag=[{}] local_diag=[{}]",
            local_path.display(),
            received_path.display(),
            compact_output(&remote_diag),
            local_diag
        )));
    }
    if received_path != local_path {
        fs::rename(&received_path, local_path).map_err(|error| {
            HostError::HdcFailure(format!(
                "startup snapshot phase=local-rename from={} to={} error={error}",
                received_path.display(),
                local_path.display()
            ))
        })?;
    }
    Ok(())
}

fn remove_stale_local_file(local_path: &Path) -> HostResult<()> {
    match fs::remove_file(local_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(HostError::HdcFailure(format!(
            "startup snapshot phase=local-cleanup path={} error={error}",
            local_path.display()
        ))),
    }
}

fn local_directory_listing(path: &Path) -> String {
    match fs::read_dir(path) {
        Ok(entries) => {
            let names = entries
                .filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect::<Vec<_>>()
                .join(",");
            if names.is_empty() {
                "empty".to_string()
            } else {
                names
            }
        }
        Err(error) => format!("read_dir failed: {error}"),
    }
}

fn validate_encode_config(config: &SyntheticSnapshotIdrConfig) -> HostResult<()> {
    if (config.display_width == 0) != (config.display_height == 0) {
        return Err(HostError::ContractViolation(format!(
            "startup snapshot encode requires both display dimensions or neither, got {}x{}",
            config.display_width, config.display_height
        )));
    }
    if config.framerate == 0 {
        return Err(HostError::ContractViolation(
            "startup snapshot encode requires non-zero framerate".to_string(),
        ));
    }
    Ok(())
}

fn snapshot_scale_filter(config: &SyntheticSnapshotIdrConfig) -> String {
    if config.display_width == 0 && config.display_height == 0 {
        return "scale=trunc(iw/2)*2:trunc(ih/2)*2,format=yuv420p".to_string();
    }
    format!(
        "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2,format=yuv420p",
        config.display_width, config.display_height, config.display_width, config.display_height
    )
}

fn shell_quote(raw: &str) -> String {
    format!("'{}'", raw.replace('\'', "'\\''"))
}

fn compact_output(output: &str) -> String {
    let compact = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    if compact.is_empty() {
        "empty".to_string()
    } else {
        compact
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_ffmpeg_snapshot_idr_args, receive_snapshot_file, snapshot_display_command,
        SyntheticSnapshotIdrConfig,
    };
    use crate::hdc::HdcBridge;
    use crate::HostResult;
    use std::cell::RefCell;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("hscrcpy-snapshot-{name}-{unique}"));
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

    #[derive(Default)]
    struct MockRecvHdc {
        recv_calls: RefCell<Vec<(String, PathBuf)>>,
    }

    impl HdcBridge for MockRecvHdc {
        fn ensure_device_visible(&self, _device_id: &str) -> HostResult<()> {
            Ok(())
        }

        fn forward_port(
            &self,
            _device_id: &str,
            _local_port: u16,
            _remote_port: u16,
        ) -> HostResult<()> {
            Ok(())
        }

        fn recv_file(
            &self,
            _device_id: &str,
            remote_path: &str,
            local_path: &Path,
        ) -> HostResult<()> {
            self.recv_calls
                .borrow_mut()
                .push((remote_path.to_string(), local_path.to_path_buf()));
            let remote_file_name = remote_path
                .rsplit('/')
                .find(|segment| !segment.is_empty())
                .expect("test remote path has file name");
            fs::write(local_path.join(remote_file_name), b"snapshot")
                .expect("mock recv should write received file");
            Ok(())
        }

        fn exec_shell(&self, _device_id: &str, _command: &str) -> HostResult<String> {
            Ok(String::new())
        }
    }

    #[test]
    fn snapshot_display_command_quotes_remote_path() {
        assert_eq!(
            snapshot_display_command("/data/local/tmp/startup image.jpg"),
            "rm -f '/data/local/tmp/startup image.jpg'; snapshot_display -f '/data/local/tmp/startup image.jpg'; test -f '/data/local/tmp/startup image.jpg'"
        );
    }

    #[test]
    fn ffmpeg_args_encode_one_scaled_padded_h264_idr() {
        let args = build_ffmpeg_snapshot_idr_args(&SyntheticSnapshotIdrConfig {
            ffmpeg_bin: "ffmpeg".to_string(),
            input_path: PathBuf::from("target/startup.jpg"),
            display_width: 1280,
            display_height: 720,
            framerate: 120,
        });

        assert!(args.windows(2).any(|pair| pair == ["-frames:v", "1"]));
        assert!(args.windows(2).any(|pair| pair == ["-c:v", "libx264"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-x264-params", "keyint=1:min-keyint=1:scenecut=0"]));
        assert!(args.contains(&"scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2,format=yuv420p".to_string()));
        assert!(args.windows(2).any(|pair| pair == ["-f", "h264"]));
        assert_eq!(args.last().map(String::as_str), Some("pipe:1"));
    }

    #[test]
    fn ffmpeg_args_can_preserve_even_source_dimensions_before_negotiation() {
        let args = build_ffmpeg_snapshot_idr_args(&SyntheticSnapshotIdrConfig {
            ffmpeg_bin: "ffmpeg".to_string(),
            input_path: PathBuf::from("target/startup.jpg"),
            display_width: 0,
            display_height: 0,
            framerate: 120,
        });

        assert!(args.contains(&"scale=trunc(iw/2)*2:trunc(ih/2)*2,format=yuv420p".to_string()));
    }

    #[test]
    fn receive_snapshot_file_pulls_into_parent_dir_then_renames_to_target() {
        let temp = TestDir::new("recv");
        let hdc = MockRecvHdc::default();
        let target = temp.path().join("startup-snapshot.jpg");

        receive_snapshot_file(
            &hdc,
            "SERIAL_A",
            "/data/local/tmp/hscrcpy_startup_snapshot.jpeg",
            &target,
        )
        .expect("snapshot receive should succeed");

        assert_eq!(fs::read(&target).expect("target should exist"), b"snapshot");
        assert!(!temp.path().join("hscrcpy_startup_snapshot.jpeg").exists());
        assert_eq!(
            hdc.recv_calls.borrow().as_slice(),
            &[(
                "/data/local/tmp/hscrcpy_startup_snapshot.jpeg".to_string(),
                temp.path().to_path_buf()
            )]
        );
    }
}
