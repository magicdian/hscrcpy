#ifndef HSCRCPY_SERVER_CAPTURE_H264_SCREEN_CAPTURE_SOURCE_H
#define HSCRCPY_SERVER_CAPTURE_H264_SCREEN_CAPTURE_SOURCE_H

#include <cstdint>
#include <condition_variable>
#include <deque>
#include <mutex>
#include <string>
#include <vector>

#include <multimedia/player_framework/native_avbuffer.h>
#include <multimedia/player_framework/native_avcodec_base.h>
#include <multimedia/player_framework/native_avscreen_capture_base.h>
#include <native_window/external_window.h>

#include "core/companion_contracts.h"

namespace hscrcpy {
namespace capture {
namespace h264 {

struct AccessUnit {
    std::vector<uint8_t> bytes;
    int64_t pts_us;
    bool is_keyframe;
};

enum class PullResult {
    kOk,
    kTimeout,
    kStopped,
    kError
};

class ScreenCaptureSource {
public:
    ScreenCaptureSource();
    ~ScreenCaptureSource();

    ScreenCaptureSource(const ScreenCaptureSource &) = delete;
    ScreenCaptureSource &operator=(const ScreenCaptureSource &) = delete;

    bool Start(const core::VideoTransportState &state, std::string *error);
    void Stop();
    PullResult PullAccessUnit(int timeout_ms, AccessUnit *unit, std::string *error);

private:
    struct CallbackDiagnostics {
        uint64_t callback_count = 0;
        uint64_t access_unit_count = 0;
        uint64_t keyframe_count = 0;
        uint64_t codec_config_count = 0;
        uint64_t total_payload_bytes = 0;
        uint64_t max_queue_depth = 0;
        uint64_t non_monotonic_pts_count = 0;
        uint64_t callback_gap_count = 0;
        uint64_t timeline_skew_count = 0;
        uint64_t dropped_units_last_report = 0;
        uint64_t last_warning_access_unit = 0;
        int64_t last_pts_us = -1;
        int64_t last_callback_wall_us = -1;
        uint64_t total_pts_step_us = 0;
        uint64_t total_callback_step_us = 0;
        uint64_t pts_step_samples = 0;
        uint64_t callback_step_samples = 0;
    };

    struct CallbackContext;

    static void OnCaptureStateChange(
        struct OH_AVScreenCapture *capture, OH_AVScreenCaptureStateCode state_code, void *user_data);
    static void OnCaptureError(struct OH_AVScreenCapture *capture, int32_t error_code, void *user_data);
    static void OnEncoderError(struct OH_AVCodec *codec, int32_t error_code, void *user_data);
    static void OnEncoderStreamChanged(struct OH_AVCodec *codec, struct OH_AVFormat *format, void *user_data);
    static void OnEncoderNeedInputBuffer(
        struct OH_AVCodec *codec, uint32_t index, struct OH_AVBuffer *buffer, void *user_data);
    static void OnEncoderOutputBuffer(
        struct OH_AVCodec *codec, uint32_t index, struct OH_AVBuffer *buffer, void *user_data);

    void SetFatalErrorLocked(const std::string &operation, int32_t code);
    void PushAccessUnitLocked(AccessUnit &&unit);
    bool IsRunningLocked() const;

    struct OH_AVScreenCapture *capture_;
    struct OH_AVCodec *encoder_;
    OHNativeWindow *encoder_surface_;
    bool started_;
    bool running_;
    std::string session_id_;
    std::string fatal_error_;
    uint64_t dropped_units_;
    CallbackDiagnostics callback_diag_;
    std::vector<uint8_t> pending_codec_config_;
    std::deque<AccessUnit> queue_;
    std::mutex mutex_;
    std::condition_variable condition_;
    CallbackContext *callback_context_;
};

}  // namespace h264
}  // namespace capture
}  // namespace hscrcpy

#endif
