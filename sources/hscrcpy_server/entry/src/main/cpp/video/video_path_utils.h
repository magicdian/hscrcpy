#ifndef HSCRCPY_SERVER_VIDEO_VIDEO_PATH_UTILS_H
#define HSCRCPY_SERVER_VIDEO_VIDEO_PATH_UTILS_H

#include <string>

#include "core/companion_contracts.h"

namespace hscrcpy {
namespace video {

core::DisplayInfo BuildDeviceDisplayInfo();
core::DisplayInfo ResolveSessionDisplay(
    const core::SessionConfigRequest &request, const core::VideoCodecDescriptor &capability);
core::ChannelLayout BuildChannelLayout(const std::string &video_activation);

}  // namespace video
}  // namespace hscrcpy

#endif
