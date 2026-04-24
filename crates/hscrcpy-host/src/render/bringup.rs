use super::{JpegFrame, JpegFrameSink};
use crate::video::{
    h264::{self, H264AccessUnit},
    PreparedVideoIngress,
};
use crate::{HostError, HostResult};
use hscrcpy_contracts::VideoCodec;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

const H264_ANNEX_B_START_CODE: [u8; 4] = [0, 0, 0, 1];
const DEFAULT_FFPLAY_BIN: &str = "ffplay";
const DEFAULT_FFPLAY_FRAMERATE: u16 = 120;
const H264_DIAGNOSTIC_SUMMARY_INTERVAL: usize = 120;
const H264_LIVE_PREVIEW_WRITE_WARN_US: u64 = 20_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct H264LivePreviewConfig {
    pub ffplay_bin: String,
    pub framerate: u16,
}

impl Default for H264LivePreviewConfig {
    fn default() -> Self {
        Self {
            ffplay_bin: DEFAULT_FFPLAY_BIN.to_string(),
            framerate: DEFAULT_FFPLAY_FRAMERATE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSessionDescriptor {
    pub session_id: String,
    pub selected_codec: VideoCodec,
    pub display_width: u32,
    pub display_height: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RenderStreamStats {
    pub total_units: usize,
    pub jpeg_frames: usize,
    pub h264_access_units: usize,
    pub last_timestamp_micros: Option<u64>,
    pub last_artifact_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedArtifact {
    pub frame_index: usize,
    pub codec: VideoCodec,
    pub timestamp_micros: u64,
    pub is_keyframe: Option<bool>,
    pub artifact_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
pub struct IngressTimingSample {
    receive_started_at: Instant,
    receive_completed_at: Instant,
}

impl IngressTimingSample {
    pub fn new(receive_started_at: Instant, receive_completed_at: Instant) -> Self {
        Self {
            receive_started_at,
            receive_completed_at,
        }
    }

    fn receive_wait_duration(&self) -> Duration {
        self.receive_completed_at
            .saturating_duration_since(self.receive_started_at)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BringupDiagnosticsSummary {
    pub units: usize,
    pub first_frame_index: usize,
    pub last_frame_index: usize,
    pub full_window: bool,
    pub pts_span_us: Option<u64>,
    pub host_receive_span_us: Option<u64>,
    pub host_present_span_us: Option<u64>,
    pub host_receive_minus_pts_us: Option<i128>,
    pub host_present_minus_pts_us: Option<i128>,
    pub ingress_wait_avg_us: Option<u64>,
    pub ingress_wait_max_us: Option<u64>,
    pub present_avg_us: u64,
    pub present_max_us: u64,
    pub ffplay_write_samples: usize,
    pub ffplay_write_avg_us: Option<u64>,
    pub ffplay_write_max_us: Option<u64>,
    pub ffplay_slow_writes: usize,
    pub ffplay_slow_threshold_us: u64,
}

impl BringupDiagnosticsSummary {
    fn event_line(&self) -> String {
        let format_u64 =
            |value: Option<u64>| value.map_or_else(|| "na".to_string(), |raw| raw.to_string());
        let format_i128 =
            |value: Option<i128>| value.map_or_else(|| "na".to_string(), |raw| raw.to_string());
        format!(
            "diag window={} units={} frame_start={} frame_end={} codec=h264 pts_span_us={} host_rx_span_us={} host_present_span_us={} host_rx_minus_pts_us={} host_present_minus_pts_us={} ingress_wait_avg_us={} ingress_wait_max_us={} present_avg_us={} present_max_us={} ffplay_write_samples={} ffplay_write_avg_us={} ffplay_write_max_us={} ffplay_slow_writes={} ffplay_slow_threshold_us={}",
            if self.full_window { "full" } else { "partial" },
            self.units,
            self.first_frame_index,
            self.last_frame_index,
            format_u64(self.pts_span_us),
            format_u64(self.host_receive_span_us),
            format_u64(self.host_present_span_us),
            format_i128(self.host_receive_minus_pts_us),
            format_i128(self.host_present_minus_pts_us),
            format_u64(self.ingress_wait_avg_us),
            format_u64(self.ingress_wait_max_us),
            self.present_avg_us,
            self.present_max_us,
            self.ffplay_write_samples,
            format_u64(self.ffplay_write_avg_us),
            format_u64(self.ffplay_write_max_us),
            self.ffplay_slow_writes,
            self.ffplay_slow_threshold_us,
        )
    }
}

pub struct BringupRenderSurface {
    output_root: PathBuf,
    session: Option<RenderSessionDescriptor>,
    session_dir: Option<PathBuf>,
    frames_dir: Option<PathBuf>,
    h264_live_preview_config: Option<H264LivePreviewConfig>,
    h264_live_preview: Option<H264LivePreview>,
    live_preview_status: Option<String>,
    record_h264_artifacts: bool,
    h264_diagnostics: H264DiagnosticState,
    pending_h264_diagnostic_summary: Option<BringupDiagnosticsSummary>,
    stats: RenderStreamStats,
}

impl BringupRenderSurface {
    pub fn new(output_root: impl Into<PathBuf>) -> Self {
        Self {
            output_root: output_root.into(),
            session: None,
            session_dir: None,
            frames_dir: None,
            h264_live_preview_config: None,
            h264_live_preview: None,
            live_preview_status: None,
            record_h264_artifacts: false,
            h264_diagnostics: H264DiagnosticState::default(),
            pending_h264_diagnostic_summary: None,
            stats: RenderStreamStats::default(),
        }
    }

    pub fn with_h264_live_preview(mut self, config: H264LivePreviewConfig) -> Self {
        self.h264_live_preview_config = Some(config);
        self
    }

    pub fn with_h264_artifact_recording(mut self) -> Self {
        self.record_h264_artifacts = true;
        self
    }

    pub fn initialize_session(&mut self, session: RenderSessionDescriptor) -> HostResult<()> {
        let session_dir = self.output_root.join(&session.session_id);
        let frames_dir = session_dir.join("frames");
        fs::create_dir_all(&frames_dir)
            .map_err(|error| io_error("create render output directories", &frames_dir, error))?;
        write_text_file(&session_dir.join("events.log"), "")?;

        self.stats = RenderStreamStats::default();
        self.h264_diagnostics = H264DiagnosticState::default();
        self.pending_h264_diagnostic_summary = None;
        self.session = Some(session.clone());
        self.session_dir = Some(session_dir.clone());
        self.frames_dir = Some(frames_dir.clone());

        self.h264_live_preview = None;
        self.live_preview_status = None;
        if session.selected_codec == VideoCodec::H264 {
            self.append_event_note(&format!(
                "diag_config codec=h264 summary_interval_units={} ffplay_slow_write_us={}",
                H264_DIAGNOSTIC_SUMMARY_INTERVAL, H264_LIVE_PREVIEW_WRITE_WARN_US
            ))?;
            if let Some(config) = &self.h264_live_preview_config {
                match H264LivePreview::spawn(config, &session_dir, &session) {
                    Ok(preview) => {
                        self.live_preview_status =
                            Some(format!("active via {}", preview.command_path()));
                        self.append_event_note(&format!(
                            "live_preview status=active backend=ffplay command={}",
                            preview.command_path()
                        ))?;
                        self.h264_live_preview = Some(preview);
                    }
                    Err(error) => {
                        let status = format!("disabled: {error}");
                        self.live_preview_status = Some(status.clone());
                        self.append_event_note(&format!(
                            "live_preview status=disabled reason={error}"
                        ))?;
                    }
                }
            } else {
                self.live_preview_status = Some("disabled by host configuration".to_string());
            }
        }

        write_text_file(
            &session_dir.join("session.txt"),
            &render_session_report(
                &session,
                self.live_preview_status.as_deref(),
                self.record_h264_artifacts,
            ),
        )?;
        write_text_file(
            &session_dir.join("preview.html"),
            &render_preview_html(
                &session,
                self.live_preview_status.as_deref(),
                self.record_h264_artifacts,
            ),
        )?;

        Ok(())
    }

    pub fn preview_html_path(&self) -> Option<PathBuf> {
        self.session_dir
            .as_ref()
            .map(|dir| dir.join("preview.html"))
    }

    pub fn session_dir(&self) -> Option<&Path> {
        self.session_dir.as_deref()
    }

    pub fn live_preview_status(&self) -> Option<&str> {
        self.live_preview_status.as_deref()
    }

    pub fn stats(&self) -> &RenderStreamStats {
        &self.stats
    }

    pub fn present_video_ingress(
        &mut self,
        ingress: &PreparedVideoIngress,
    ) -> HostResult<RenderedArtifact> {
        self.present_video_ingress_internal(ingress, None)
    }

    pub fn present_video_ingress_with_timing(
        &mut self,
        ingress: &PreparedVideoIngress,
        timing: IngressTimingSample,
    ) -> HostResult<RenderedArtifact> {
        self.present_video_ingress_internal(ingress, Some(timing))
    }

    pub fn take_h264_diagnostic_summary(&mut self) -> Option<BringupDiagnosticsSummary> {
        self.pending_h264_diagnostic_summary.take()
    }

    pub fn flush_h264_diagnostic_summary(
        &mut self,
    ) -> HostResult<Option<BringupDiagnosticsSummary>> {
        let Some(summary) = self.h264_diagnostics.flush_partial_window() else {
            return Ok(None);
        };
        self.append_event_note(&summary.event_line())?;
        self.pending_h264_diagnostic_summary = Some(summary.clone());
        Ok(Some(summary))
    }

    fn present_video_ingress_internal(
        &mut self,
        ingress: &PreparedVideoIngress,
        timing: Option<IngressTimingSample>,
    ) -> HostResult<RenderedArtifact> {
        match ingress {
            PreparedVideoIngress::H264(access_unit) => {
                self.present_h264_access_unit(access_unit, timing)
            }
            PreparedVideoIngress::Jpeg(frame) => self.write_jpeg_frame(frame),
            PreparedVideoIngress::DeferredExperimental(packet) => {
                Err(HostError::RenderFailure(format!(
                    "render bringup only supports negotiated h264/jpeg ingress, got {:?}",
                    packet.metadata.codec
                )))
            }
        }
    }

    fn present_h264_access_unit(
        &mut self,
        access_unit: &H264AccessUnit,
        timing: Option<IngressTimingSample>,
    ) -> HostResult<RenderedArtifact> {
        let frame_index = self.stats.total_units + 1;
        let present_started_at = Instant::now();

        let session_dir = self.require_session_dir()?;
        let frame_path = if self.record_h264_artifacts {
            let frame_path = self
                .require_frames_dir()?
                .join(format!("frame-{frame_index:06}.h264"));
            fs::write(&frame_path, &access_unit.encoded_bytes)
                .map_err(|error| io_error("write h264 access unit", &frame_path, error))?;

            let stream_path = session_dir.join("stream.h264");
            let mut stream = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&stream_path)
                .map_err(|error| io_error("open h264 stream artifact", &stream_path, error))?;
            stream
                .write_all(&H264_ANNEX_B_START_CODE)
                .and_then(|_| stream.write_all(&access_unit.encoded_bytes))
                .map_err(|error| io_error("append h264 access unit", &stream_path, error))?;
            Some(frame_path)
        } else {
            None
        };

        let preview_result = self.feed_live_preview(access_unit)?;
        if let Some(note) = preview_result.note.as_deref() {
            self.append_event_note(note)?;
        }
        if let Some(error) = preview_result.error.as_deref() {
            self.live_preview_status = Some(format!("disabled: {error}"));
            self.append_event_note(&format!("live_preview status=disabled reason={error}"))?;
        }

        let present_elapsed = present_started_at.elapsed();
        let present_completed_at = Instant::now();
        if let Some(summary) = self.h264_diagnostics.record_frame(
            frame_index,
            access_unit.timestamp_micros,
            timing,
            present_completed_at,
            present_elapsed,
            preview_result.write_duration_us,
        ) {
            self.append_event_note(&summary.event_line())?;
            self.pending_h264_diagnostic_summary = Some(summary);
        }

        self.record_artifact(
            frame_index,
            VideoCodec::H264,
            access_unit.timestamp_micros,
            Some(access_unit.is_keyframe),
            frame_path.clone(),
        )?;
        self.stats.h264_access_units += 1;
        Ok(RenderedArtifact {
            frame_index,
            codec: VideoCodec::H264,
            timestamp_micros: access_unit.timestamp_micros,
            is_keyframe: Some(access_unit.is_keyframe),
            artifact_path: frame_path,
        })
    }

    fn write_jpeg_frame(&mut self, frame: &JpegFrame) -> HostResult<RenderedArtifact> {
        let session_dir = self.require_session_dir()?;
        let frame_index = self.stats.total_units + 1;
        let frame_path = self
            .require_frames_dir()?
            .join(format!("frame-{frame_index:06}.jpg"));
        fs::write(&frame_path, &frame.encoded_bytes)
            .map_err(|error| io_error("write jpeg frame artifact", &frame_path, error))?;

        let latest_path = session_dir.join("latest.jpg");
        fs::write(&latest_path, &frame.encoded_bytes)
            .map_err(|error| io_error("refresh latest jpeg preview", &latest_path, error))?;

        self.record_artifact(
            frame_index,
            VideoCodec::Jpeg,
            frame.timestamp_micros,
            None,
            Some(frame_path.clone()),
        )?;
        self.stats.jpeg_frames += 1;
        Ok(RenderedArtifact {
            frame_index,
            codec: VideoCodec::Jpeg,
            timestamp_micros: frame.timestamp_micros,
            is_keyframe: None,
            artifact_path: Some(frame_path),
        })
    }

    fn feed_live_preview(
        &mut self,
        access_unit: &H264AccessUnit,
    ) -> HostResult<H264LivePreviewResult> {
        let Some(preview) = self.h264_live_preview.as_mut() else {
            return Ok(H264LivePreviewResult::default());
        };
        match preview.present_access_unit(access_unit) {
            Ok(metrics) => Ok(H264LivePreviewResult {
                write_duration_us: metrics.write_duration_us,
                note: metrics.waiting_for_keyframe.then(|| {
                    "live_preview status=waiting_for_keyframe reason=first_h264_access_unit_is_not_idr"
                        .to_string()
                }),
                error: None,
            }),
            Err(error) => {
                self.h264_live_preview = None;
                Ok(H264LivePreviewResult {
                    write_duration_us: None,
                    note: None,
                    error: Some(error.to_string()),
                })
            }
        }
    }

    fn record_artifact(
        &mut self,
        frame_index: usize,
        codec: VideoCodec,
        timestamp_micros: u64,
        is_keyframe: Option<bool>,
        artifact_path: Option<PathBuf>,
    ) -> HostResult<()> {
        let event_path = self.require_session_dir()?.join("events.log");
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&event_path)
            .map_err(|error| io_error("open render event log", &event_path, error))?;
        let artifact_value = artifact_path
            .as_ref()
            .map_or_else(|| "disabled".to_string(), |path| path.display().to_string());
        writeln!(
            log,
            "frame={} codec={} pts_us={} keyframe={} artifact={}",
            frame_index,
            codec_name(&codec),
            timestamp_micros,
            format_optional_bool(is_keyframe),
            artifact_value
        )
        .map_err(|error| io_error("append render event log", &event_path, error))?;

        self.stats.total_units = frame_index;
        self.stats.last_timestamp_micros = Some(timestamp_micros);
        self.stats.last_artifact_path = artifact_path;
        Ok(())
    }

    fn append_event_note(&self, note: &str) -> HostResult<()> {
        let event_path = self
            .session_dir
            .as_ref()
            .map(|dir| dir.join("events.log"))
            .ok_or_else(|| {
                HostError::RenderFailure(
                    "render surface must be initialized before appending event notes".to_string(),
                )
            })?;
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&event_path)
            .map_err(|error| io_error("open render event log", &event_path, error))?;
        writeln!(log, "{note}")
            .map_err(|error| io_error("append render event note", &event_path, error))?;
        Ok(())
    }

    fn require_session_dir(&self) -> HostResult<&Path> {
        self.session_dir.as_deref().ok_or_else(|| {
            HostError::RenderFailure(
                "render surface must be initialized with session metadata before presenting frames"
                    .to_string(),
            )
        })
    }

    fn require_frames_dir(&self) -> HostResult<&Path> {
        self.frames_dir.as_deref().ok_or_else(|| {
            HostError::RenderFailure(
                "render surface frames directory is unavailable before initialization".to_string(),
            )
        })
    }
}

struct H264LivePreview {
    command_path: String,
    child: Child,
    stdin: Option<ChildStdin>,
    received_keyframe: bool,
    logged_waiting_for_keyframe: bool,
    pending_decoder_config: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct H264LivePreviewMetrics {
    write_duration_us: Option<u64>,
    waiting_for_keyframe: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct H264LivePreviewResult {
    write_duration_us: Option<u64>,
    note: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Default)]
struct H264DiagnosticState {
    window: H264DiagnosticWindow,
}

#[derive(Debug, Default)]
struct H264DiagnosticWindow {
    units: usize,
    first_frame_index: Option<usize>,
    last_frame_index: Option<usize>,
    first_pts_us: Option<u64>,
    last_pts_us: Option<u64>,
    first_host_receive_at: Option<Instant>,
    last_host_receive_at: Option<Instant>,
    first_host_present_at: Option<Instant>,
    last_host_present_at: Option<Instant>,
    ingress_wait_total_us: u128,
    ingress_wait_max_us: u64,
    ingress_wait_samples: usize,
    present_total_us: u128,
    present_max_us: u64,
    ffplay_write_total_us: u128,
    ffplay_write_max_us: u64,
    ffplay_write_samples: usize,
    ffplay_slow_writes: usize,
}

impl H264DiagnosticState {
    fn record_frame(
        &mut self,
        frame_index: usize,
        pts_us: u64,
        timing: Option<IngressTimingSample>,
        present_completed_at: Instant,
        present_elapsed: Duration,
        ffplay_write_us: Option<u64>,
    ) -> Option<BringupDiagnosticsSummary> {
        self.window.record_frame(
            frame_index,
            pts_us,
            timing,
            present_completed_at,
            present_elapsed,
            ffplay_write_us,
        );
        if self.window.units >= H264_DIAGNOSTIC_SUMMARY_INTERVAL {
            return self.flush_window(true);
        }
        None
    }

    fn flush_partial_window(&mut self) -> Option<BringupDiagnosticsSummary> {
        self.flush_window(false)
    }

    fn flush_window(&mut self, full_window: bool) -> Option<BringupDiagnosticsSummary> {
        let summary = self.window.build_summary(full_window)?;
        self.window = H264DiagnosticWindow::default();
        Some(summary)
    }
}

impl H264DiagnosticWindow {
    fn record_frame(
        &mut self,
        frame_index: usize,
        pts_us: u64,
        timing: Option<IngressTimingSample>,
        present_completed_at: Instant,
        present_elapsed: Duration,
        ffplay_write_us: Option<u64>,
    ) {
        self.units += 1;
        self.last_frame_index = Some(frame_index);
        if self.first_frame_index.is_none() {
            self.first_frame_index = Some(frame_index);
        }
        self.last_pts_us = Some(pts_us);
        if self.first_pts_us.is_none() {
            self.first_pts_us = Some(pts_us);
        }

        if let Some(sample) = timing {
            self.last_host_receive_at = Some(sample.receive_completed_at);
            if self.first_host_receive_at.is_none() {
                self.first_host_receive_at = Some(sample.receive_completed_at);
            }

            let ingress_wait_us = duration_to_u64_micros(sample.receive_wait_duration());
            self.ingress_wait_total_us = self
                .ingress_wait_total_us
                .saturating_add(u128::from(ingress_wait_us));
            self.ingress_wait_max_us = self.ingress_wait_max_us.max(ingress_wait_us);
            self.ingress_wait_samples += 1;
        }

        self.last_host_present_at = Some(present_completed_at);
        if self.first_host_present_at.is_none() {
            self.first_host_present_at = Some(present_completed_at);
        }
        let present_elapsed_us = duration_to_u64_micros(present_elapsed);
        self.present_total_us = self
            .present_total_us
            .saturating_add(u128::from(present_elapsed_us));
        self.present_max_us = self.present_max_us.max(present_elapsed_us);

        if let Some(write_us) = ffplay_write_us {
            self.ffplay_write_total_us = self
                .ffplay_write_total_us
                .saturating_add(u128::from(write_us));
            self.ffplay_write_max_us = self.ffplay_write_max_us.max(write_us);
            self.ffplay_write_samples += 1;
            if write_us >= H264_LIVE_PREVIEW_WRITE_WARN_US {
                self.ffplay_slow_writes += 1;
            }
        }
    }

    fn build_summary(&self, full_window: bool) -> Option<BringupDiagnosticsSummary> {
        let first_frame_index = self.first_frame_index?;
        let last_frame_index = self.last_frame_index?;
        let pts_span_us = span_u64(self.first_pts_us, self.last_pts_us);
        let host_receive_span_us =
            span_instant_us(self.first_host_receive_at, self.last_host_receive_at);
        let host_present_span_us =
            span_instant_us(self.first_host_present_at, self.last_host_present_at);
        let host_receive_minus_pts_us = subtract_span(host_receive_span_us, pts_span_us);
        let host_present_minus_pts_us = subtract_span(host_present_span_us, pts_span_us);

        Some(BringupDiagnosticsSummary {
            units: self.units,
            first_frame_index,
            last_frame_index,
            full_window,
            pts_span_us,
            host_receive_span_us,
            host_present_span_us,
            host_receive_minus_pts_us,
            host_present_minus_pts_us,
            ingress_wait_avg_us: average_u128_to_u64(
                self.ingress_wait_total_us,
                self.ingress_wait_samples,
            ),
            ingress_wait_max_us: if self.ingress_wait_samples > 0 {
                Some(self.ingress_wait_max_us)
            } else {
                None
            },
            present_avg_us: average_u128_to_u64(self.present_total_us, self.units).unwrap_or(0),
            present_max_us: self.present_max_us,
            ffplay_write_samples: self.ffplay_write_samples,
            ffplay_write_avg_us: average_u128_to_u64(
                self.ffplay_write_total_us,
                self.ffplay_write_samples,
            ),
            ffplay_write_max_us: if self.ffplay_write_samples > 0 {
                Some(self.ffplay_write_max_us)
            } else {
                None
            },
            ffplay_slow_writes: self.ffplay_slow_writes,
            ffplay_slow_threshold_us: H264_LIVE_PREVIEW_WRITE_WARN_US,
        })
    }
}

impl H264LivePreview {
    fn spawn(
        config: &H264LivePreviewConfig,
        session_dir: &Path,
        session: &RenderSessionDescriptor,
    ) -> HostResult<Self> {
        let log_path = session_dir.join("live-preview.log");
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|error| io_error("open live preview log", &log_path, error))?;
        let log_file_err = log_file
            .try_clone()
            .map_err(|error| io_error("clone live preview log handle", &log_path, error))?;

        let mut child = Command::new(&config.ffplay_bin)
            .arg("-loglevel")
            .arg("warning")
            .arg("-fflags")
            .arg("nobuffer")
            .arg("-flags")
            .arg("low_delay")
            .arg("-framedrop")
            .arg("-window_title")
            .arg(format!("hscrcpy {}", session.session_id))
            .arg("-framerate")
            .arg(config.framerate.to_string())
            .arg("-f")
            .arg("h264")
            .arg("-i")
            .arg("pipe:0")
            .stdin(Stdio::piped())
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_err))
            .spawn()
            .map_err(|error| {
                HostError::RenderFailure(format!(
                    "spawn live preview command `{}` failed: {error}",
                    config.ffplay_bin
                ))
            })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            HostError::RenderFailure(format!(
                "live preview command `{}` did not expose stdin",
                config.ffplay_bin
            ))
        })?;

        Ok(Self {
            command_path: config.ffplay_bin.clone(),
            child,
            stdin: Some(stdin),
            received_keyframe: false,
            logged_waiting_for_keyframe: false,
            pending_decoder_config: Vec::new(),
        })
    }

    fn command_path(&self) -> &str {
        &self.command_path
    }

    fn present_access_unit(
        &mut self,
        access_unit: &H264AccessUnit,
    ) -> HostResult<H264LivePreviewMetrics> {
        if !self.received_keyframe && !access_unit.is_keyframe {
            let config = h264::decoder_config_bytes(&access_unit.encoded_bytes)?;
            if !config.is_empty() {
                self.pending_decoder_config = config;
            }
            let should_log = !self.logged_waiting_for_keyframe;
            self.logged_waiting_for_keyframe = true;
            return Ok(H264LivePreviewMetrics {
                write_duration_us: None,
                waiting_for_keyframe: should_log,
            });
        }
        if !self.received_keyframe {
            let config = h264::decoder_config_bytes(&access_unit.encoded_bytes)?;
            if !config.is_empty() {
                self.pending_decoder_config = config;
            }
            self.received_keyframe = true;
        }

        let stdin = self.stdin.as_mut().ok_or_else(|| {
            HostError::RenderFailure(format!(
                "live preview command `{}` stdin is unavailable",
                self.command_path
            ))
        })?;
        let write_started_at = Instant::now();
        if !self.pending_decoder_config.is_empty() {
            stdin
                .write_all(&self.pending_decoder_config)
                .map_err(|error| {
                    HostError::RenderFailure(format!(
                        "write h264 decoder config into live preview command `{}` failed: {error}",
                        self.command_path
                    ))
                })?;
            self.pending_decoder_config.clear();
        }
        stdin
            .write_all(&access_unit.encoded_bytes)
            .and_then(|_| stdin.flush())
            .map_err(|error| {
                HostError::RenderFailure(format!(
                    "write h264 access unit into live preview command `{}` failed: {error}",
                    self.command_path
                ))
            })?;
        Ok(H264LivePreviewMetrics {
            write_duration_us: Some(duration_to_u64_micros(write_started_at.elapsed())),
            waiting_for_keyframe: false,
        })
    }
}

impl Drop for H264LivePreview {
    fn drop(&mut self) {
        drop(self.stdin.take());
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
            Err(_) => {}
        }
    }
}

impl JpegFrameSink for BringupRenderSurface {
    fn present_jpeg_frame(&mut self, frame: &JpegFrame) -> HostResult<()> {
        self.write_jpeg_frame(frame).map(|_| ())
    }
}

fn span_u64(start: Option<u64>, end: Option<u64>) -> Option<u64> {
    start.zip(end).map(|(a, b)| b.saturating_sub(a))
}

fn span_instant_us(start: Option<Instant>, end: Option<Instant>) -> Option<u64> {
    start
        .zip(end)
        .map(|(a, b)| duration_to_u64_micros(b.saturating_duration_since(a)))
}

fn subtract_span(lhs: Option<u64>, rhs: Option<u64>) -> Option<i128> {
    let lhs = lhs?;
    let rhs = rhs?;
    Some(i128::from(lhs) - i128::from(rhs))
}

fn average_u128_to_u64(total: u128, count: usize) -> Option<u64> {
    if count == 0 {
        return None;
    }
    u64::try_from(total / count as u128).ok().or(Some(u64::MAX))
}

fn duration_to_u64_micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).ok().unwrap_or(u64::MAX)
}

fn render_session_report(
    session: &RenderSessionDescriptor,
    live_preview_status: Option<&str>,
    record_h264_artifacts: bool,
) -> String {
    format!(
        "session_id={}\nselected_codec={}\ndisplay_width={}\ndisplay_height={}\npreview_html=preview.html\njpeg_latest=latest.jpg\nh264_recording={}\nh264_stream={}\nlive_preview={}\n",
        session.session_id,
        codec_name(&session.selected_codec),
        session.display_width,
        session.display_height,
        if record_h264_artifacts { "enabled" } else { "disabled" },
        if record_h264_artifacts { "stream.h264" } else { "disabled" },
        live_preview_status.unwrap_or("not_configured"),
    )
}

fn render_preview_html(
    session: &RenderSessionDescriptor,
    live_preview_status: Option<&str>,
    record_h264_artifacts: bool,
) -> String {
    let h264_recording_text = if record_h264_artifacts {
        "H264 mainline records per-access-unit artifacts under <code>frames/</code> and appends <code>stream.h264</code>."
    } else {
        "H264 mainline keeps disk recording disabled unless the host is started with explicit H264 recording enabled."
    };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta http-equiv=\"refresh\" content=\"1\">\n<title>hscrcpy Render Bringup</title>\n<style>\nbody {{ font-family: ui-monospace, monospace; margin: 24px; background: #111; color: #f5f5f5; }}\nmain {{ max-width: 960px; margin: 0 auto; }}\nimg {{ width: 100%; max-width: 100%; border: 1px solid #444; background: #000; }}\np {{ line-height: 1.5; }}\ncode {{ color: #8be9fd; }}\n</style>\n</head>\n<body>\n<main>\n<h1>hscrcpy host render bringup</h1>\n<p>Session: <code>{}</code></p>\n<p>Negotiated codec: <code>{}</code></p>\n<p>Display hint: <code>{}x{}</code></p>\n<p>Live preview: <code>{}</code></p>\n<p>This preview auto-refreshes once per second. JPEG fallback updates <code>latest.jpg</code> directly. {}</p>\n<img src=\"latest.jpg\" alt=\"Waiting for JPEG fallback frame\">\n</main>\n</body>\n</html>\n",
        session.session_id,
        codec_name(&session.selected_codec),
        session.display_width,
        session.display_height,
        live_preview_status.unwrap_or("not_configured"),
        h264_recording_text,
    )
}

fn write_text_file(path: &Path, contents: &str) -> HostResult<()> {
    fs::write(path, contents).map_err(|error| io_error("write render text artifact", path, error))
}

fn io_error(operation: &str, path: &Path, error: std::io::Error) -> HostError {
    HostError::RenderFailure(format!("{operation} at {} failed: {error}", path.display()))
}

fn codec_name(codec: &VideoCodec) -> &'static str {
    match codec {
        VideoCodec::H264 => "h264",
        VideoCodec::Jpeg => "jpeg",
        VideoCodec::H265Experimental => "h265",
    }
}

fn format_optional_bool(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "na",
    }
}

#[cfg(test)]
mod tests {
    use super::{BringupRenderSurface, IngressTimingSample, RenderSessionDescriptor};
    use crate::render::JpegFrame;
    use crate::video::{h264::H264AccessUnit, PreparedVideoIngress};
    use hscrcpy_contracts::VideoCodec;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("hscrcpy-render-{name}-{unique}"));
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

    fn session(codec: VideoCodec) -> RenderSessionDescriptor {
        RenderSessionDescriptor {
            session_id: format!("session-{}", codec_name(&codec)),
            selected_codec: codec,
            display_width: 1280,
            display_height: 720,
        }
    }

    #[test]
    fn writes_jpeg_frames_into_bringup_surface() {
        let temp = TestDir::new("jpeg");
        let mut surface = BringupRenderSurface::new(temp.path());
        surface
            .initialize_session(session(VideoCodec::Jpeg))
            .expect("render surface should initialize");

        let artifact = surface
            .present_video_ingress(&PreparedVideoIngress::Jpeg(JpegFrame {
                timestamp_micros: 123,
                encoded_bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            }))
            .expect("jpeg frame should render");

        assert_eq!(artifact.codec, VideoCodec::Jpeg);
        assert_eq!(surface.stats().jpeg_frames, 1);

        let session_dir = surface.session_dir().expect("session dir should exist");
        assert_eq!(
            fs::read(session_dir.join("latest.jpg")).expect("latest preview should exist"),
            vec![0xff, 0xd8, 0xff, 0xd9]
        );
        assert!(fs::read_to_string(session_dir.join("preview.html"))
            .expect("preview page should exist")
            .contains("latest.jpg"));
        assert_eq!(
            fs::read(
                artifact
                    .artifact_path
                    .as_ref()
                    .expect("jpeg artifact path should exist")
            )
            .expect("frame artifact should exist"),
            vec![0xff, 0xd8, 0xff, 0xd9]
        );
    }

    #[test]
    fn skips_h264_artifacts_by_default() {
        let temp = TestDir::new("h264-no-record");
        let mut surface = BringupRenderSurface::new(temp.path());
        surface
            .initialize_session(session(VideoCodec::H264))
            .expect("render surface should initialize");

        let artifact = surface
            .present_video_ingress(&PreparedVideoIngress::H264(H264AccessUnit {
                timestamp_micros: 456,
                is_keyframe: true,
                encoded_bytes: vec![1, 2, 3, 4],
            }))
            .expect("h264 access unit should render");

        assert_eq!(artifact.codec, VideoCodec::H264);
        assert_eq!(artifact.is_keyframe, Some(true));
        assert_eq!(artifact.artifact_path, None);
        assert_eq!(surface.stats().h264_access_units, 1);
        assert_eq!(surface.stats().last_artifact_path, None);

        let session_dir = surface.session_dir().expect("session dir should exist");
        assert!(
            !session_dir.join("stream.h264").exists(),
            "stream.h264 should be opt-in"
        );
        assert!(
            fs::read_to_string(session_dir.join("events.log"))
                .expect("events log should exist")
                .contains("keyframe=true artifact=disabled"),
            "events.log should record disabled artifact state"
        );
    }

    #[test]
    fn appends_h264_access_units_into_stream_artifacts_when_enabled() {
        let temp = TestDir::new("h264");
        let mut surface = BringupRenderSurface::new(temp.path()).with_h264_artifact_recording();
        surface
            .initialize_session(session(VideoCodec::H264))
            .expect("render surface should initialize");

        let artifact = surface
            .present_video_ingress(&PreparedVideoIngress::H264(H264AccessUnit {
                timestamp_micros: 456,
                is_keyframe: true,
                encoded_bytes: vec![1, 2, 3, 4],
            }))
            .expect("h264 access unit should render");

        assert_eq!(artifact.codec, VideoCodec::H264);
        assert_eq!(artifact.is_keyframe, Some(true));
        assert_eq!(surface.stats().h264_access_units, 1);

        let session_dir = surface.session_dir().expect("session dir should exist");
        assert_eq!(
            fs::read(
                artifact
                    .artifact_path
                    .as_ref()
                    .expect("h264 artifact path should exist")
            )
            .expect("access unit artifact should exist"),
            vec![1, 2, 3, 4]
        );
        assert_eq!(
            fs::read(session_dir.join("stream.h264")).expect("stream artifact should exist"),
            vec![0, 0, 0, 1, 1, 2, 3, 4]
        );
    }

    #[test]
    fn flushes_h264_timing_summary_with_ingress_samples() {
        let temp = TestDir::new("h264-diag");
        let mut surface = BringupRenderSurface::new(temp.path());
        surface
            .initialize_session(session(VideoCodec::H264))
            .expect("render surface should initialize");

        let base = Instant::now();
        for index in 0..3_u64 {
            let receive_started_at = base + Duration::from_millis(index * 16);
            let receive_completed_at = receive_started_at + Duration::from_micros(700);
            surface
                .present_video_ingress_with_timing(
                    &PreparedVideoIngress::H264(H264AccessUnit {
                        timestamp_micros: 1_000_000 + index * 16_000,
                        is_keyframe: index == 0,
                        encoded_bytes: vec![1, 2, 3, 4],
                    }),
                    IngressTimingSample::new(receive_started_at, receive_completed_at),
                )
                .expect("timed h264 access unit should render");
        }

        let summary = surface
            .flush_h264_diagnostic_summary()
            .expect("summary flush should succeed")
            .expect("summary should be produced");

        assert_eq!(summary.units, 3);
        assert_eq!(summary.full_window, false);
        assert_eq!(summary.pts_span_us, Some(32_000));
        assert_eq!(summary.ingress_wait_avg_us, Some(700));
        assert_eq!(summary.ingress_wait_max_us, Some(700));
        assert!(summary.host_receive_span_us.is_some());
        assert!(summary.host_present_span_us.is_some());

        let pending = surface
            .take_h264_diagnostic_summary()
            .expect("pending summary should be available");
        assert_eq!(pending.units, 3);

        let session_dir = surface.session_dir().expect("session dir should exist");
        assert!(
            fs::read_to_string(session_dir.join("events.log"))
                .expect("events log should exist")
                .contains("diag window=partial"),
            "events.log should include structured summary line"
        );
    }

    #[test]
    fn rejects_present_before_initialization() {
        let temp = TestDir::new("uninitialized");
        let mut surface = BringupRenderSurface::new(temp.path());
        let err = surface
            .present_video_ingress(&PreparedVideoIngress::Jpeg(JpegFrame {
                timestamp_micros: 1,
                encoded_bytes: vec![1, 2, 3],
            }))
            .expect_err("uninitialized surface should reject frames");

        assert!(
            err.to_string().contains("initialized"),
            "unexpected error: {err}"
        );
    }

    fn codec_name(codec: &VideoCodec) -> &'static str {
        match codec {
            VideoCodec::H264 => "h264",
            VideoCodec::Jpeg => "jpeg",
            VideoCodec::H265Experimental => "h265",
        }
    }
}
