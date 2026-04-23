use super::{JpegFrame, JpegFrameSink};
use crate::video::{h264::H264AccessUnit, PreparedVideoIngress};
use crate::{HostError, HostResult};
use hscrcpy_contracts::VideoCodec;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const H264_ANNEX_B_START_CODE: [u8; 4] = [0, 0, 0, 1];

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
    pub artifact_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BringupRenderSurface {
    output_root: PathBuf,
    session: Option<RenderSessionDescriptor>,
    session_dir: Option<PathBuf>,
    frames_dir: Option<PathBuf>,
    stats: RenderStreamStats,
}

impl BringupRenderSurface {
    pub fn new(output_root: impl Into<PathBuf>) -> Self {
        Self {
            output_root: output_root.into(),
            session: None,
            session_dir: None,
            frames_dir: None,
            stats: RenderStreamStats::default(),
        }
    }

    pub fn initialize_session(&mut self, session: RenderSessionDescriptor) -> HostResult<()> {
        let session_dir = self.output_root.join(&session.session_id);
        let frames_dir = session_dir.join("frames");
        fs::create_dir_all(&frames_dir)
            .map_err(|error| io_error("create render output directories", &frames_dir, error))?;
        write_text_file(
            &session_dir.join("session.txt"),
            &render_session_report(&session),
        )?;
        write_text_file(
            &session_dir.join("preview.html"),
            &render_preview_html(&session),
        )?;
        write_text_file(&session_dir.join("events.log"), "")?;

        self.stats = RenderStreamStats::default();
        self.session = Some(session);
        self.session_dir = Some(session_dir);
        self.frames_dir = Some(frames_dir);
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

    pub fn stats(&self) -> &RenderStreamStats {
        &self.stats
    }

    pub fn present_video_ingress(
        &mut self,
        ingress: &PreparedVideoIngress,
    ) -> HostResult<RenderedArtifact> {
        match ingress {
            PreparedVideoIngress::H264(access_unit) => self.present_h264_access_unit(access_unit),
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
    ) -> HostResult<RenderedArtifact> {
        let session_dir = self.require_session_dir()?;
        let frame_index = self.stats.total_units + 1;
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

        self.record_artifact(
            frame_index,
            VideoCodec::H264,
            access_unit.timestamp_micros,
            frame_path.clone(),
        )?;
        self.stats.h264_access_units += 1;
        Ok(RenderedArtifact {
            frame_index,
            codec: VideoCodec::H264,
            timestamp_micros: access_unit.timestamp_micros,
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
            frame_path.clone(),
        )?;
        self.stats.jpeg_frames += 1;
        Ok(RenderedArtifact {
            frame_index,
            codec: VideoCodec::Jpeg,
            timestamp_micros: frame.timestamp_micros,
            artifact_path: frame_path,
        })
    }

    fn record_artifact(
        &mut self,
        frame_index: usize,
        codec: VideoCodec,
        timestamp_micros: u64,
        artifact_path: PathBuf,
    ) -> HostResult<()> {
        let event_path = self.require_session_dir()?.join("events.log");
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&event_path)
            .map_err(|error| io_error("open render event log", &event_path, error))?;
        writeln!(
            log,
            "frame={} codec={} pts_us={} artifact={}",
            frame_index,
            codec_name(&codec),
            timestamp_micros,
            artifact_path.display()
        )
        .map_err(|error| io_error("append render event log", &event_path, error))?;

        self.stats.total_units = frame_index;
        self.stats.last_timestamp_micros = Some(timestamp_micros);
        self.stats.last_artifact_path = Some(artifact_path);
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

impl JpegFrameSink for BringupRenderSurface {
    fn present_jpeg_frame(&mut self, frame: &JpegFrame) -> HostResult<()> {
        self.write_jpeg_frame(frame).map(|_| ())
    }
}

fn render_session_report(session: &RenderSessionDescriptor) -> String {
    format!(
        "session_id={}\nselected_codec={}\ndisplay_width={}\ndisplay_height={}\npreview_html=preview.html\njpeg_latest=latest.jpg\nh264_stream=stream.h264\n",
        session.session_id,
        codec_name(&session.selected_codec),
        session.display_width,
        session.display_height,
    )
}

fn render_preview_html(session: &RenderSessionDescriptor) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta http-equiv=\"refresh\" content=\"1\">\n<title>hscrcpy Render Bringup</title>\n<style>\nbody {{ font-family: ui-monospace, monospace; margin: 24px; background: #111; color: #f5f5f5; }}\nmain {{ max-width: 960px; margin: 0 auto; }}\nimg {{ width: 100%; max-width: 100%; border: 1px solid #444; background: #000; }}\np {{ line-height: 1.5; }}\ncode {{ color: #8be9fd; }}\n</style>\n</head>\n<body>\n<main>\n<h1>hscrcpy host render bringup</h1>\n<p>Session: <code>{}</code></p>\n<p>Negotiated codec: <code>{}</code></p>\n<p>Display hint: <code>{}x{}</code></p>\n<p>This preview auto-refreshes once per second. JPEG fallback updates <code>latest.jpg</code> directly. H264 mainline writes per-access-unit artifacts under <code>frames/</code> and appends an Annex-B-style stream to <code>stream.h264</code> for decoder bringup.</p>\n<img src=\"latest.jpg\" alt=\"Waiting for JPEG fallback frame\">\n</main>\n</body>\n</html>\n",
        session.session_id,
        codec_name(&session.selected_codec),
        session.display_width,
        session.display_height,
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

#[cfg(test)]
mod tests {
    use super::{BringupRenderSurface, RenderSessionDescriptor};
    use crate::render::JpegFrame;
    use crate::video::{h264::H264AccessUnit, PreparedVideoIngress};
    use hscrcpy_contracts::VideoCodec;
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
            fs::read(&artifact.artifact_path).expect("frame artifact should exist"),
            vec![0xff, 0xd8, 0xff, 0xd9]
        );
    }

    #[test]
    fn appends_h264_access_units_into_stream_artifacts() {
        let temp = TestDir::new("h264");
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
        assert_eq!(surface.stats().h264_access_units, 1);

        let session_dir = surface.session_dir().expect("session dir should exist");
        assert_eq!(
            fs::read(&artifact.artifact_path).expect("access unit artifact should exist"),
            vec![1, 2, 3, 4]
        );
        assert_eq!(
            fs::read(session_dir.join("stream.h264")).expect("stream artifact should exist"),
            vec![0, 0, 0, 1, 1, 2, 3, 4]
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
