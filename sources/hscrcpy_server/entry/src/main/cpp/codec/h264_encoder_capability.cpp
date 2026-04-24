#include "codec/h264_encoder_capability.h"

#include <algorithm>
#include <sstream>

#include <multimedia/player_framework/native_avcapability.h>

namespace hscrcpy {
namespace codec {
namespace h264 {

namespace {

static constexpr int32_t kDefaultWidth = 1280;
static constexpr int32_t kDefaultHeight = 720;
static constexpr int32_t kDefaultFrameRate = 30;
static constexpr int32_t kDefaultBitrate = 8 * 1000 * 1000;

int32_t MaxInt(int32_t left, int32_t right)
{
    return left > right ? left : right;
}

int32_t ClampToRange(int32_t value, const OH_AVRange &range)
{
    return std::max(range.minVal, std::min(value, range.maxVal));
}

int32_t AlignToLowerBoundary(int32_t value, int32_t alignment)
{
    if (alignment <= 1) {
        return value;
    }
    const int32_t truncated = (value / alignment) * alignment;
    return truncated > 0 ? truncated : alignment;
}

void AssignError(std::string *error, const std::string &operation, const std::string &message)
{
    if (error == nullptr) {
        return;
    }
    *error = operation + ": " + message;
}

OH_AVCapability *ResolveCapability(bool *hardware)
{
    OH_AVCapability *capability =
        OH_AVCodec_GetCapabilityByCategory(OH_AVCODEC_MIMETYPE_VIDEO_AVC, true, HARDWARE);
    if (capability != nullptr) {
        if (hardware != nullptr) {
            *hardware = true;
        }
        return capability;
    }

    capability = OH_AVCodec_GetCapabilityByCategory(OH_AVCODEC_MIMETYPE_VIDEO_AVC, true, SOFTWARE);
    if (capability != nullptr) {
        if (hardware != nullptr) {
            *hardware = false;
        }
        return capability;
    }

    capability = OH_AVCodec_GetCapability(OH_AVCODEC_MIMETYPE_VIDEO_AVC, true);
    if (capability != nullptr && hardware != nullptr) {
        *hardware = OH_AVCapability_IsHardware(capability);
    }
    return capability;
}

}  // namespace

bool ResolveEncoderConfiguration(
    const core::VideoTransportState &state, EncoderConfiguration *configuration, std::string *error)
{
    static constexpr const char *kOperation = "codec/h264_encoder_capability.resolve";
    if (configuration == nullptr) {
        AssignError(error, kOperation, "output configuration must not be null");
        return false;
    }

    bool hardware = false;
    OH_AVCapability *capability = ResolveCapability(&hardware);
    if (capability == nullptr) {
        AssignError(error, kOperation, "AVCodec has no available H.264 encoder capability");
        return false;
    }

    const char *capability_name = OH_AVCapability_GetName(capability);
    configuration->codec_name = capability_name != nullptr ? capability_name : "unknown";
    configuration->hardware = hardware;

    int32_t width = MaxInt(state.display.width, 2);
    int32_t height = MaxInt(state.display.height, 2);
    if (width == 2) {
        width = kDefaultWidth;
    }
    if (height == 2) {
        height = kDefaultHeight;
    }

    OH_AVRange width_range = {};
    OH_AVRange height_range = {};
    if (OH_AVCapability_GetVideoWidthRange(capability, &width_range) == AV_ERR_OK &&
        OH_AVCapability_GetVideoHeightRange(capability, &height_range) == AV_ERR_OK) {
        width = ClampToRange(width, width_range);
        height = ClampToRange(height, height_range);
    }

    int32_t width_alignment = 0;
    int32_t height_alignment = 0;
    if (OH_AVCapability_GetVideoWidthAlignment(capability, &width_alignment) == AV_ERR_OK) {
        width = AlignToLowerBoundary(width, width_alignment);
    }
    if (OH_AVCapability_GetVideoHeightAlignment(capability, &height_alignment) == AV_ERR_OK) {
        height = AlignToLowerBoundary(height, height_alignment);
    }
    width = AlignToLowerBoundary(width, 2);
    height = AlignToLowerBoundary(height, 2);

    if (!OH_AVCapability_IsVideoSizeSupported(capability, width, height)) {
        std::ostringstream stream;
        stream << "requested size " << width << "x" << height
               << " is not supported by AVCodec capability `" << configuration->codec_name << "`";
        AssignError(error, kOperation, stream.str());
        return false;
    }

    int32_t frame_rate = kDefaultFrameRate;
    OH_AVRange frame_rate_range = {};
    OH_AVErrCode frame_range_result =
        OH_AVCapability_GetVideoFrameRateRangeForSize(capability, width, height, &frame_rate_range);
    if (frame_range_result != AV_ERR_OK) {
        frame_range_result = OH_AVCapability_GetVideoFrameRateRange(capability, &frame_rate_range);
    }
    if (frame_range_result == AV_ERR_OK) {
        frame_rate = ClampToRange(frame_rate, frame_rate_range);
    }

    int32_t bitrate = kDefaultBitrate;
    OH_AVRange bitrate_range = {};
    if (OH_AVCapability_GetEncoderBitrateRange(capability, &bitrate_range) == AV_ERR_OK) {
        bitrate = ClampToRange(bitrate, bitrate_range);
    }

    configuration->width = width;
    configuration->height = height;
    configuration->frame_rate = frame_rate;
    configuration->bitrate = bitrate;
    return true;
}

}  // namespace h264
}  // namespace codec
}  // namespace hscrcpy
