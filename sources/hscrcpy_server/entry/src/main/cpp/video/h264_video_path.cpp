#include "video/h264_video_path.h"

#include "video/video_path_utils.h"

namespace hscrcpy {
namespace video {
namespace h264 {

namespace {

static constexpr int kH264MaxFps = 60;

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
        "capture/display_capture",
        "video/h264_video_path",
        "transport/session_channel",
        "transport/video_channel"
    };
}

const std::vector<uint8_t> &GetPlaceholderH264AccessUnitBytes()
{
    static const std::vector<uint8_t> kPlaceholderH264AccessUnit = {
        0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x1E, 0xAC, 0xD9, 0x40, 0xA0,
        0x2F, 0xF9, 0x70, 0x11, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x00, 0x03,
        0x00, 0x3C, 0x8F, 0x16, 0x2E, 0x48, 0x00, 0x00, 0x00, 0x01, 0x68, 0xEE,
        0x3C, 0x80, 0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00
    };
    return kPlaceholderH264AccessUnit;
}

}  // namespace h264
}  // namespace video
}  // namespace hscrcpy
