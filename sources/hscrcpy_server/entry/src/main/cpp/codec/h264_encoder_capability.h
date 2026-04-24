#ifndef HSCRCPY_SERVER_CODEC_H264_ENCODER_CAPABILITY_H
#define HSCRCPY_SERVER_CODEC_H264_ENCODER_CAPABILITY_H

#include <cstdint>
#include <string>

#include "core/companion_contracts.h"

namespace hscrcpy {
namespace codec {
namespace h264 {

struct EncoderConfiguration {
    int32_t width;
    int32_t height;
    int32_t frame_rate;
    int32_t bitrate;
    bool hardware;
    std::string codec_name;
};

bool ResolveEncoderConfiguration(
    const core::VideoTransportState &state, EncoderConfiguration *configuration, std::string *error);

}  // namespace h264
}  // namespace codec
}  // namespace hscrcpy

#endif
