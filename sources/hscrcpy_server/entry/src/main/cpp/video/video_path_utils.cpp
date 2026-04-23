#include "video/video_path_utils.h"

#include <algorithm>

namespace hscrcpy {
namespace video {

namespace {

static constexpr int kDeviceDisplayWidth = 1920;
static constexpr int kDeviceDisplayHeight = 1080;
static constexpr int kDeviceDisplayRotation = 0;

int ClampToPositiveLimit(int value, int limit)
{
    return std::max(1, std::min(value, limit));
}

}  // namespace

core::DisplayInfo BuildDeviceDisplayInfo()
{
    return {
        kDeviceDisplayWidth,
        kDeviceDisplayHeight,
        kDeviceDisplayRotation
    };
}

core::DisplayInfo ResolveSessionDisplay(
    const core::SessionConfigRequest &request, const core::VideoCodecDescriptor &capability)
{
    return {
        ClampToPositiveLimit(request.video_max_width, capability.max_width),
        ClampToPositiveLimit(request.video_max_height, capability.max_height),
        kDeviceDisplayRotation
    };
}

core::ChannelLayout BuildChannelLayout(const std::string &video_activation)
{
    return {
        {
            core::kChannelNameSession,
            core::kChannelStateReady,
            core::kPayloadTypeUtf8Json,
            "opened before session_config and kept for control plus teardown"
        },
        {
            core::kChannelNameVideo,
            core::kChannelStatePendingOpen,
            core::kPayloadTypeBinary,
            video_activation
        }
    };
}

}  // namespace video
}  // namespace hscrcpy
