#ifndef HSCRCPY_SERVER_CORE_COMPANION_CONTRACTS_H
#define HSCRCPY_SERVER_CORE_COMPANION_CONTRACTS_H

#include <cstdint>
#include <string>
#include <vector>

namespace hscrcpy {
namespace core {

static constexpr int kShellApiVersion = 1;
static constexpr int kProtocolMajor = 1;
static constexpr int kProtocolMinor = 0;
static constexpr const char *kCompanionName = "hscrcpy Harmony Companion";
static constexpr const char *kNativeCoreVersion = "0.4.0-h264-mainline";
static constexpr const char *kSessionMessageTypeHostHello = "host_hello";
static constexpr const char *kSessionMessageTypeDeviceHello = "device_hello";
static constexpr const char *kSessionMessageTypeAuthorizationUpdate = "authorization_update";
static constexpr const char *kSessionMessageTypeSessionConfig = "session_config";
static constexpr const char *kSessionMessageTypeSessionReady = "session_ready";
static constexpr const char *kSessionMessageTypeSessionError = "session_error";
static constexpr const char *kSessionMessageTypeStopSession = "stop_session";
static constexpr const char *kSessionPhaseVideoNegotiation = "video_negotiation";
static constexpr const char *kSessionPhaseAwaitingSessionConfig = "awaiting_session_config";
static constexpr const char *kSessionPhaseReady = "ready";
static constexpr const char *kSessionPhaseStopped = "stopped";
static constexpr const char *kSessionPhaseFailed = "failed";
static constexpr const char *kChannelNameSession = "session";
static constexpr const char *kChannelNameVideo = "video";
static constexpr const char *kChannelStateReady = "ready";
static constexpr const char *kChannelStatePendingOpen = "pending_open";
static constexpr const char *kPayloadTypeUtf8Json = "utf8_json";
static constexpr const char *kPayloadTypeBinary = "binary";
static constexpr const char *kVideoCodecH264 = "h264";
static constexpr const char *kVideoCodecJpeg = "jpeg";
static constexpr const char *kVideoCodecH265 = "h265";
static constexpr const char *kEncoderKindHardware = "hardware";
static constexpr const char *kEncoderKindSoftware = "software";
static constexpr const char *kAuthorizationGranted = "granted";
static constexpr const char *kAuthorizationNeedsUserAction = "needs_user_action";
static constexpr const char *kAuthorizationDenied = "denied";
static constexpr const char *kAuthorizationUnsupported = "unsupported";
static constexpr const char *kFeatureVideo = "video";
static constexpr const char *kFeatureControl = "control";
static constexpr const char *kControlEventType = "control_event";
static constexpr const char *kControlEventPointerDown = "pointer_down";
static constexpr const char *kControlEventPointerMove = "pointer_move";
static constexpr const char *kControlEventPointerUp = "pointer_up";
static constexpr const char *kControlEventScroll = "scroll";
static constexpr const char *kControlEventKeyDown = "key_down";
static constexpr const char *kControlEventKeyUp = "key_up";
static constexpr const char *kControlEventDeviceAction = "device_action";
static constexpr const char *kControlButtonPrimary = "primary";
static constexpr const char *kControlButtonSecondary = "secondary";
static constexpr const char *kControlButtonMiddle = "middle";
static constexpr const char *kDeviceActionBack = "back";
static constexpr const char *kDeviceActionHome = "home";
static constexpr const char *kSessionErrorProtocolMajorMismatch = "protocol_major_mismatch";
static constexpr const char *kSessionErrorNoSharedVideoCodec = "no_shared_video_codec";
static constexpr const char *kSessionErrorFeatureUnsupported = "feature_unsupported";
static constexpr const char *kSessionErrorAuthorizationDenied = "authorization_denied";
static constexpr const char *kSessionErrorAuthorizationPending = "authorization_pending";
static constexpr const char *kSessionErrorSessionConfigRejected = "session_config_rejected";
static constexpr const char *kSessionErrorInvalidMessage = "invalid_message";
static constexpr int32_t kDefaultSessionPort = 27182;
static constexpr int32_t kDefaultVideoPort = 27183;

struct CompanionDescriptor {
    int shell_api_version;
    int protocol_major;
    int protocol_minor;
    std::string companion_name;
    std::string native_core_version;
    std::vector<std::string> shell_responsibilities;
    std::vector<std::string> native_responsibilities;
    std::vector<std::string> planned_modules;
    std::vector<std::string> supported_session_channels;
    std::vector<std::string> supported_video_codecs;
};

struct AuthorizationStateMap {
    std::string video_capture;
    std::string input_injection;
};

struct VideoCodecDescriptor {
    std::string codec;
    std::string encoder_kind;
    int32_t max_width;
    int32_t max_height;
    int32_t max_fps;
    std::string bitrate_control;
};

struct DisplayInfo {
    int32_t width;
    int32_t height;
    int32_t rotation;
};

struct SessionNegotiationRequest {
    std::string session_id;
    std::vector<std::string> host_supported_video_codecs;
    std::vector<std::string> preferred_video_codecs;
    int32_t video_max_width;
    int32_t video_max_height;
    int32_t video_max_fps;
    bool has_video_bitrate_kbps;
    int32_t video_bitrate_kbps;
    bool has_video_iframe_interval_ms;
    int32_t video_iframe_interval_ms;
    bool control_enabled;
};

struct HostHelloRequest {
    std::string type;
    std::string session_id;
    int32_t protocol_major;
    int32_t protocol_minor;
    std::string host_version;
    std::vector<std::string> requested_features;
    std::vector<std::string> supported_video_codecs;
    std::vector<std::string> preferred_video_codecs;
    int32_t video_max_width;
    int32_t video_max_height;
    int32_t video_max_fps;
    bool has_video_bitrate_kbps;
    int32_t video_bitrate_kbps;
    bool has_video_iframe_interval_ms;
    int32_t video_iframe_interval_ms;
};

struct SessionConfigRequest {
    std::string session_id;
    std::string selected_video_codec;
    int32_t video_max_width;
    int32_t video_max_height;
    int32_t video_max_fps;
    bool has_video_bitrate_kbps;
    int32_t video_bitrate_kbps;
    bool has_video_iframe_interval_ms;
    int32_t video_iframe_interval_ms;
    bool control_enabled;
};

struct VideoConfig {
    int32_t max_width;
    int32_t max_height;
    int32_t max_fps;
    bool has_bitrate_kbps;
    int32_t bitrate_kbps;
    bool has_iframe_interval_ms;
    int32_t iframe_interval_ms;
};

struct ControlConfig {
    bool enabled;
};

struct NormalizedPosition {
    double x;
    double y;
};

struct ControlEventRequest {
    std::string type;
    std::string session_id;
    std::string event_type;
    int32_t sequence;
    bool has_position_norm;
    NormalizedPosition position_norm;
    bool has_pointer_id;
    int32_t pointer_id;
    std::string button;
    bool has_scroll_delta_x;
    int32_t scroll_delta_x;
    bool has_scroll_delta_y;
    int32_t scroll_delta_y;
    std::string key_code;
    std::string text;
    std::string device_action;
};

struct StopSessionRequest {
    std::string session_id;
    bool has_reason;
    std::string reason;
};

struct SessionConfigPreview {
    std::string type;
    std::string session_id;
    std::string selected_video_codec;
    VideoConfig video;
    ControlConfig control;
};

struct VideoNegotiationPreview {
    std::string selected_video_codec;
    bool fallback_available;
    std::string fallback_video_codec;
    bool fallback_used;
    std::string selection_mode;
    std::string selection_reason;
    std::vector<std::string> host_supported_video_codecs;
    std::vector<std::string> preferred_video_codecs;
    std::vector<std::string> shared_video_codecs;
    std::string selected_path_module;
};

struct ChannelBinding {
    std::string name;
    std::string state;
    std::string payload_type;
    std::string activation;
};

struct ChannelLayout {
    ChannelBinding session;
    ChannelBinding video;
};

struct DeviceHelloPreview {
    std::string type;
    std::string session_id;
    int protocol_major;
    int protocol_minor;
    std::string companion_version;
    std::string device_name;
    AuthorizationStateMap authorization;
    std::vector<std::string> available_features;
    std::vector<VideoCodecDescriptor> available_video_codecs;
    DisplayInfo display;
};

struct SessionReadyPreview {
    std::string type;
    std::string session_id;
    std::string selected_video_codec;
    ChannelLayout channel_layout;
    DisplayInfo display;
};

struct SessionErrorMessage {
    std::string type;
    std::string session_id;
    std::string code;
    std::string message;
    bool retryable;
};

struct VideoUnitPreview {
    std::string codec;
    std::vector<std::string> unit_fields;
    std::string delivery;
    std::string keyframe_strategy;
    std::string payload_semantics;
};

struct ControlBaselinePreview {
    std::string channel_name;
    std::string injector_backend;
    std::string execution_mode;
    std::vector<std::string> supported_event_types;
    std::vector<std::string> unsupported_event_types;
    std::vector<std::string> supported_device_actions;
    std::vector<std::string> pipeline_stages;
    std::vector<std::string> notes;
};

struct ControlHandlingPreview {
    std::string type;
    std::string session_id;
    std::string event_type;
    int32_t sequence;
    std::string route;
    bool accepted;
    std::string validation_result;
    std::string injection_status;
    std::string injection_target;
    std::vector<std::string> steps;
    std::vector<std::string> notes;
};

struct SessionNegotiationPreview {
    std::string phase;
    std::string session_id;
    DeviceHelloPreview device_hello;
    VideoNegotiationPreview negotiation;
    SessionConfigPreview session_config;
    SessionReadyPreview session_ready;
    ControlBaselinePreview control_baseline;
    VideoUnitPreview video_unit;
    std::vector<std::string> pipeline_stages;
    std::vector<std::string> notes;
};

struct SessionTransportEnvelope {
    std::string channel_name;
    std::string payload_type;
    std::string message_type;
    std::string payload_json;
};

struct SessionTransportState {
    std::string session_id;
    int32_t protocol_major;
    int32_t protocol_minor;
    std::string phase;
    AuthorizationStateMap authorization;
    std::vector<std::string> requested_features;
    std::vector<std::string> shared_video_codecs;
    std::vector<VideoCodecDescriptor> available_video_codecs;
    std::string selected_video_codec;
    bool control_enabled;
    ChannelLayout channel_layout;
};

struct SessionTransportOpenResult {
    SessionTransportState state;
    bool accepted;
    bool has_device_hello;
    DeviceHelloPreview device_hello;
    bool has_session_error;
    SessionErrorMessage session_error;
    VideoNegotiationPreview negotiation;
    SessionConfigRequest suggested_session_config;
    SessionTransportEnvelope outbound_message;
    std::vector<std::string> notes;
};

struct VideoTransportState {
    std::string session_id;
    std::string selected_video_codec;
    ChannelBinding binding;
    DisplayInfo display;
    std::string selected_path_module;
    std::vector<std::string> pipeline_stages;
    bool active;
};

struct SessionTransportConfigureResult {
    SessionTransportState state;
    bool accepted;
    bool has_session_ready;
    SessionReadyPreview session_ready;
    bool has_video_transport;
    VideoTransportState video_transport;
    bool has_session_error;
    SessionErrorMessage session_error;
    SessionTransportEnvelope outbound_message;
    std::vector<std::string> pipeline_stages;
    std::vector<std::string> notes;
};

struct SessionTransportStopResult {
    SessionTransportState state;
    bool accepted;
    std::vector<std::string> notes;
};

struct SessionListenerRuntimeStatus {
    bool running;
    bool listener_bound;
    int32_t session_port;
    std::string state;
    std::string last_error;
    std::string active_session_id;
};

struct VideoTransportActivationResult {
    VideoTransportState state;
    bool activated;
    std::vector<std::string> notes;
};

struct VideoPacketRequest {
    std::string session_id;
    std::string codec;
    int64_t pts_us;
    bool is_keyframe;
    int32_t payload_length;
};

struct VideoPacketEnvelope {
    std::string channel_name;
    std::string payload_type;
    std::string codec;
    int64_t pts_us;
    bool is_keyframe;
    int32_t payload_length;
    std::vector<std::string> header_fields;
    std::string delivery;
    std::string payload_semantics;
};

struct VideoPacketTransportResult {
    VideoTransportState state;
    bool accepted;
    VideoPacketEnvelope packet;
    std::vector<std::string> notes;
};

}  // namespace core
}  // namespace hscrcpy

#endif
