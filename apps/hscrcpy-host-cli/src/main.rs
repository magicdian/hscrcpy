use hscrcpy_contracts::{SessionStartRequest, VideoCodec};
use hscrcpy_host::companion::HdcCompanionManager;
use hscrcpy_host::hdc::RuntimeHdcBridge;
use hscrcpy_host::render::{BringupRenderSurface, RenderSessionDescriptor};
use hscrcpy_host::session::{SessionBootstrap, SessionOrchestrator};
use hscrcpy_host::HostResult;
use std::env;
use std::path::PathBuf;
use std::process;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliOptions {
    device_id: String,
    requested_codec: VideoCodec,
    preferred_max_fps: u16,
    enable_control: bool,
    max_frames: usize,
    output_dir: PathBuf,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            device_id: "auto".to_string(),
            requested_codec: VideoCodec::H264,
            preferred_max_fps: 60,
            enable_control: true,
            max_frames: 120,
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
    let mut surface = BringupRenderSurface::new(&options.output_dir);
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
    if let Some(path) = surface.preview_html_path() {
        println!("preview surface: {}", path.display());
    }

    let capture_result = capture_frames(&mut plan.runtime, &mut surface, options.max_frames);
    let stop_reason = format!(
        "render bringup finished after {} unit(s)",
        surface.stats().total_units
    );
    let stop_result = plan.runtime.stop(Some(stop_reason));

    capture_result?;
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
    max_frames: usize,
) -> HostResult<()> {
    for _ in 0..max_frames {
        let ingress = runtime.receive_video_ingress()?;
        let artifact = surface.present_video_ingress(&ingress)?;
        if artifact.frame_index == 1 || artifact.frame_index % 30 == 0 {
            println!(
                "captured frame {} codec={} pts_us={} artifact={}",
                artifact.frame_index,
                codec_name(&artifact.codec),
                artifact.timestamp_micros,
                artifact.artifact_path.display()
            );
        }
    }
    Ok(())
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
                options.max_frames = next_value(&mut args, "--max-frames")?
                    .parse()
                    .map_err(|error| format!("invalid --max-frames value: {error}"))?;
            }
            "--output-dir" => {
                options.output_dir = PathBuf::from(next_value(&mut args, "--output-dir")?);
            }
            "--no-control" => {
                options.enable_control = false;
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
    "Usage: hscrcpy-host-cli [--device <id>] [--codec <h264|jpeg>] [--fps <n>] [--max-frames <n>] [--output-dir <path>] [--no-control]\n\
\n\
Starts a host session, captures negotiated video ingress, and writes bring-up render artifacts.\n\
\n\
Examples:\n\
  hscrcpy-host-cli --device auto --codec h264 --max-frames 120\n\
  hscrcpy-host-cli --device 192.168.0.10:5555 --codec jpeg --output-dir /tmp/hscrcpy-preview"
}
