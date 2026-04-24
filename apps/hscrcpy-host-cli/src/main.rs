use hscrcpy_contracts::{SessionStartRequest, VideoCodec};
use hscrcpy_host::companion::HdcCompanionManager;
use hscrcpy_host::hdc::RuntimeHdcBridge;
use hscrcpy_host::render::{
    BringupDiagnosticsSummary, BringupRenderSurface, H264LivePreviewConfig, IngressTimingSample,
    RenderSessionDescriptor,
};
use hscrcpy_host::session::{SessionBootstrap, SessionOrchestrator};
use hscrcpy_host::HostResult;
use std::env;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliOptions {
    device_id: String,
    requested_codec: VideoCodec,
    preferred_max_fps: u16,
    enable_control: bool,
    max_frames: Option<usize>,
    live_preview: bool,
    ffplay_bin: String,
    output_dir: PathBuf,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            device_id: "auto".to_string(),
            requested_codec: VideoCodec::H264,
            preferred_max_fps: 60,
            enable_control: true,
            max_frames: None,
            live_preview: true,
            ffplay_bin: "ffplay".to_string(),
            output_dir: PathBuf::from("target/host-render-bringup"),
        }
    }
}

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = parse_args(env::args().skip(1))?;
    run_bringup(options).map_err(|error| error.to_string())
}

fn run_bringup(options: CliOptions) -> HostResult<()> {
    let orchestrator = SessionOrchestrator::new(
        RuntimeHdcBridge::new(),
        HdcCompanionManager::new(RuntimeHdcBridge::new()),
    );
    let request = match options.requested_codec {
        VideoCodec::H264 => {
            SessionStartRequest::h264_mainline(options.preferred_max_fps, options.enable_control)
        }
        VideoCodec::Jpeg => {
            SessionStartRequest::jpeg_baseline(options.preferred_max_fps, options.enable_control)
        }
        VideoCodec::H265Experimental => {
            return Err(hscrcpy_host::HostError::ContractViolation(
                "host render bringup only supports h264 or jpeg requests".to_string(),
            ))
        }
    };
    let bootstrap = SessionBootstrap::new(&options.device_id, request);
    let mut plan = orchestrator.start(&bootstrap)?;
    let mut surface = if options.live_preview && options.requested_codec == VideoCodec::H264 {
        BringupRenderSurface::new(&options.output_dir).with_h264_live_preview(
            H264LivePreviewConfig {
                ffplay_bin: options.ffplay_bin.clone(),
            },
        )
    } else {
        BringupRenderSurface::new(&options.output_dir)
    };
    surface.initialize_session(RenderSessionDescriptor {
        session_id: plan.response.session_id.clone(),
        selected_codec: plan.response.selected_codec.clone(),
        display_width: u32::from(plan.negotiation.session_ready.display.width),
        display_height: u32::from(plan.negotiation.session_ready.display.height),
    })?;

    println!(
        "session {} started with codec {}",
        plan.response.session_id,
        codec_name(&plan.response.selected_codec)
    );
    if let Some(status) = surface.live_preview_status() {
        println!("live preview: {status}");
    }
    if let Some(path) = surface.preview_html_path() {
        println!("preview surface: {}", path.display());
    }
    if let Some(session_dir) = surface.session_dir() {
        println!(
            "diagnostic log: {}",
            session_dir.join("events.log").display()
        );
    }

    let capture_result = capture_frames(&mut plan.runtime, &mut surface, options.max_frames);
    let summary_flush_result = surface.flush_h264_diagnostic_summary();
    let stop_reason = format!(
        "render bringup finished after {} unit(s)",
        surface.stats().total_units
    );
    let stop_result = plan.runtime.stop(Some(stop_reason));

    capture_result?;
    if let Some(summary) = summary_flush_result? {
        print_diagnostics_summary(&summary);
    }
    stop_result?;

    let stats = surface.stats();
    println!(
        "captured {} unit(s): {} jpeg, {} h264",
        stats.total_units, stats.jpeg_frames, stats.h264_access_units
    );
    if let Some(path) = &stats.last_artifact_path {
        println!("latest artifact: {}", path.display());
    }
    Ok(())
}

fn capture_frames(
    runtime: &mut hscrcpy_host::session::SessionRuntime,
    surface: &mut BringupRenderSurface,
    max_frames: Option<usize>,
) -> HostResult<()> {
    let mut captured = 0usize;
    loop {
        if let Some(limit) = max_frames {
            if captured >= limit {
                break;
            }
        }
        let receive_started_at = Instant::now();
        let ingress = runtime.receive_video_ingress()?;
        let receive_completed_at = Instant::now();
        let artifact = surface.present_video_ingress_with_timing(
            &ingress,
            IngressTimingSample::new(receive_started_at, receive_completed_at),
        )?;
        captured += 1;
        if artifact.frame_index == 1 {
            println!(
                "captured frame {} codec={} pts_us={} artifact={}",
                artifact.frame_index,
                codec_name(&artifact.codec),
                artifact.timestamp_micros,
                artifact.artifact_path.display()
            );
        }
        if let Some(summary) = surface.take_h264_diagnostic_summary() {
            print_diagnostics_summary(&summary);
        }
    }
    Ok(())
}

fn print_diagnostics_summary(summary: &BringupDiagnosticsSummary) {
    let format_u64 =
        |value: Option<u64>| value.map_or_else(|| "na".to_string(), |raw| raw.to_string());
    let format_i128 =
        |value: Option<i128>| value.map_or_else(|| "na".to_string(), |raw| raw.to_string());
    println!(
        "diag window={} units={} frame={}..{} pts_span_us={} host_rx_span_us={} host_present_span_us={} host_rx_minus_pts_us={} host_present_minus_pts_us={} ingress_wait_avg_us={} ingress_wait_max_us={} present_avg_us={} present_max_us={} ffplay_write_avg_us={} ffplay_write_max_us={} ffplay_slow_writes={}/{}",
        if summary.full_window { "full" } else { "partial" },
        summary.units,
        summary.first_frame_index,
        summary.last_frame_index,
        format_u64(summary.pts_span_us),
        format_u64(summary.host_receive_span_us),
        format_u64(summary.host_present_span_us),
        format_i128(summary.host_receive_minus_pts_us),
        format_i128(summary.host_present_minus_pts_us),
        format_u64(summary.ingress_wait_avg_us),
        format_u64(summary.ingress_wait_max_us),
        summary.present_avg_us,
        summary.present_max_us,
        format_u64(summary.ffplay_write_avg_us),
        format_u64(summary.ffplay_write_max_us),
        summary.ffplay_slow_writes,
        summary.ffplay_write_samples,
    );
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<CliOptions, String> {
    let mut options = CliOptions::default();
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            "--device" => {
                options.device_id = next_value(&mut args, "--device")?;
            }
            "--codec" => {
                options.requested_codec = parse_codec(&next_value(&mut args, "--codec")?)?;
            }
            "--fps" => {
                options.preferred_max_fps = next_value(&mut args, "--fps")?
                    .parse()
                    .map_err(|error| format!("invalid --fps value: {error}"))?;
            }
            "--max-frames" => {
                options.max_frames = Some(
                    next_value(&mut args, "--max-frames")?
                        .parse()
                        .map_err(|error| format!("invalid --max-frames value: {error}"))?,
                );
            }
            "--ffplay-bin" => {
                options.ffplay_bin = next_value(&mut args, "--ffplay-bin")?;
            }
            "--output-dir" => {
                options.output_dir = PathBuf::from(next_value(&mut args, "--output-dir")?);
            }
            "--no-control" => {
                options.enable_control = false;
            }
            "--no-live-preview" => {
                options.live_preview = false;
            }
            unexpected => {
                return Err(format!(
                    "unrecognized argument `{unexpected}`\n\n{}",
                    usage_text()
                ))
            }
        }
    }

    Ok(options)
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for {flag}\n\n{}", usage_text()))
}

fn parse_codec(raw: &str) -> Result<VideoCodec, String> {
    match raw {
        "h264" => Ok(VideoCodec::H264),
        "jpeg" => Ok(VideoCodec::Jpeg),
        other => Err(format!(
            "unsupported codec `{other}`; expected `h264` or `jpeg`"
        )),
    }
}

fn codec_name(codec: &VideoCodec) -> &'static str {
    match codec {
        VideoCodec::H264 => "h264",
        VideoCodec::Jpeg => "jpeg",
        VideoCodec::H265Experimental => "h265",
    }
}

fn print_usage() {
    println!("{}", usage_text());
}

fn usage_text() -> &'static str {
    "Usage: hscrcpy-host-cli [--device <id>] [--codec <h264|jpeg>] [--fps <n>] [--max-frames <n>] [--output-dir <path>] [--ffplay-bin <path>] [--no-control] [--no-live-preview]\n\
\n\
Starts a host session, captures negotiated video ingress, writes bring-up render artifacts, and streams H.264 mainline packets into a realtime ffplay preview by default.\n\
\n\
Examples:\n\
  hscrcpy-host-cli --device auto --codec h264\n\
  hscrcpy-host-cli --device auto --codec h264 --max-frames 120\n\
  hscrcpy-host-cli --device auto --codec h264 --ffplay-bin /opt/homebrew/bin/ffplay\n\
  hscrcpy-host-cli --device 192.168.0.10:5555 --codec jpeg --output-dir /tmp/hscrcpy-preview --no-live-preview\n\
\n\
By default the host runs continuously until interrupted. Use --max-frames to stop after a fixed number of video units."
}
