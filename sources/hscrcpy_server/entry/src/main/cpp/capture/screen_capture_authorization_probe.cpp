#include "capture/screen_capture_authorization_probe.h"

#include <multimedia/player_framework/native_avscreen_capture.h>

#include "codec/h264_encoder_capability.h"
#include "core/companion_contracts.h"

namespace hscrcpy {
namespace capture {
namespace auth {

namespace {

static constexpr int32_t kDefaultVideoWidth = 1280;
static constexpr int32_t kDefaultVideoHeight = 720;
static constexpr int32_t kDefaultFrameRate = 30;
static constexpr int32_t kDefaultBitrate = 4 * 1000 * 1000;
static constexpr int32_t kDefaultAudioSampleRate = 48000;
static constexpr int32_t kDefaultAudioChannels = 2;
static constexpr int32_t kDefaultAudioBitrate = 128000;

std::string MapProbeResultToAuthorizationState(OH_AVSCREEN_CAPTURE_ErrCode result)
{
    if (result == AV_SCREEN_CAPTURE_ERR_OK) {
        return core::kAuthorizationGranted;
    }
    if (result == AV_SCREEN_CAPTURE_ERR_UNSUPPORT) {
        return core::kAuthorizationUnsupported;
    }
    if (result == AV_SCREEN_CAPTURE_ERR_OPERATE_NOT_PERMIT) {
        return core::kAuthorizationNeedsUserAction;
    }
    if (result == AV_SCREEN_CAPTURE_ERR_INVALID_VAL) {
        return core::kAuthorizationNeedsUserAction;
    }
    return core::kAuthorizationNeedsUserAction;
}

codec::h264::EncoderConfiguration BuildProbeEncoderConfiguration()
{
    codec::h264::EncoderConfiguration configuration = {};
    configuration.width = kDefaultVideoWidth;
    configuration.height = kDefaultVideoHeight;
    configuration.frame_rate = kDefaultFrameRate;
    configuration.bitrate = kDefaultBitrate;
    configuration.hardware = false;
    configuration.codec_name = "probe-default";

    core::VideoTransportState probe_state = {};
    probe_state.display.width = kDefaultVideoWidth;
    probe_state.display.height = kDefaultVideoHeight;

    codec::h264::EncoderConfiguration resolved = {};
    std::string resolve_error;
    if (codec::h264::ResolveEncoderConfiguration(probe_state, &resolved, &resolve_error)) {
        configuration = resolved;
    }
    return configuration;
}

OH_AVScreenCaptureConfig BuildProbeConfig(const codec::h264::EncoderConfiguration &encoder_configuration)
{
    OH_AVScreenCaptureConfig config = {};
    config.captureMode = OH_CAPTURE_HOME_SCREEN;
    config.dataType = OH_ORIGINAL_STREAM;
    config.audioInfo.micCapInfo.audioSampleRate = kDefaultAudioSampleRate;
    config.audioInfo.micCapInfo.audioChannels = kDefaultAudioChannels;
    config.audioInfo.micCapInfo.audioSource = OH_SOURCE_DEFAULT;
    config.audioInfo.innerCapInfo.audioSampleRate = kDefaultAudioSampleRate;
    config.audioInfo.innerCapInfo.audioChannels = kDefaultAudioChannels;
    config.audioInfo.innerCapInfo.audioSource = OH_SOURCE_DEFAULT;
    config.audioInfo.audioEncInfo.audioBitrate = kDefaultAudioBitrate;
    config.audioInfo.audioEncInfo.audioCodecformat = OH_AAC_LC;
    config.videoInfo.videoCapInfo.videoFrameWidth = encoder_configuration.width;
    config.videoInfo.videoCapInfo.videoFrameHeight = encoder_configuration.height;
    config.videoInfo.videoCapInfo.videoSource = OH_VIDEO_SOURCE_SURFACE_RGBA;
    config.videoInfo.videoEncInfo.videoCodec = OH_H264;
    config.videoInfo.videoEncInfo.videoBitrate = encoder_configuration.bitrate;
    config.videoInfo.videoEncInfo.videoFrameRate = encoder_configuration.frame_rate;
    return config;
}

}  // namespace

std::string ProbeVideoCaptureAuthorizationState()
{
    OH_AVScreenCapture *capture = OH_AVScreenCapture_Create();
    if (capture == nullptr) {
        return core::kAuthorizationNeedsUserAction;
    }

    const codec::h264::EncoderConfiguration encoder_configuration = BuildProbeEncoderConfiguration();
    const OH_AVSCREEN_CAPTURE_ErrCode init_result = OH_AVScreenCapture_Init(capture, BuildProbeConfig(encoder_configuration));
    if (init_result != AV_SCREEN_CAPTURE_ERR_OK) {
        (void)OH_AVScreenCapture_Release(capture);
        return MapProbeResultToAuthorizationState(init_result);
    }

    (void)OH_AVScreenCapture_SetMicrophoneEnabled(capture, false);

    const OH_AVSCREEN_CAPTURE_ErrCode start_result = OH_AVScreenCapture_StartScreenCapture(capture);
    if (start_result == AV_SCREEN_CAPTURE_ERR_OK) {
        (void)OH_AVScreenCapture_StopScreenCapture(capture);
    }

    (void)OH_AVScreenCapture_Release(capture);
    return MapProbeResultToAuthorizationState(start_result);
}

}  // namespace auth
}  // namespace capture
}  // namespace hscrcpy
