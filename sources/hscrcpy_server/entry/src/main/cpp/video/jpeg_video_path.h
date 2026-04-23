#ifndef HSCRCPY_SERVER_VIDEO_JPEG_VIDEO_PATH_H
#define HSCRCPY_SERVER_VIDEO_JPEG_VIDEO_PATH_H

#include <vector>

#include "core/companion_contracts.h"

namespace hscrcpy {
namespace video {
namespace jpeg {

core::VideoCodecDescriptor BuildJpegCodecDescriptor();
core::DisplayInfo ResolveSessionDisplay(const core::SessionConfigRequest &request);
core::ChannelLayout BuildChannelLayout();
core::VideoUnitPreview BuildVideoUnitPreview();
std::vector<std::string> BuildPipelineStages();
const std::vector<uint8_t> &GetPlaceholderJpegFrameBytes();

}  // namespace jpeg
}  // namespace video
}  // namespace hscrcpy

#endif
