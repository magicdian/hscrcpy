#ifndef HSCRCPY_SERVER_VIDEO_H264_VIDEO_PATH_H
#define HSCRCPY_SERVER_VIDEO_H264_VIDEO_PATH_H

#include <vector>

#include "core/companion_contracts.h"

namespace hscrcpy {
namespace video {
namespace h264 {

core::VideoCodecDescriptor BuildH264CodecDescriptor();
core::DisplayInfo ResolveSessionDisplay(const core::SessionConfigRequest &request);
core::ChannelLayout BuildChannelLayout();
core::VideoUnitPreview BuildVideoUnitPreview();
std::vector<std::string> BuildPipelineStages();

}  // namespace h264
}  // namespace video
}  // namespace hscrcpy

#endif
