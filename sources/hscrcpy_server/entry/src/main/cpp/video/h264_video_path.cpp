#include "video/h264_video_path.h"

#include "video/video_path_utils.h"

namespace hscrcpy {
namespace video {
namespace h264 {

namespace {

static constexpr int kH264MaxFps = 120;

}  // namespace

core::VideoCodecDescriptor BuildH264CodecDescriptor()
{
    const core::DisplayInfo display = video::BuildDeviceDisplayInfo();
    return {
        core::kVideoCodecH264,
        core::kEncoderKindHardware,
        display.width,
        display.height,
        kH264MaxFps,
        "vbr"
    };
}

core::DisplayInfo ResolveSessionDisplay(const core::SessionConfigRequest &request)
{
    return video::ResolveSessionDisplay(request, BuildH264CodecDescriptor());
}

core::ChannelLayout BuildChannelLayout()
{
    return video::BuildChannelLayout("opens after session_ready for H.264 access-unit delivery");
}

core::VideoUnitPreview BuildVideoUnitPreview()
{
    return {
        core::kVideoCodecH264,
        {"codec", "pts_us", "is_keyframe", "payload_length"},
        "one H.264 access unit per binary payload unit on the video channel",
        "keyframes follow host-selected iframe_interval_ms when provided, otherwise the platform default cadence",
        "payload bytes stay provisional, but every unit still carries the host-device contract fields"
    };
}

std::vector<std::string> BuildPipelineStages()
{
    return {
        "codec/h264_encoder_capability",
        "capture/h264_screen_capture_source",
        "video/h264_video_path",
        "transport/session_channel",
        "transport/video_channel"
    };
}

}  // namespace h264
}  // namespace video
}  // namespace hscrcpy
