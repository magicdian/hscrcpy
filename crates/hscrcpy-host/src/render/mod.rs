mod bringup;

use crate::HostResult;

pub use bringup::{
    BringupRenderSurface, RenderSessionDescriptor, RenderStreamStats, RenderedArtifact,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub timestamp_micros: u64,
}

pub trait RenderBackend {
    fn initialize(&mut self) -> HostResult<()>;
    fn present_frame(&mut self, frame: &VideoFrame) -> HostResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JpegFrame {
    pub timestamp_micros: u64,
    pub encoded_bytes: Vec<u8>,
}

pub trait JpegFrameSink {
    fn present_jpeg_frame(&mut self, frame: &JpegFrame) -> HostResult<()>;
}

pub struct StubRenderer;

impl RenderBackend for StubRenderer {
    fn initialize(&mut self) -> HostResult<()> {
        Ok(())
    }

    fn present_frame(&mut self, _frame: &VideoFrame) -> HostResult<()> {
        Ok(())
    }
}

pub struct StubJpegFrameSink;

impl JpegFrameSink for StubJpegFrameSink {
    fn present_jpeg_frame(&mut self, _frame: &JpegFrame) -> HostResult<()> {
        Ok(())
    }
}
