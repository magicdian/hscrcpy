#include "capture/h264_screen_capture_source.h"

#include <algorithm>
#include <chrono>
#include <cstdlib>
#include <sstream>

#include <multimedia/player_framework/native_avcodec_videoencoder.h>
#include <multimedia/player_framework/native_avformat.h>
#include <multimedia/player_framework/native_avscreen_capture.h>

#include "codec/h264_encoder_capability.h"
#include "core/native_diag_log.h"

namespace hscrcpy {
namespace capture {
namespace h264 {

namespace {

static constexpr uint64_t kMaxQueuedAccessUnits = 8;
static constexpr int32_t kDefaultAudioSampleRate = 48000;
static constexpr int32_t kDefaultAudioChannels = 2;
static constexpr int32_t kDefaultAudioBitrate = 128000;
static constexpr int32_t kDefaultIFrameIntervalMs = 2000;
static constexpr int32_t kRepeatPreviousFrameAfterUs = 33000;
static constexpr int32_t kRepeatPreviousMaxCount = 1000000;
static constexpr int64_t kNanosecondsPerMicrosecond = 1000;
static constexpr uint64_t kCallbackSummaryEveryAccessUnits = 120;
static constexpr uint64_t kStartupAccessUnitTraceLimit = 5;
static constexpr int64_t kCallbackGapWarnUs = 200000;
static constexpr int64_t kTimelineSkewWarnUs = 120000;
static constexpr uint64_t kAnomalyWarnThrottleAccessUnits = 120;
static constexpr size_t kLargePayloadWarnBytes = 512 * 1024;
static constexpr size_t kNalTypesTraceLimit = 16;

struct NalInspection {
    uint64_t nal_count = 0;
    uint64_t sps_count = 0;
    uint64_t pps_count = 0;
    uint64_t idr_count = 0;
    std::string nal_types;
};

struct CallbackSummarySnapshot {
    uint64_t callback_count = 0;
    uint64_t access_unit_count = 0;
    uint64_t keyframe_count = 0;
    uint64_t codec_config_count = 0;
    uint64_t total_payload_bytes = 0;
    uint64_t dropped_units = 0;
    uint64_t max_queue_depth = 0;
    uint64_t non_monotonic_pts_count = 0;
    uint64_t callback_gap_count = 0;
    uint64_t timeline_skew_count = 0;
    uint64_t stream_changed_count = 0;
    uint64_t stream_changed_config_count = 0;
    uint64_t codec_config_parse_failures = 0;
    uint64_t access_units_with_config = 0;
    uint64_t idr_with_config_count = 0;
    uint64_t idr_with_prepended_config_count = 0;
    uint64_t first_keyframe_index = 0;
    int64_t first_access_unit_after_start_us = -1;
    int64_t first_keyframe_after_start_us = -1;
    uint64_t pts_step_samples = 0;
    uint64_t callback_step_samples = 0;
    uint64_t avg_pts_step_us = 0;
    uint64_t avg_callback_step_us = 0;
    uint64_t queue_depth = 0;
};

struct CallbackAnomalySnapshot {
    const char *kind = "";
    uint64_t access_unit_index = 0;
    int64_t pts_us = 0;
    int64_t delta_pts_us = 0;
    int64_t callback_wall_us = 0;
    int64_t delta_wall_us = 0;
    size_t payload_bytes = 0;
    bool is_keyframe = false;
    uint64_t queue_depth = 0;
    uint64_t dropped_units = 0;
    uint32_t attr_flags = 0;
};

struct AccessUnitTraceSnapshot {
    const char *kind = "";
    uint64_t access_unit_index = 0;
    int64_t pts_us = 0;
    int64_t raw_pts = 0;
    int64_t after_start_us = -1;
    size_t payload_bytes = 0;
    size_t emitted_bytes = 0;
    size_t pending_config_bytes_before = 0;
    size_t prepended_config_bytes = 0;
    uint64_t queue_depth = 0;
    uint32_t attr_flags = 0;
    NalInspection input_nals = {};
    NalInspection emitted_nals = {};
};

std::string BuildOperationError(const std::string &operation, const std::string &message)
{
    return operation + ": " + message;
}

std::string BuildOperationErrorCode(const std::string &operation, int32_t code)
{
    std::ostringstream stream;
    stream << operation << " failed with code " << code;
    return stream.str();
}

bool StartsWithStartCode(const std::vector<uint8_t> &bytes, size_t offset, size_t *next_offset)
{
    if (offset + 3 > bytes.size()) {
        return false;
    }
    if (bytes[offset] != 0x00 || bytes[offset + 1] != 0x00) {
        return false;
    }
    if (bytes[offset + 2] == 0x01) {
        if (next_offset != nullptr) {
            *next_offset = offset + 3;
        }
        return true;
    }
    if (offset + 4 <= bytes.size() && bytes[offset + 2] == 0x00 && bytes[offset + 3] == 0x01) {
        if (next_offset != nullptr) {
            *next_offset = offset + 4;
        }
        return true;
    }
    return false;
}

bool ContainsIdrNalUnit(const std::vector<uint8_t> &bytes)
{
    for (size_t index = 0; index + 3 < bytes.size(); ++index) {
        size_t nal_offset = 0;
        if (!StartsWithStartCode(bytes, index, &nal_offset)) {
            continue;
        }
        if (nal_offset >= bytes.size()) {
            continue;
        }
        const uint8_t nal_type = bytes[nal_offset] & 0x1F;
        if (nal_type == 5) {
            return true;
        }
    }
    return false;
}

bool ContainsDecoderConfigNalUnit(const std::vector<uint8_t> &bytes)
{
    for (size_t index = 0; index + 3 < bytes.size(); ++index) {
        size_t nal_offset = 0;
        if (!StartsWithStartCode(bytes, index, &nal_offset)) {
            continue;
        }
        if (nal_offset >= bytes.size()) {
            continue;
        }
        const uint8_t nal_type = bytes[nal_offset] & 0x1F;
        if (nal_type == 7 || nal_type == 8) {
            return true;
        }
    }
    return false;
}

NalInspection InspectAnnexBNalUnits(const std::vector<uint8_t> &bytes)
{
    NalInspection inspection = {};
    std::ostringstream types;
    for (size_t cursor = 0; cursor < bytes.size();) {
        size_t nal_offset = 0;
        if (!StartsWithStartCode(bytes, cursor, &nal_offset)) {
            ++cursor;
            continue;
        }
        if (nal_offset >= bytes.size()) {
            break;
        }
        const uint8_t nal_type = bytes[nal_offset] & 0x1F;
        if (inspection.nal_count < kNalTypesTraceLimit) {
            if (!inspection.nal_types.empty()) {
                types << ",";
            }
            types << static_cast<unsigned int>(nal_type);
            inspection.nal_types = types.str();
        }
        ++inspection.nal_count;
        if (nal_type == 7) {
            ++inspection.sps_count;
        } else if (nal_type == 8) {
            ++inspection.pps_count;
        } else if (nal_type == 5) {
            ++inspection.idr_count;
        }

        size_t next_start_code_offset = bytes.size();
        for (size_t index = nal_offset; index + 3 < bytes.size(); ++index) {
            size_t ignored = 0;
            if (StartsWithStartCode(bytes, index, &ignored)) {
                next_start_code_offset = index;
                break;
            }
        }
        cursor = next_start_code_offset;
    }
    if (inspection.nal_count > kNalTypesTraceLimit) {
        types << ",...";
        inspection.nal_types = types.str();
    }
    return inspection;
}

void AppendBytes(const std::vector<uint8_t> &source, std::vector<uint8_t> *target)
{
    if (target == nullptr || source.empty()) {
        return;
    }
    target->insert(target->end(), source.begin(), source.end());
}

void AppendAnnexBStartCode(std::vector<uint8_t> *target)
{
    static constexpr uint8_t kStartCode[] = {0x00, 0x00, 0x00, 0x01};
    if (target == nullptr) {
        return;
    }
    target->insert(target->end(), kStartCode, kStartCode + sizeof(kStartCode));
}

bool ExtractAnnexBDecoderConfig(const std::vector<uint8_t> &payload, std::vector<uint8_t> *config)
{
    if (config == nullptr) {
        return false;
    }
    config->clear();
    size_t cursor = 0;
    while (cursor < payload.size()) {
        size_t nal_offset = 0;
        if (!StartsWithStartCode(payload, cursor, &nal_offset)) {
            ++cursor;
            continue;
        }
        const size_t start_code_offset = cursor;
        const size_t next_start_code_offset = [&payload, nal_offset]() {
            for (size_t index = nal_offset; index + 3 < payload.size(); ++index) {
                size_t ignored = 0;
                if (StartsWithStartCode(payload, index, &ignored)) {
                    return index;
                }
            }
            return payload.size();
        }();
        if (nal_offset < payload.size()) {
            const uint8_t nal_type = payload[nal_offset] & 0x1F;
            if (nal_type == 7 || nal_type == 8) {
                config->insert(config->end(), payload.data() + start_code_offset, payload.data() + next_start_code_offset);
            }
        }
        cursor = next_start_code_offset;
    }
    return !config->empty();
}

bool ExtractAvcCDecoderConfig(const std::vector<uint8_t> &payload, std::vector<uint8_t> *config)
{
    if (config == nullptr || payload.size() < 7 || payload[0] != 1) {
        return false;
    }
    config->clear();
    size_t cursor = 5;
    const uint8_t sps_count = payload[cursor++] & 0x1F;
    for (uint8_t index = 0; index < sps_count; ++index) {
        if (cursor + 2 > payload.size()) {
            config->clear();
            return false;
        }
        const size_t nal_size = (static_cast<size_t>(payload[cursor]) << 8) | payload[cursor + 1];
        cursor += 2;
        if (nal_size == 0 || cursor + nal_size > payload.size()) {
            config->clear();
            return false;
        }
        AppendAnnexBStartCode(config);
        config->insert(config->end(), payload.data() + cursor, payload.data() + cursor + nal_size);
        cursor += nal_size;
    }
    if (cursor >= payload.size()) {
        return !config->empty();
    }
    const uint8_t pps_count = payload[cursor++];
    for (uint8_t index = 0; index < pps_count; ++index) {
        if (cursor + 2 > payload.size()) {
            config->clear();
            return false;
        }
        const size_t nal_size = (static_cast<size_t>(payload[cursor]) << 8) | payload[cursor + 1];
        cursor += 2;
        if (nal_size == 0 || cursor + nal_size > payload.size()) {
            config->clear();
            return false;
        }
        AppendAnnexBStartCode(config);
        config->insert(config->end(), payload.data() + cursor, payload.data() + cursor + nal_size);
        cursor += nal_size;
    }
    return !config->empty();
}

bool ExtractDecoderConfig(const std::vector<uint8_t> &payload, std::vector<uint8_t> *config)
{
    return ExtractAnnexBDecoderConfig(payload, config) || ExtractAvcCDecoderConfig(payload, config);
}

bool ShouldEmitCallbackSummary(uint64_t access_unit_count)
{
    return access_unit_count != 0 && (access_unit_count % kCallbackSummaryEveryAccessUnits) == 0;
}

bool ShouldEmitCallbackWarning(uint64_t access_unit_count, uint64_t *last_warning_access_unit)
{
    if (last_warning_access_unit == nullptr) {
        return false;
    }
    if (access_unit_count <= 3 ||
        access_unit_count >= *last_warning_access_unit + kAnomalyWarnThrottleAccessUnits) {
        *last_warning_access_unit = access_unit_count;
        return true;
    }
    return false;
}

void LogCallbackSummary(const std::string &session_id, const CallbackSummarySnapshot &snapshot)
{
    const uint64_t average_payload_bytes =
        snapshot.access_unit_count == 0 ? 0 : (snapshot.total_payload_bytes / snapshot.access_unit_count);
    const uint64_t keyframe_percent =
        snapshot.access_unit_count == 0 ? 0 : ((snapshot.keyframe_count * 100) / snapshot.access_unit_count);
    core::diag::Info(
        "capture/h264_screen_capture_source",
        "encoder_output_summary",
        "session_id=%s callbacks=%llu access_units=%llu keyframes=%llu keyframe_pct=%llu payload_bytes_total=%llu "
        "payload_bytes_avg=%llu codec_config_units=%llu dropped_units=%llu queue_depth=%llu queue_depth_max=%llu "
        "pts_step_avg_us=%llu callback_step_avg_us=%llu pts_non_monotonic=%llu callback_gap_warns=%llu "
        "timeline_skew_warns=%llu stream_changed=%llu stream_changed_config=%llu codec_config_parse_failures=%llu "
        "access_units_with_config=%llu idr_with_config=%llu idr_with_prepended_config=%llu first_keyframe_idx=%llu "
        "first_access_unit_after_start_us=%lld first_keyframe_after_start_us=%lld",
        session_id.c_str(),
        static_cast<unsigned long long>(snapshot.callback_count),
        static_cast<unsigned long long>(snapshot.access_unit_count),
        static_cast<unsigned long long>(snapshot.keyframe_count),
        static_cast<unsigned long long>(keyframe_percent),
        static_cast<unsigned long long>(snapshot.total_payload_bytes),
        static_cast<unsigned long long>(average_payload_bytes),
        static_cast<unsigned long long>(snapshot.codec_config_count),
        static_cast<unsigned long long>(snapshot.dropped_units),
        static_cast<unsigned long long>(snapshot.queue_depth),
        static_cast<unsigned long long>(snapshot.max_queue_depth),
        static_cast<unsigned long long>(snapshot.avg_pts_step_us),
        static_cast<unsigned long long>(snapshot.avg_callback_step_us),
        static_cast<unsigned long long>(snapshot.non_monotonic_pts_count),
        static_cast<unsigned long long>(snapshot.callback_gap_count),
        static_cast<unsigned long long>(snapshot.timeline_skew_count),
        static_cast<unsigned long long>(snapshot.stream_changed_count),
        static_cast<unsigned long long>(snapshot.stream_changed_config_count),
        static_cast<unsigned long long>(snapshot.codec_config_parse_failures),
        static_cast<unsigned long long>(snapshot.access_units_with_config),
        static_cast<unsigned long long>(snapshot.idr_with_config_count),
        static_cast<unsigned long long>(snapshot.idr_with_prepended_config_count),
        static_cast<unsigned long long>(snapshot.first_keyframe_index),
        static_cast<long long>(snapshot.first_access_unit_after_start_us),
        static_cast<long long>(snapshot.first_keyframe_after_start_us));
}

void LogCallbackAnomaly(const std::string &session_id, const CallbackAnomalySnapshot &anomaly)
{
    core::diag::Warn(
        "capture/h264_screen_capture_source",
        "encoder_output_anomaly",
        "session_id=%s kind=%s frame_idx=%llu pts_us=%lld delta_pts_us=%lld callback_wall_us=%lld "
        "delta_callback_us=%lld payload_bytes=%zu keyframe=%d queue_depth=%llu dropped_units=%llu flags=0x%X",
        session_id.c_str(),
        anomaly.kind,
        static_cast<unsigned long long>(anomaly.access_unit_index),
        static_cast<long long>(anomaly.pts_us),
        static_cast<long long>(anomaly.delta_pts_us),
        static_cast<long long>(anomaly.callback_wall_us),
        static_cast<long long>(anomaly.delta_wall_us),
        anomaly.payload_bytes,
        anomaly.is_keyframe ? 1 : 0,
        static_cast<unsigned long long>(anomaly.queue_depth),
        static_cast<unsigned long long>(anomaly.dropped_units),
        anomaly.attr_flags);
}

void LogAccessUnitTrace(const std::string &session_id, const AccessUnitTraceSnapshot &trace)
{
    core::diag::Info(
        "capture/h264_screen_capture_source",
        "encoder_access_unit_trace",
        "session_id=%s kind=%s frame_idx=%llu pts_us=%lld raw_pts=%lld after_start_us=%lld payload_bytes=%zu "
        "emitted_bytes=%zu pending_config_before=%zu prepended_config_bytes=%zu flags=0x%X queue_depth=%llu "
        "input_nals=%llu input_sps=%llu input_pps=%llu input_idr=%llu input_types=%s "
        "emitted_nals=%llu emitted_sps=%llu emitted_pps=%llu emitted_idr=%llu emitted_types=%s",
        session_id.c_str(),
        trace.kind,
        static_cast<unsigned long long>(trace.access_unit_index),
        static_cast<long long>(trace.pts_us),
        static_cast<long long>(trace.raw_pts),
        static_cast<long long>(trace.after_start_us),
        trace.payload_bytes,
        trace.emitted_bytes,
        trace.pending_config_bytes_before,
        trace.prepended_config_bytes,
        trace.attr_flags,
        static_cast<unsigned long long>(trace.queue_depth),
        static_cast<unsigned long long>(trace.input_nals.nal_count),
        static_cast<unsigned long long>(trace.input_nals.sps_count),
        static_cast<unsigned long long>(trace.input_nals.pps_count),
        static_cast<unsigned long long>(trace.input_nals.idr_count),
        trace.input_nals.nal_types.empty() ? "none" : trace.input_nals.nal_types.c_str(),
        static_cast<unsigned long long>(trace.emitted_nals.nal_count),
        static_cast<unsigned long long>(trace.emitted_nals.sps_count),
        static_cast<unsigned long long>(trace.emitted_nals.pps_count),
        static_cast<unsigned long long>(trace.emitted_nals.idr_count),
        trace.emitted_nals.nal_types.empty() ? "none" : trace.emitted_nals.nal_types.c_str());
}

}  // namespace

struct ScreenCaptureSource::CallbackContext {
    ScreenCaptureSource *owner = nullptr;
};

ScreenCaptureSource::ScreenCaptureSource()
    : capture_(nullptr),
      encoder_(nullptr),
      encoder_surface_(nullptr),
      started_(false),
      running_(false),
      session_id_(),
      fatal_error_(),
      dropped_units_(0),
      callback_diag_(),
      pending_codec_config_(),
      queue_(),
      mutex_(),
      condition_(),
      callback_context_(nullptr)
{
}

ScreenCaptureSource::~ScreenCaptureSource()
{
    Stop();
}

bool ScreenCaptureSource::Start(const core::VideoTransportState &state, std::string *error)
{
    static constexpr const char *kOperation = "capture/h264_screen_capture_source.start";
    {
        std::lock_guard<std::mutex> lock(mutex_);
        if (started_) {
            if (error != nullptr) {
                *error = BuildOperationError(kOperation, "source has already started");
            }
            return false;
        }
    }

    codec::h264::EncoderConfiguration configuration = {};
    std::string config_error;
    if (!codec::h264::ResolveEncoderConfiguration(state, &configuration, &config_error)) {
        if (error != nullptr) {
            *error = BuildOperationError(kOperation, config_error);
        }
        return false;
    }

    callback_context_ = new CallbackContext();
    callback_context_->owner = this;

    encoder_ = OH_VideoEncoder_CreateByMime(OH_AVCODEC_MIMETYPE_VIDEO_AVC);
    if (encoder_ == nullptr) {
        if (error != nullptr) {
            *error = BuildOperationError(kOperation, "OH_VideoEncoder_CreateByMime returned null");
        }
        Stop();
        return false;
    }

    OH_AVFormat *encoder_format = OH_AVFormat_Create();
    if (encoder_format == nullptr) {
        if (error != nullptr) {
            *error = BuildOperationError(kOperation, "OH_AVFormat_Create returned null for encoder configuration");
        }
        Stop();
        return false;
    }

    (void)OH_AVFormat_SetIntValue(encoder_format, OH_MD_KEY_WIDTH, configuration.width);
    (void)OH_AVFormat_SetIntValue(encoder_format, OH_MD_KEY_HEIGHT, configuration.height);
    (void)OH_AVFormat_SetDoubleValue(
        encoder_format, OH_MD_KEY_FRAME_RATE, static_cast<double>(configuration.frame_rate));
    (void)OH_AVFormat_SetIntValue(encoder_format, OH_MD_KEY_PIXEL_FORMAT, AV_PIXEL_FORMAT_SURFACE_FORMAT);
    (void)OH_AVFormat_SetIntValue(encoder_format, OH_MD_KEY_VIDEO_ENCODE_BITRATE_MODE, BITRATE_MODE_VBR);
    (void)OH_AVFormat_SetLongValue(encoder_format, OH_MD_KEY_BITRATE, configuration.bitrate);
    const int32_t iframe_interval_ms =
        state.config.has_iframe_interval_ms && state.config.iframe_interval_ms > 0 ?
            state.config.iframe_interval_ms :
            kDefaultIFrameIntervalMs;
    const bool repeat_after_set = OH_AVFormat_SetIntValue(
        encoder_format, OH_MD_KEY_VIDEO_ENCODER_REPEAT_PREVIOUS_FRAME_AFTER, kRepeatPreviousFrameAfterUs);
    const bool repeat_max_set = OH_AVFormat_SetIntValue(
        encoder_format, OH_MD_KEY_VIDEO_ENCODER_REPEAT_PREVIOUS_MAX_COUNT, kRepeatPreviousMaxCount);
    (void)OH_AVFormat_SetIntValue(encoder_format, OH_MD_KEY_I_FRAME_INTERVAL, iframe_interval_ms);

    core::diag::Info(
        "capture/h264_screen_capture_source",
        "start_config",
        "session_id=%s width=%d height=%d frame_rate=%d bitrate=%lld iframe_interval_ms=%d repeat_previous_frame_after_value=%d repeat_previous_after_set=%d repeat_previous_max_count=%d repeat_previous_max_set=%d",
        state.session_id.c_str(),
        static_cast<int>(configuration.width),
        static_cast<int>(configuration.height),
        static_cast<int>(configuration.frame_rate),
        static_cast<long long>(configuration.bitrate),
        static_cast<int>(iframe_interval_ms),
        static_cast<int>(kRepeatPreviousFrameAfterUs),
        repeat_after_set ? 1 : 0,
        static_cast<int>(kRepeatPreviousMaxCount),
        repeat_max_set ? 1 : 0);

    const OH_AVErrCode configure_result = OH_VideoEncoder_Configure(encoder_, encoder_format);
    OH_AVFormat_Destroy(encoder_format);
    encoder_format = nullptr;
    if (configure_result != AV_ERR_OK) {
        if (error != nullptr) {
            *error = BuildOperationErrorCode("capture/h264_screen_capture_source.encoder_configure", configure_result);
        }
        Stop();
        return false;
    }

    const OH_AVErrCode surface_result = OH_VideoEncoder_GetSurface(encoder_, &encoder_surface_);
    if (surface_result != AV_ERR_OK || encoder_surface_ == nullptr) {
        if (error != nullptr) {
            *error = BuildOperationErrorCode("capture/h264_screen_capture_source.encoder_get_surface", surface_result);
        }
        Stop();
        return false;
    }

    const OH_AVErrCode encoder_callback_result = OH_VideoEncoder_RegisterCallback(
        encoder_,
        {OnEncoderError, OnEncoderStreamChanged, OnEncoderNeedInputBuffer, OnEncoderOutputBuffer},
        callback_context_);
    if (encoder_callback_result != AV_ERR_OK) {
        if (error != nullptr) {
            *error = BuildOperationErrorCode(
                "capture/h264_screen_capture_source.encoder_register_callback", encoder_callback_result);
        }
        Stop();
        return false;
    }

    {
        std::lock_guard<std::mutex> lock(mutex_);
        started_ = true;
        running_ = true;
        session_id_ = state.session_id;
        fatal_error_.clear();
        dropped_units_ = 0;
        callback_diag_ = {};
        callback_diag_.start_wall_us = core::diag::NowSteadyTimeUs();
        queue_.clear();
        pending_codec_config_.clear();
    }

    const OH_AVErrCode prepare_result = OH_VideoEncoder_Prepare(encoder_);
    if (prepare_result != AV_ERR_OK) {
        if (error != nullptr) {
            *error = BuildOperationErrorCode("capture/h264_screen_capture_source.encoder_prepare", prepare_result);
        }
        Stop();
        return false;
    }

    const OH_AVErrCode encoder_start_result = OH_VideoEncoder_Start(encoder_);
    if (encoder_start_result != AV_ERR_OK) {
        if (error != nullptr) {
            *error = BuildOperationErrorCode("capture/h264_screen_capture_source.encoder_start", encoder_start_result);
        }
        Stop();
        return false;
    }

    capture_ = OH_AVScreenCapture_Create();
    if (capture_ == nullptr) {
        if (error != nullptr) {
            *error = BuildOperationError(kOperation, "OH_AVScreenCapture_Create returned null");
        }
        Stop();
        return false;
    }

    const OH_AVSCREEN_CAPTURE_ErrCode state_cb_result =
        OH_AVScreenCapture_SetStateCallback(capture_, OnCaptureStateChange, callback_context_);
    if (state_cb_result != AV_SCREEN_CAPTURE_ERR_OK) {
        if (error != nullptr) {
            *error = BuildOperationErrorCode(
                "capture/h264_screen_capture_source.capture_set_state_callback", state_cb_result);
        }
        Stop();
        return false;
    }

    const OH_AVSCREEN_CAPTURE_ErrCode error_cb_result =
        OH_AVScreenCapture_SetErrorCallback(capture_, OnCaptureError, callback_context_);
    if (error_cb_result != AV_SCREEN_CAPTURE_ERR_OK) {
        if (error != nullptr) {
            *error = BuildOperationErrorCode(
                "capture/h264_screen_capture_source.capture_set_error_callback", error_cb_result);
        }
        Stop();
        return false;
    }

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
    config.videoInfo.videoCapInfo.videoFrameWidth = configuration.width;
    config.videoInfo.videoCapInfo.videoFrameHeight = configuration.height;
    config.videoInfo.videoCapInfo.videoSource = OH_VIDEO_SOURCE_SURFACE_RGBA;
    config.videoInfo.videoEncInfo.videoCodec = OH_H264;
    config.videoInfo.videoEncInfo.videoBitrate = configuration.bitrate;
    config.videoInfo.videoEncInfo.videoFrameRate = configuration.frame_rate;

    const OH_AVSCREEN_CAPTURE_ErrCode init_result = OH_AVScreenCapture_Init(capture_, config);
    if (init_result != AV_SCREEN_CAPTURE_ERR_OK) {
        if (error != nullptr) {
            *error = BuildOperationErrorCode("capture/h264_screen_capture_source.capture_init", init_result);
        }
        Stop();
        return false;
    }

    (void)OH_AVScreenCapture_SetMicrophoneEnabled(capture_, false);

    const OH_AVSCREEN_CAPTURE_ErrCode capture_start_result =
        OH_AVScreenCapture_StartScreenCaptureWithSurface(capture_, encoder_surface_);
    if (capture_start_result != AV_SCREEN_CAPTURE_ERR_OK) {
        if (error != nullptr) {
            *error = BuildOperationErrorCode(
                "capture/h264_screen_capture_source.capture_start_with_surface", capture_start_result);
        }
        Stop();
        return false;
    }
    return true;
}

void ScreenCaptureSource::Stop()
{
    bool should_log_summary = false;
    CallbackSummarySnapshot summary_snapshot = {};
    std::string session_id;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        running_ = false;
        started_ = false;
        if (callback_diag_.access_unit_count > 0) {
            summary_snapshot.callback_count = callback_diag_.callback_count;
            summary_snapshot.access_unit_count = callback_diag_.access_unit_count;
            summary_snapshot.keyframe_count = callback_diag_.keyframe_count;
            summary_snapshot.codec_config_count = callback_diag_.codec_config_count;
            summary_snapshot.total_payload_bytes = callback_diag_.total_payload_bytes;
            summary_snapshot.dropped_units = dropped_units_;
            summary_snapshot.max_queue_depth = callback_diag_.max_queue_depth;
            summary_snapshot.non_monotonic_pts_count = callback_diag_.non_monotonic_pts_count;
            summary_snapshot.callback_gap_count = callback_diag_.callback_gap_count;
            summary_snapshot.timeline_skew_count = callback_diag_.timeline_skew_count;
            summary_snapshot.stream_changed_count = callback_diag_.stream_changed_count;
            summary_snapshot.stream_changed_config_count = callback_diag_.stream_changed_config_count;
            summary_snapshot.codec_config_parse_failures = callback_diag_.codec_config_parse_failures;
            summary_snapshot.access_units_with_config = callback_diag_.access_units_with_config;
            summary_snapshot.idr_with_config_count = callback_diag_.idr_with_config_count;
            summary_snapshot.idr_with_prepended_config_count = callback_diag_.idr_with_prepended_config_count;
            summary_snapshot.first_keyframe_index = callback_diag_.first_keyframe_index;
            summary_snapshot.first_access_unit_after_start_us = callback_diag_.first_access_unit_after_start_us;
            summary_snapshot.first_keyframe_after_start_us = callback_diag_.first_keyframe_after_start_us;
            summary_snapshot.pts_step_samples = callback_diag_.pts_step_samples;
            summary_snapshot.callback_step_samples = callback_diag_.callback_step_samples;
            summary_snapshot.avg_pts_step_us = callback_diag_.pts_step_samples == 0 ?
                0 :
                (callback_diag_.total_pts_step_us / callback_diag_.pts_step_samples);
            summary_snapshot.avg_callback_step_us = callback_diag_.callback_step_samples == 0 ?
                0 :
                (callback_diag_.total_callback_step_us / callback_diag_.callback_step_samples);
            summary_snapshot.queue_depth = queue_.size();
            session_id = session_id_;
            should_log_summary = true;
        }
        queue_.clear();
        pending_codec_config_.clear();
        session_id_.clear();
        condition_.notify_all();
    }

    if (should_log_summary) {
        LogCallbackSummary(session_id, summary_snapshot);
    }

    if (capture_ != nullptr) {
        (void)OH_AVScreenCapture_StopScreenCapture(capture_);
        (void)OH_AVScreenCapture_Release(capture_);
        capture_ = nullptr;
    }

    if (encoder_ != nullptr) {
        (void)OH_VideoEncoder_Stop(encoder_);
        (void)OH_VideoEncoder_Destroy(encoder_);
        encoder_ = nullptr;
    }

    if (encoder_surface_ != nullptr) {
        OH_NativeWindow_DestroyNativeWindow(encoder_surface_);
        encoder_surface_ = nullptr;
    }

    if (callback_context_ != nullptr) {
        callback_context_->owner = nullptr;
        delete callback_context_;
        callback_context_ = nullptr;
    }
}

PullResult ScreenCaptureSource::PullAccessUnit(int timeout_ms, AccessUnit *unit, std::string *error)
{
    static constexpr const char *kOperation = "capture/h264_screen_capture_source.pull_access_unit";
    if (unit == nullptr) {
        if (error != nullptr) {
            *error = BuildOperationError(kOperation, "output access unit must not be null");
        }
        return PullResult::kError;
    }

    const std::chrono::milliseconds timeout(timeout_ms < 0 ? 0 : timeout_ms);
    std::unique_lock<std::mutex> lock(mutex_);
    if (!started_) {
        if (error != nullptr) {
            *error = BuildOperationError(kOperation, "source is not started");
        }
        return PullResult::kError;
    }

    const bool ready = condition_.wait_for(lock, timeout, [this]() {
        return !queue_.empty() || !running_ || !fatal_error_.empty();
    });
    if (!ready) {
        return PullResult::kTimeout;
    }

    if (!queue_.empty()) {
        *unit = std::move(queue_.front());
        queue_.pop_front();
        return PullResult::kOk;
    }

    if (!fatal_error_.empty()) {
        if (error != nullptr) {
            *error = fatal_error_;
        }
        return PullResult::kError;
    }
    return PullResult::kStopped;
}

void ScreenCaptureSource::OnCaptureStateChange(
    struct OH_AVScreenCapture *capture, OH_AVScreenCaptureStateCode state_code, void *user_data)
{
    (void)capture;
    CallbackContext *context = static_cast<CallbackContext *>(user_data);
    if (context == nullptr || context->owner == nullptr) {
        return;
    }

    std::lock_guard<std::mutex> lock(context->owner->mutex_);
    if (!context->owner->IsRunningLocked()) {
        return;
    }

    core::diag::Info(
        "capture/h264_screen_capture_source",
        "capture_state",
        "session_id=%s state_code=%d",
        context->owner->session_id_.c_str(),
        static_cast<int>(state_code));

    if (state_code == OH_SCREEN_CAPTURE_STATE_CANCELED ||
        state_code == OH_SCREEN_CAPTURE_STATE_STOPPED_BY_USER ||
        state_code == OH_SCREEN_CAPTURE_STATE_INTERRUPTED_BY_OTHER ||
        state_code == OH_SCREEN_CAPTURE_STATE_STOPPED_BY_CALL ||
        state_code == OH_SCREEN_CAPTURE_STATE_STOPPED_BY_USER_SWITCHES) {
        context->owner->SetFatalErrorLocked("capture/h264_screen_capture_source.capture_state", state_code);
        context->owner->running_ = false;
        context->owner->condition_.notify_all();
    }
}

void ScreenCaptureSource::OnCaptureError(struct OH_AVScreenCapture *capture, int32_t error_code, void *user_data)
{
    (void)capture;
    CallbackContext *context = static_cast<CallbackContext *>(user_data);
    if (context == nullptr || context->owner == nullptr) {
        return;
    }

    std::lock_guard<std::mutex> lock(context->owner->mutex_);
    context->owner->SetFatalErrorLocked("capture/h264_screen_capture_source.capture_error", error_code);
    context->owner->running_ = false;
    context->owner->condition_.notify_all();
}

void ScreenCaptureSource::OnEncoderError(struct OH_AVCodec *codec, int32_t error_code, void *user_data)
{
    (void)codec;
    CallbackContext *context = static_cast<CallbackContext *>(user_data);
    if (context == nullptr || context->owner == nullptr) {
        return;
    }

    std::lock_guard<std::mutex> lock(context->owner->mutex_);
    context->owner->SetFatalErrorLocked("capture/h264_screen_capture_source.encoder_error", error_code);
    context->owner->running_ = false;
    context->owner->condition_.notify_all();
}

void ScreenCaptureSource::OnEncoderStreamChanged(struct OH_AVCodec *codec, struct OH_AVFormat *format, void *user_data)
{
    (void)codec;
    CallbackContext *context = static_cast<CallbackContext *>(user_data);
    if (context == nullptr || context->owner == nullptr || format == nullptr) {
        return;
    }

    std::string session_id;
    {
        std::lock_guard<std::mutex> lock(context->owner->mutex_);
        if (!context->owner->IsRunningLocked()) {
            return;
        }
        ++context->owner->callback_diag_.stream_changed_count;
        session_id = context->owner->session_id_;
    }

    uint8_t *codec_config = nullptr;
    size_t codec_config_size = 0;
    if (!OH_AVFormat_GetBuffer(format, OH_MD_KEY_CODEC_CONFIG, &codec_config, &codec_config_size) ||
        codec_config == nullptr || codec_config_size == 0) {
        core::diag::Warn(
            "capture/h264_screen_capture_source",
            "encoder_stream_changed",
            "session_id=%s has_codec_config=0 codec_config_bytes=%zu",
            session_id.c_str(),
            codec_config_size);
        return;
    }

    std::vector<uint8_t> payload(codec_config, codec_config + codec_config_size);
    std::vector<uint8_t> annex_b_config;
    if (!ExtractDecoderConfig(payload, &annex_b_config)) {
        {
            std::lock_guard<std::mutex> lock(context->owner->mutex_);
            ++context->owner->callback_diag_.codec_config_parse_failures;
        }
        core::diag::Warn(
            "capture/h264_screen_capture_source",
            "encoder_stream_changed",
            "session_id=%s has_codec_config=1 codec_config_bytes=%zu parse_result=failed",
            session_id.c_str(),
            codec_config_size);
        return;
    }

    std::lock_guard<std::mutex> lock(context->owner->mutex_);
    if (!context->owner->IsRunningLocked()) {
        return;
    }
    context->owner->pending_codec_config_ = std::move(annex_b_config);
    ++context->owner->callback_diag_.codec_config_count;
    ++context->owner->callback_diag_.stream_changed_config_count;
    const size_t pending_bytes = context->owner->pending_codec_config_.size();
    core::diag::Info(
        "capture/h264_screen_capture_source",
        "encoder_stream_changed",
        "session_id=%s has_codec_config=1 codec_config_bytes=%zu annexb_config_bytes=%zu parse_result=ok",
        context->owner->session_id_.c_str(),
        codec_config_size,
        pending_bytes);
}

void ScreenCaptureSource::OnEncoderNeedInputBuffer(
    struct OH_AVCodec *codec, uint32_t index, struct OH_AVBuffer *buffer, void *user_data)
{
    (void)codec;
    (void)index;
    (void)buffer;
    (void)user_data;
}

void ScreenCaptureSource::OnEncoderOutputBuffer(
    struct OH_AVCodec *codec, uint32_t index, struct OH_AVBuffer *buffer, void *user_data)
{
    CallbackContext *context = static_cast<CallbackContext *>(user_data);
    if (context == nullptr || context->owner == nullptr || codec == nullptr) {
        return;
    }

    ScreenCaptureSource *owner = context->owner;
    const int64_t callback_wall_us = core::diag::NowSteadyTimeUs();
    bool should_notify = false;
    bool should_log_summary = false;
    bool should_log_anomaly = false;
    bool should_log_access_unit_trace = false;
    CallbackSummarySnapshot summary_snapshot = {};
    CallbackAnomalySnapshot anomaly_snapshot = {};
    AccessUnitTraceSnapshot access_unit_trace = {};
    std::string session_id;
    int64_t callback_step_us = -1;
    bool callback_gap_detected = false;

    {
        std::lock_guard<std::mutex> lock(owner->mutex_);
        CallbackDiagnostics &diag = owner->callback_diag_;
        ++diag.callback_count;
        if (diag.last_callback_wall_us >= 0) {
            callback_step_us = callback_wall_us - diag.last_callback_wall_us;
            if (callback_step_us > 0) {
                diag.total_callback_step_us += static_cast<uint64_t>(callback_step_us);
                ++diag.callback_step_samples;
            }
            if (callback_step_us >= kCallbackGapWarnUs) {
                ++diag.callback_gap_count;
                callback_gap_detected = true;
            }
        }
        diag.last_callback_wall_us = callback_wall_us;
    }

    if (buffer != nullptr) {
        OH_AVCodecBufferAttr attr = {};
        const OH_AVErrCode attr_result = OH_AVBuffer_GetBufferAttr(buffer, &attr);
        if (attr_result == AV_ERR_OK) {
            if ((attr.flags & AVCODEC_BUFFER_FLAGS_EOS) != 0) {
                std::lock_guard<std::mutex> lock(owner->mutex_);
                owner->running_ = false;
                should_notify = true;
            } else if (attr.size > 0 && attr.offset >= 0) {
                uint8_t *address = OH_AVBuffer_GetAddr(buffer);
                const int32_t capacity = OH_AVBuffer_GetCapacity(buffer);
                const int64_t end_offset = static_cast<int64_t>(attr.offset) + static_cast<int64_t>(attr.size);
                if (address != nullptr && capacity > 0 && end_offset <= static_cast<int64_t>(capacity)) {
                    std::vector<uint8_t> payload(
                        address + static_cast<size_t>(attr.offset),
                        address + static_cast<size_t>(attr.offset + attr.size));
                    const bool has_idr = ContainsIdrNalUnit(payload);
                    const bool has_decoder_config = ContainsDecoderConfigNalUnit(payload);
                    const bool is_codec_data = (attr.flags & AVCODEC_BUFFER_FLAGS_CODEC_DATA) != 0;

                    std::lock_guard<std::mutex> lock(owner->mutex_);
                    if (owner->IsRunningLocked()) {
                        CallbackDiagnostics &diag = owner->callback_diag_;
                        if (is_codec_data) {
                            std::vector<uint8_t> decoder_config;
                            if (ExtractDecoderConfig(payload, &decoder_config)) {
                                ++diag.codec_config_count;
                                owner->pending_codec_config_ = std::move(decoder_config);
                                core::diag::Info(
                                    "capture/h264_screen_capture_source",
                                    "encoder_codec_config",
                                    "session_id=%s source=output_buffer flags=0x%X codec_config_bytes=%zu annexb_config_bytes=%zu",
                                    owner->session_id_.c_str(),
                                    static_cast<unsigned int>(attr.flags),
                                    payload.size(),
                                    owner->pending_codec_config_.size());
                            } else {
                                ++diag.codec_config_parse_failures;
                                core::diag::Warn(
                                    "capture/h264_screen_capture_source",
                                    "encoder_codec_config",
                                    "session_id=%s source=output_buffer flags=0x%X codec_config_bytes=%zu parse_result=failed",
                                    owner->session_id_.c_str(),
                                    static_cast<unsigned int>(attr.flags),
                                    payload.size());
                            }
                        } else {
                            AccessUnit unit = {};
                            unit.pts_us = attr.pts >= 0 ? (attr.pts / kNanosecondsPerMicrosecond) : 0;
                            unit.is_keyframe = has_idr;
                            const NalInspection input_nals = InspectAnnexBNalUnits(payload);
                            const size_t pending_config_bytes_before = owner->pending_codec_config_.size();
                            size_t prepended_config_bytes = 0;

                            if (has_decoder_config) {
                                std::vector<uint8_t> decoder_config;
                                if (ExtractDecoderConfig(payload, &decoder_config)) {
                                    owner->pending_codec_config_ = std::move(decoder_config);
                                }
                            }

                            if (has_idr && !has_decoder_config && !owner->pending_codec_config_.empty()) {
                                unit.bytes.reserve(owner->pending_codec_config_.size() + payload.size());
                                prepended_config_bytes = owner->pending_codec_config_.size();
                                AppendBytes(owner->pending_codec_config_, &unit.bytes);
                                AppendBytes(payload, &unit.bytes);
                            } else {
                                unit.bytes = std::move(payload);
                            }

                            if (has_idr) {
                                owner->pending_codec_config_.clear();
                            }
                            const NalInspection emitted_nals = InspectAnnexBNalUnits(unit.bytes);

                            const int64_t previous_pts_us = diag.last_pts_us;
                            int64_t delta_pts_us = 0;
                            bool non_monotonic_pts = false;
                            if (previous_pts_us >= 0) {
                                delta_pts_us = unit.pts_us - previous_pts_us;
                                if (delta_pts_us < 0) {
                                    ++diag.non_monotonic_pts_count;
                                    non_monotonic_pts = true;
                                } else {
                                    diag.total_pts_step_us += static_cast<uint64_t>(delta_pts_us);
                                    ++diag.pts_step_samples;
                                }
                            }
                            diag.last_pts_us = unit.pts_us;

                            const bool timeline_skew =
                                (delta_pts_us >= 0 && callback_step_us > 0 &&
                                    std::llabs(callback_step_us - delta_pts_us) >= kTimelineSkewWarnUs);
                            if (timeline_skew) {
                                ++diag.timeline_skew_count;
                            }

                            const size_t payload_bytes = unit.bytes.size();
                            ++diag.access_unit_count;
                            if (unit.is_keyframe) {
                                ++diag.keyframe_count;
                                if (diag.first_keyframe_index == 0) {
                                    diag.first_keyframe_index = diag.access_unit_count;
                                }
                            }
                            if (has_decoder_config) {
                                ++diag.access_units_with_config;
                            }
                            if (has_idr && has_decoder_config) {
                                ++diag.idr_with_config_count;
                            }
                            if (has_idr && prepended_config_bytes > 0) {
                                ++diag.idr_with_prepended_config_count;
                            }
                            const int64_t after_start_us =
                                diag.start_wall_us >= 0 ? (callback_wall_us - diag.start_wall_us) : -1;
                            if (diag.first_access_unit_after_start_us < 0) {
                                diag.first_access_unit_after_start_us = after_start_us;
                            }
                            if (unit.is_keyframe && diag.first_keyframe_after_start_us < 0) {
                                diag.first_keyframe_after_start_us = after_start_us;
                            }
                            diag.total_payload_bytes += static_cast<uint64_t>(payload_bytes);

                            const bool trace_startup_unit =
                                diag.access_unit_count <= kStartupAccessUnitTraceLimit;
                            const bool trace_keyframe = unit.is_keyframe;
                            if (trace_startup_unit || trace_keyframe || has_decoder_config || prepended_config_bytes > 0) {
                                access_unit_trace.kind = trace_keyframe ?
                                    (trace_startup_unit ? "startup_keyframe" : "keyframe") :
                                    (has_decoder_config ? "decoder_config_unit" : "startup");
                                access_unit_trace.access_unit_index = diag.access_unit_count;
                                access_unit_trace.pts_us = unit.pts_us;
                                access_unit_trace.raw_pts = attr.pts;
                                access_unit_trace.after_start_us = after_start_us;
                                access_unit_trace.payload_bytes = static_cast<size_t>(attr.size);
                                access_unit_trace.emitted_bytes = payload_bytes;
                                access_unit_trace.pending_config_bytes_before = pending_config_bytes_before;
                                access_unit_trace.prepended_config_bytes = prepended_config_bytes;
                                access_unit_trace.attr_flags = static_cast<uint32_t>(attr.flags);
                                access_unit_trace.input_nals = input_nals;
                                access_unit_trace.emitted_nals = emitted_nals;
                                session_id = owner->session_id_;
                                should_log_access_unit_trace = true;
                            }

                            owner->PushAccessUnitLocked(std::move(unit));
                            diag.max_queue_depth =
                                std::max<uint64_t>(diag.max_queue_depth, static_cast<uint64_t>(owner->queue_.size()));
                            access_unit_trace.queue_depth = owner->queue_.size();

                            const bool queue_pressure = owner->queue_.size() >= kMaxQueuedAccessUnits;
                            const bool large_payload = payload_bytes >= kLargePayloadWarnBytes;
                            const bool dropped_now = owner->dropped_units_ > diag.dropped_units_last_report;
                            if (dropped_now) {
                                diag.dropped_units_last_report = owner->dropped_units_;
                            }

                            if ((non_monotonic_pts || callback_gap_detected || timeline_skew || queue_pressure ||
                                    dropped_now || large_payload) &&
                                ShouldEmitCallbackWarning(diag.access_unit_count, &diag.last_warning_access_unit)) {
                                const char *anomaly_kind = "unknown";
                                if (non_monotonic_pts) {
                                    anomaly_kind = "pts_non_monotonic";
                                } else if (dropped_now) {
                                    anomaly_kind = "queue_drop";
                                } else if (queue_pressure) {
                                    anomaly_kind = "queue_pressure";
                                } else if (timeline_skew) {
                                    anomaly_kind = "timeline_skew";
                                } else if (callback_gap_detected) {
                                    anomaly_kind = "callback_gap";
                                } else if (large_payload) {
                                    anomaly_kind = "large_payload";
                                }
                                anomaly_snapshot.kind = anomaly_kind;
                                anomaly_snapshot.access_unit_index = diag.access_unit_count;
                                anomaly_snapshot.pts_us = unit.pts_us;
                                anomaly_snapshot.delta_pts_us = delta_pts_us;
                                anomaly_snapshot.callback_wall_us = callback_wall_us;
                                anomaly_snapshot.delta_wall_us = callback_step_us;
                                anomaly_snapshot.payload_bytes = payload_bytes;
                                anomaly_snapshot.is_keyframe = unit.is_keyframe;
                                anomaly_snapshot.queue_depth = owner->queue_.size();
                                anomaly_snapshot.dropped_units = owner->dropped_units_;
                                anomaly_snapshot.attr_flags = static_cast<uint32_t>(attr.flags);
                                session_id = owner->session_id_;
                                should_log_anomaly = true;
                            }

                            if (ShouldEmitCallbackSummary(diag.access_unit_count)) {
                                summary_snapshot.callback_count = diag.callback_count;
                                summary_snapshot.access_unit_count = diag.access_unit_count;
                                summary_snapshot.keyframe_count = diag.keyframe_count;
                                summary_snapshot.codec_config_count = diag.codec_config_count;
                                summary_snapshot.total_payload_bytes = diag.total_payload_bytes;
                                summary_snapshot.dropped_units = owner->dropped_units_;
                                summary_snapshot.max_queue_depth = diag.max_queue_depth;
                                summary_snapshot.non_monotonic_pts_count = diag.non_monotonic_pts_count;
                                summary_snapshot.callback_gap_count = diag.callback_gap_count;
                                summary_snapshot.timeline_skew_count = diag.timeline_skew_count;
                                summary_snapshot.stream_changed_count = diag.stream_changed_count;
                                summary_snapshot.stream_changed_config_count = diag.stream_changed_config_count;
                                summary_snapshot.codec_config_parse_failures = diag.codec_config_parse_failures;
                                summary_snapshot.access_units_with_config = diag.access_units_with_config;
                                summary_snapshot.idr_with_config_count = diag.idr_with_config_count;
                                summary_snapshot.idr_with_prepended_config_count = diag.idr_with_prepended_config_count;
                                summary_snapshot.first_keyframe_index = diag.first_keyframe_index;
                                summary_snapshot.first_access_unit_after_start_us = diag.first_access_unit_after_start_us;
                                summary_snapshot.first_keyframe_after_start_us = diag.first_keyframe_after_start_us;
                                summary_snapshot.pts_step_samples = diag.pts_step_samples;
                                summary_snapshot.callback_step_samples = diag.callback_step_samples;
                                summary_snapshot.avg_pts_step_us =
                                    diag.pts_step_samples == 0 ? 0 : (diag.total_pts_step_us / diag.pts_step_samples);
                                summary_snapshot.avg_callback_step_us = diag.callback_step_samples == 0 ?
                                    0 :
                                    (diag.total_callback_step_us / diag.callback_step_samples);
                                summary_snapshot.queue_depth = owner->queue_.size();
                                session_id = owner->session_id_;
                                should_log_summary = true;
                            }
                            should_notify = true;
                        }
                    }
                } else {
                    std::lock_guard<std::mutex> lock(owner->mutex_);
                    owner->SetFatalErrorLocked("capture/h264_screen_capture_source.encoder_buffer_bounds", AV_ERR_INVALID_VAL);
                    owner->running_ = false;
                    should_notify = true;
                }
            }
        } else {
            std::lock_guard<std::mutex> lock(owner->mutex_);
            owner->SetFatalErrorLocked("capture/h264_screen_capture_source.encoder_buffer_attr", attr_result);
            owner->running_ = false;
            should_notify = true;
        }
    }

    const OH_AVErrCode free_result = OH_VideoEncoder_FreeOutputBuffer(codec, index);
    if (free_result != AV_ERR_OK) {
        std::lock_guard<std::mutex> lock(owner->mutex_);
        owner->SetFatalErrorLocked("capture/h264_screen_capture_source.encoder_free_output", free_result);
        owner->running_ = false;
        should_notify = true;
    }

    if (should_notify) {
        owner->condition_.notify_all();
    }
    if (should_log_summary) {
        LogCallbackSummary(session_id, summary_snapshot);
    }
    if (should_log_anomaly) {
        LogCallbackAnomaly(session_id, anomaly_snapshot);
    }
    if (should_log_access_unit_trace) {
        LogAccessUnitTrace(session_id, access_unit_trace);
    }
}

void ScreenCaptureSource::SetFatalErrorLocked(const std::string &operation, int32_t code)
{
    if (!fatal_error_.empty()) {
        return;
    }
    fatal_error_ = BuildOperationErrorCode(operation, code);
    core::diag::Error(
        "capture/h264_screen_capture_source",
        "fatal_error",
        "session_id=%s operation=%s code=%d",
        session_id_.c_str(),
        operation.c_str(),
        static_cast<int>(code));
}

void ScreenCaptureSource::PushAccessUnitLocked(AccessUnit &&unit)
{
    if (queue_.size() >= kMaxQueuedAccessUnits) {
        queue_.pop_front();
        ++dropped_units_;
    }
    queue_.push_back(std::move(unit));
}

bool ScreenCaptureSource::IsRunningLocked() const
{
    return running_;
}

}  // namespace h264
}  // namespace capture
}  // namespace hscrcpy
