#include "bridge/companion_napi.h"

#include <algorithm>
#include <string>
#include <vector>

#include "bridge/napi_helpers.h"
#include "core/companion_core.h"
#include "transport/session_channel.h"

namespace hscrcpy {
namespace bridge {

namespace {

bool ContainsCodec(const std::vector<std::string> &codecs, const std::string &codec)
{
    return std::find(codecs.begin(), codecs.end(), codec) != codecs.end();
}

bool GetRequiredObjectProperty(napi_env env, napi_value object, const char *field_name, napi_value *result)
{
    bool has_property = false;
    if (!CheckStatus(env, napi_has_named_property(env, object, field_name, &has_property), "napi_has_named_property")) {
        return false;
    }
    if (!has_property) {
        return ThrowTypeError(env, field_name, "an object");
    }

    if (!CheckStatus(env, napi_get_named_property(env, object, field_name, result), "napi_get_named_property")) {
        return false;
    }

    napi_valuetype value_type = napi_undefined;
    if (!CheckStatus(env, napi_typeof(env, *result, &value_type), "napi_typeof")) {
        return false;
    }
    if (value_type != napi_object) {
        return ThrowTypeError(env, field_name, "an object");
    }
    return true;
}

bool GetRequiredInt64Property(napi_env env, napi_value object, const char *field_name, int64_t *result)
{
    bool has_property = false;
    if (!CheckStatus(env, napi_has_named_property(env, object, field_name, &has_property), "napi_has_named_property")) {
        return false;
    }
    if (!has_property) {
        return ThrowTypeError(env, field_name, "a number");
    }

    napi_value field = nullptr;
    if (!CheckStatus(env, napi_get_named_property(env, object, field_name, &field), "napi_get_named_property")) {
        return false;
    }

    napi_valuetype value_type = napi_undefined;
    if (!CheckStatus(env, napi_typeof(env, field, &value_type), "napi_typeof")) {
        return false;
    }
    if (value_type != napi_number) {
        return ThrowTypeError(env, field_name, "a number");
    }

    return CheckStatus(env, napi_get_value_int64(env, field, result), "napi_get_value_int64");
}

napi_value CreateInt64Value(napi_env env, int64_t value)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_int64(env, value, &result), "napi_create_int64")) {
        return nullptr;
    }
    return result;
}

bool ReadAuthorizationStateMap(napi_env env, napi_value value, core::AuthorizationStateMap *authorization)
{
    return GetRequiredStringProperty(env, value, "videoCapture", &authorization->video_capture) &&
        GetRequiredStringProperty(env, value, "inputInjection", &authorization->input_injection);
}

bool ReadDisplayInfo(napi_env env, napi_value value, core::DisplayInfo *display)
{
    return GetRequiredInt32Property(env, value, "width", &display->width) &&
        GetRequiredInt32Property(env, value, "height", &display->height) &&
        GetRequiredInt32Property(env, value, "rotation", &display->rotation);
}

bool ReadChannelBinding(napi_env env, napi_value value, core::ChannelBinding *binding)
{
    return GetRequiredStringProperty(env, value, "name", &binding->name) &&
        GetRequiredStringProperty(env, value, "state", &binding->state) &&
        GetRequiredStringProperty(env, value, "payloadType", &binding->payload_type) &&
        GetRequiredStringProperty(env, value, "activation", &binding->activation);
}

bool ReadChannelLayout(napi_env env, napi_value value, core::ChannelLayout *layout)
{
    napi_value session = nullptr;
    napi_value video = nullptr;
    return GetRequiredObjectProperty(env, value, "session", &session) &&
        GetRequiredObjectProperty(env, value, "video", &video) &&
        ReadChannelBinding(env, session, &layout->session) &&
        ReadChannelBinding(env, video, &layout->video);
}

bool ReadVideoCodecDescriptorArray(
    napi_env env, napi_value object, const char *field_name, std::vector<core::VideoCodecDescriptor> *codecs)
{
    bool has_property = false;
    if (!CheckStatus(env, napi_has_named_property(env, object, field_name, &has_property), "napi_has_named_property")) {
        return false;
    }
    if (!has_property) {
        codecs->clear();
        return true;
    }

    napi_value field = nullptr;
    if (!CheckStatus(env, napi_get_named_property(env, object, field_name, &field), "napi_get_named_property")) {
        return false;
    }

    bool is_array = false;
    if (!CheckStatus(env, napi_is_array(env, field, &is_array), "napi_is_array")) {
        return false;
    }
    if (!is_array) {
        return ThrowTypeError(env, field_name, "an array");
    }

    uint32_t length = 0;
    if (!CheckStatus(env, napi_get_array_length(env, field, &length), "napi_get_array_length")) {
        return false;
    }

    codecs->clear();
    codecs->reserve(length);
    for (uint32_t index = 0; index < length; ++index) {
        napi_value item = nullptr;
        if (!CheckStatus(env, napi_get_element(env, field, index, &item), "napi_get_element")) {
            return false;
        }

        core::VideoCodecDescriptor codec = {};
        if (!GetRequiredStringProperty(env, item, "codec", &codec.codec) ||
            !GetRequiredStringProperty(env, item, "encoderKind", &codec.encoder_kind) ||
            !GetRequiredInt32Property(env, item, "maxWidth", &codec.max_width) ||
            !GetRequiredInt32Property(env, item, "maxHeight", &codec.max_height) ||
            !GetRequiredInt32Property(env, item, "maxFps", &codec.max_fps)) {
            return false;
        }

        bool has_bitrate_control = false;
        if (!GetOptionalStringProperty(env, item, "bitrateControl", &codec.bitrate_control, &has_bitrate_control)) {
            return false;
        }
        codecs->push_back(codec);
    }

    return true;
}

napi_value CreateAuthorizationValue(napi_env env, const core::AuthorizationStateMap &authorization)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "videoCapture", CreateString(env, authorization.video_capture)) ||
        !SetNamedProperty(env, result, "inputInjection", CreateString(env, authorization.input_injection))) {
        return nullptr;
    }

    return result;
}

napi_value CreateDisplayInfoValue(napi_env env, const core::DisplayInfo &display)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "width", CreateInt32(env, display.width)) ||
        !SetNamedProperty(env, result, "height", CreateInt32(env, display.height)) ||
        !SetNamedProperty(env, result, "rotation", CreateInt32(env, display.rotation))) {
        return nullptr;
    }

    return result;
}

napi_value CreateVideoCodecDescriptorValue(napi_env env, const core::VideoCodecDescriptor &codec)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "codec", CreateString(env, codec.codec)) ||
        !SetNamedProperty(env, result, "encoderKind", CreateString(env, codec.encoder_kind)) ||
        !SetNamedProperty(env, result, "maxWidth", CreateInt32(env, codec.max_width)) ||
        !SetNamedProperty(env, result, "maxHeight", CreateInt32(env, codec.max_height)) ||
        !SetNamedProperty(env, result, "maxFps", CreateInt32(env, codec.max_fps))) {
        return nullptr;
    }

    if (!codec.bitrate_control.empty() &&
        !SetNamedProperty(env, result, "bitrateControl", CreateString(env, codec.bitrate_control))) {
        return nullptr;
    }

    return result;
}

napi_value CreateVideoCodecDescriptorArray(napi_env env, const std::vector<core::VideoCodecDescriptor> &codecs)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_array_with_length(env, codecs.size(), &result), "napi_create_array_with_length")) {
        return nullptr;
    }

    for (size_t index = 0; index < codecs.size(); ++index) {
        napi_value codec = CreateVideoCodecDescriptorValue(env, codecs[index]);
        if (codec == nullptr) {
            return nullptr;
        }
        if (!CheckStatus(env, napi_set_element(env, result, index, codec), "napi_set_element")) {
            return nullptr;
        }
    }

    return result;
}

napi_value CreateChannelBindingValue(napi_env env, const core::ChannelBinding &binding)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "name", CreateString(env, binding.name)) ||
        !SetNamedProperty(env, result, "state", CreateString(env, binding.state)) ||
        !SetNamedProperty(env, result, "payloadType", CreateString(env, binding.payload_type)) ||
        !SetNamedProperty(env, result, "activation", CreateString(env, binding.activation))) {
        return nullptr;
    }

    return result;
}

napi_value CreateChannelLayoutValue(napi_env env, const core::ChannelLayout &layout)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "session", CreateChannelBindingValue(env, layout.session)) ||
        !SetNamedProperty(env, result, "video", CreateChannelBindingValue(env, layout.video))) {
        return nullptr;
    }

    return result;
}

napi_value CreateVideoConfigValue(napi_env env, const core::VideoConfig &video)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "maxWidth", CreateInt32(env, video.max_width)) ||
        !SetNamedProperty(env, result, "maxHeight", CreateInt32(env, video.max_height)) ||
        !SetNamedProperty(env, result, "maxFps", CreateInt32(env, video.max_fps))) {
        return nullptr;
    }

    if (video.has_bitrate_kbps &&
        !SetNamedProperty(env, result, "bitrateKbps", CreateInt32(env, video.bitrate_kbps))) {
        return nullptr;
    }
    if (video.has_iframe_interval_ms &&
        !SetNamedProperty(env, result, "iframeIntervalMs", CreateInt32(env, video.iframe_interval_ms))) {
        return nullptr;
    }

    return result;
}

napi_value CreateControlConfigValue(napi_env env, const core::ControlConfig &control)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "enabled", CreateBoolean(env, control.enabled))) {
        return nullptr;
    }

    return result;
}

napi_value CreateControlBaselinePreviewValue(napi_env env, const core::ControlBaselinePreview &preview)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "channelName", CreateString(env, preview.channel_name)) ||
        !SetNamedProperty(env, result, "injectorBackend", CreateString(env, preview.injector_backend)) ||
        !SetNamedProperty(env, result, "executionMode", CreateString(env, preview.execution_mode)) ||
        !SetNamedProperty(env, result, "supportedEventTypes", CreateStringArray(env, preview.supported_event_types)) ||
        !SetNamedProperty(env, result, "unsupportedEventTypes", CreateStringArray(env, preview.unsupported_event_types)) ||
        !SetNamedProperty(env, result, "supportedDeviceActions", CreateStringArray(env, preview.supported_device_actions)) ||
        !SetNamedProperty(env, result, "pipelineStages", CreateStringArray(env, preview.pipeline_stages)) ||
        !SetNamedProperty(env, result, "notes", CreateStringArray(env, preview.notes))) {
        return nullptr;
    }

    return result;
}

napi_value CreateControlHandlingPreviewValue(napi_env env, const core::ControlHandlingPreview &preview)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "type", CreateString(env, preview.type)) ||
        !SetNamedProperty(env, result, "sessionId", CreateString(env, preview.session_id)) ||
        !SetNamedProperty(env, result, "eventType", CreateString(env, preview.event_type)) ||
        !SetNamedProperty(env, result, "sequence", CreateInt32(env, preview.sequence)) ||
        !SetNamedProperty(env, result, "route", CreateString(env, preview.route)) ||
        !SetNamedProperty(env, result, "accepted", CreateBoolean(env, preview.accepted)) ||
        !SetNamedProperty(env, result, "validationResult", CreateString(env, preview.validation_result)) ||
        !SetNamedProperty(env, result, "injectionStatus", CreateString(env, preview.injection_status)) ||
        !SetNamedProperty(env, result, "injectionTarget", CreateString(env, preview.injection_target)) ||
        !SetNamedProperty(env, result, "steps", CreateStringArray(env, preview.steps)) ||
        !SetNamedProperty(env, result, "notes", CreateStringArray(env, preview.notes))) {
        return nullptr;
    }

    return result;
}

napi_value CreateCompanionDescriptorValue(napi_env env, const core::CompanionDescriptor &descriptor)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "shellApiVersion", CreateInt32(env, descriptor.shell_api_version)) ||
        !SetNamedProperty(env, result, "protocolMajor", CreateInt32(env, descriptor.protocol_major)) ||
        !SetNamedProperty(env, result, "protocolMinor", CreateInt32(env, descriptor.protocol_minor)) ||
        !SetNamedProperty(env, result, "companionName", CreateString(env, descriptor.companion_name)) ||
        !SetNamedProperty(env, result, "nativeCoreVersion", CreateString(env, descriptor.native_core_version)) ||
        !SetNamedProperty(env, result, "shellResponsibilities", CreateStringArray(env, descriptor.shell_responsibilities)) ||
        !SetNamedProperty(env, result, "nativeResponsibilities", CreateStringArray(env, descriptor.native_responsibilities)) ||
        !SetNamedProperty(env, result, "plannedModules", CreateStringArray(env, descriptor.planned_modules)) ||
        !SetNamedProperty(env, result, "supportedSessionChannels", CreateStringArray(env, descriptor.supported_session_channels)) ||
        !SetNamedProperty(env, result, "supportedVideoCodecs", CreateStringArray(env, descriptor.supported_video_codecs))) {
        return nullptr;
    }

    return result;
}

napi_value CreateSessionConfigPreviewValue(napi_env env, const core::SessionConfigPreview &preview)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "type", CreateString(env, preview.type)) ||
        !SetNamedProperty(env, result, "sessionId", CreateString(env, preview.session_id)) ||
        !SetNamedProperty(env, result, "selectedVideoCodec", CreateString(env, preview.selected_video_codec)) ||
        !SetNamedProperty(env, result, "video", CreateVideoConfigValue(env, preview.video)) ||
        !SetNamedProperty(env, result, "control", CreateControlConfigValue(env, preview.control))) {
        return nullptr;
    }

    return result;
}

napi_value CreateVideoNegotiationPreviewValue(napi_env env, const core::VideoNegotiationPreview &preview)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "selectedVideoCodec", CreateString(env, preview.selected_video_codec)) ||
        !SetNamedProperty(env, result, "fallbackAvailable", CreateBoolean(env, preview.fallback_available)) ||
        !SetNamedProperty(env, result, "fallbackVideoCodec", CreateString(env, preview.fallback_video_codec)) ||
        !SetNamedProperty(env, result, "fallbackUsed", CreateBoolean(env, preview.fallback_used)) ||
        !SetNamedProperty(env, result, "selectionMode", CreateString(env, preview.selection_mode)) ||
        !SetNamedProperty(env, result, "selectionReason", CreateString(env, preview.selection_reason)) ||
        !SetNamedProperty(env, result, "hostSupportedVideoCodecs", CreateStringArray(env, preview.host_supported_video_codecs)) ||
        !SetNamedProperty(env, result, "preferredVideoCodecs", CreateStringArray(env, preview.preferred_video_codecs)) ||
        !SetNamedProperty(env, result, "sharedVideoCodecs", CreateStringArray(env, preview.shared_video_codecs)) ||
        !SetNamedProperty(env, result, "selectedPathModule", CreateString(env, preview.selected_path_module))) {
        return nullptr;
    }

    return result;
}

napi_value CreateDeviceHelloPreviewValue(napi_env env, const core::DeviceHelloPreview &preview)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "type", CreateString(env, preview.type)) ||
        !SetNamedProperty(env, result, "sessionId", CreateString(env, preview.session_id)) ||
        !SetNamedProperty(env, result, "protocolMajor", CreateInt32(env, preview.protocol_major)) ||
        !SetNamedProperty(env, result, "protocolMinor", CreateInt32(env, preview.protocol_minor)) ||
        !SetNamedProperty(env, result, "companionVersion", CreateString(env, preview.companion_version)) ||
        !SetNamedProperty(env, result, "deviceName", CreateString(env, preview.device_name)) ||
        !SetNamedProperty(env, result, "authorization", CreateAuthorizationValue(env, preview.authorization)) ||
        !SetNamedProperty(env, result, "availableFeatures", CreateStringArray(env, preview.available_features)) ||
        !SetNamedProperty(env, result, "availableVideoCodecs", CreateVideoCodecDescriptorArray(env, preview.available_video_codecs)) ||
        !SetNamedProperty(env, result, "display", CreateDisplayInfoValue(env, preview.display))) {
        return nullptr;
    }

    return result;
}

napi_value CreateSessionReadyPreviewValue(napi_env env, const core::SessionReadyPreview &preview)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "type", CreateString(env, preview.type)) ||
        !SetNamedProperty(env, result, "sessionId", CreateString(env, preview.session_id)) ||
        !SetNamedProperty(env, result, "selectedVideoCodec", CreateString(env, preview.selected_video_codec)) ||
        !SetNamedProperty(env, result, "channelLayout", CreateChannelLayoutValue(env, preview.channel_layout)) ||
        !SetNamedProperty(env, result, "display", CreateDisplayInfoValue(env, preview.display))) {
        return nullptr;
    }

    return result;
}

napi_value CreateSessionErrorValue(napi_env env, const core::SessionErrorMessage &session_error)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "type", CreateString(env, session_error.type)) ||
        !SetNamedProperty(env, result, "sessionId", CreateString(env, session_error.session_id)) ||
        !SetNamedProperty(env, result, "code", CreateString(env, session_error.code)) ||
        !SetNamedProperty(env, result, "message", CreateString(env, session_error.message)) ||
        !SetNamedProperty(env, result, "retryable", CreateBoolean(env, session_error.retryable))) {
        return nullptr;
    }

    return result;
}

napi_value CreateVideoUnitPreviewValue(napi_env env, const core::VideoUnitPreview &preview)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "codec", CreateString(env, preview.codec)) ||
        !SetNamedProperty(env, result, "unitFields", CreateStringArray(env, preview.unit_fields)) ||
        !SetNamedProperty(env, result, "delivery", CreateString(env, preview.delivery)) ||
        !SetNamedProperty(env, result, "keyframeStrategy", CreateString(env, preview.keyframe_strategy)) ||
        !SetNamedProperty(env, result, "payloadSemantics", CreateString(env, preview.payload_semantics))) {
        return nullptr;
    }

    return result;
}

napi_value CreateSessionNegotiationPreviewValue(napi_env env, const core::SessionNegotiationPreview &preview)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "phase", CreateString(env, preview.phase)) ||
        !SetNamedProperty(env, result, "sessionId", CreateString(env, preview.session_id)) ||
        !SetNamedProperty(env, result, "deviceHello", CreateDeviceHelloPreviewValue(env, preview.device_hello)) ||
        !SetNamedProperty(env, result, "negotiation", CreateVideoNegotiationPreviewValue(env, preview.negotiation)) ||
        !SetNamedProperty(env, result, "sessionConfig", CreateSessionConfigPreviewValue(env, preview.session_config)) ||
        !SetNamedProperty(env, result, "sessionReady", CreateSessionReadyPreviewValue(env, preview.session_ready)) ||
        !SetNamedProperty(env, result, "controlBaseline", CreateControlBaselinePreviewValue(env, preview.control_baseline)) ||
        !SetNamedProperty(env, result, "videoUnit", CreateVideoUnitPreviewValue(env, preview.video_unit)) ||
        !SetNamedProperty(env, result, "pipelineStages", CreateStringArray(env, preview.pipeline_stages)) ||
        !SetNamedProperty(env, result, "notes", CreateStringArray(env, preview.notes))) {
        return nullptr;
    }

    return result;
}

napi_value CreateSessionTransportEnvelopeValue(napi_env env, const core::SessionTransportEnvelope &envelope)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "channelName", CreateString(env, envelope.channel_name)) ||
        !SetNamedProperty(env, result, "payloadType", CreateString(env, envelope.payload_type)) ||
        !SetNamedProperty(env, result, "messageType", CreateString(env, envelope.message_type)) ||
        !SetNamedProperty(env, result, "payloadJson", CreateString(env, envelope.payload_json))) {
        return nullptr;
    }

    return result;
}

napi_value CreateSessionTransportStateValue(napi_env env, const core::SessionTransportState &state)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "sessionId", CreateString(env, state.session_id)) ||
        !SetNamedProperty(env, result, "protocolMajor", CreateInt32(env, state.protocol_major)) ||
        !SetNamedProperty(env, result, "protocolMinor", CreateInt32(env, state.protocol_minor)) ||
        !SetNamedProperty(env, result, "phase", CreateString(env, state.phase)) ||
        !SetNamedProperty(env, result, "authorization", CreateAuthorizationValue(env, state.authorization)) ||
        !SetNamedProperty(env, result, "requestedFeatures", CreateStringArray(env, state.requested_features)) ||
        !SetNamedProperty(env, result, "sharedVideoCodecs", CreateStringArray(env, state.shared_video_codecs)) ||
        !SetNamedProperty(env, result, "availableVideoCodecs", CreateVideoCodecDescriptorArray(env, state.available_video_codecs)) ||
        !SetNamedProperty(env, result, "selectedVideoCodec", CreateString(env, state.selected_video_codec)) ||
        !SetNamedProperty(env, result, "controlEnabled", CreateBoolean(env, state.control_enabled)) ||
        !SetNamedProperty(env, result, "channelLayout", CreateChannelLayoutValue(env, state.channel_layout))) {
        return nullptr;
    }

    return result;
}

napi_value CreateSessionListenerRuntimeStatusValue(
    napi_env env, const core::SessionListenerRuntimeStatus &status)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "running", CreateBoolean(env, status.running)) ||
        !SetNamedProperty(env, result, "listenerBound", CreateBoolean(env, status.listener_bound)) ||
        !SetNamedProperty(env, result, "sessionPort", CreateInt32(env, status.session_port)) ||
        !SetNamedProperty(env, result, "state", CreateString(env, status.state)) ||
        !SetNamedProperty(env, result, "lastError", CreateString(env, status.last_error)) ||
        !SetNamedProperty(env, result, "activeSessionId", CreateString(env, status.active_session_id))) {
        return nullptr;
    }

    return result;
}

napi_value CreateVideoTransportStateValue(napi_env env, const core::VideoTransportState &state)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "sessionId", CreateString(env, state.session_id)) ||
        !SetNamedProperty(env, result, "selectedVideoCodec", CreateString(env, state.selected_video_codec)) ||
        !SetNamedProperty(env, result, "binding", CreateChannelBindingValue(env, state.binding)) ||
        !SetNamedProperty(env, result, "display", CreateDisplayInfoValue(env, state.display)) ||
        !SetNamedProperty(env, result, "config", CreateVideoConfigValue(env, state.config)) ||
        !SetNamedProperty(env, result, "selectedPathModule", CreateString(env, state.selected_path_module)) ||
        !SetNamedProperty(env, result, "pipelineStages", CreateStringArray(env, state.pipeline_stages)) ||
        !SetNamedProperty(env, result, "active", CreateBoolean(env, state.active))) {
        return nullptr;
    }

    return result;
}

napi_value CreateVideoTransportActivationResultValue(napi_env env, const core::VideoTransportActivationResult &result_value)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "state", CreateVideoTransportStateValue(env, result_value.state)) ||
        !SetNamedProperty(env, result, "activated", CreateBoolean(env, result_value.activated)) ||
        !SetNamedProperty(env, result, "notes", CreateStringArray(env, result_value.notes))) {
        return nullptr;
    }

    return result;
}

napi_value CreateVideoPacketEnvelopeValue(napi_env env, const core::VideoPacketEnvelope &packet)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "channelName", CreateString(env, packet.channel_name)) ||
        !SetNamedProperty(env, result, "payloadType", CreateString(env, packet.payload_type)) ||
        !SetNamedProperty(env, result, "codec", CreateString(env, packet.codec)) ||
        !SetNamedProperty(env, result, "ptsUs", CreateInt64Value(env, packet.pts_us)) ||
        !SetNamedProperty(env, result, "isKeyframe", CreateBoolean(env, packet.is_keyframe)) ||
        !SetNamedProperty(env, result, "payloadLength", CreateInt32(env, packet.payload_length)) ||
        !SetNamedProperty(env, result, "headerFields", CreateStringArray(env, packet.header_fields)) ||
        !SetNamedProperty(env, result, "delivery", CreateString(env, packet.delivery)) ||
        !SetNamedProperty(env, result, "payloadSemantics", CreateString(env, packet.payload_semantics))) {
        return nullptr;
    }

    return result;
}

napi_value CreateVideoPacketTransportResultValue(napi_env env, const core::VideoPacketTransportResult &result_value)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "state", CreateVideoTransportStateValue(env, result_value.state)) ||
        !SetNamedProperty(env, result, "accepted", CreateBoolean(env, result_value.accepted)) ||
        !SetNamedProperty(env, result, "notes", CreateStringArray(env, result_value.notes))) {
        return nullptr;
    }

    if (result_value.accepted &&
        !SetNamedProperty(env, result, "packet", CreateVideoPacketEnvelopeValue(env, result_value.packet))) {
        return nullptr;
    }

    return result;
}

napi_value CreateSessionTransportOpenResultValue(napi_env env, const core::SessionTransportOpenResult &result_value)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "state", CreateSessionTransportStateValue(env, result_value.state)) ||
        !SetNamedProperty(env, result, "accepted", CreateBoolean(env, result_value.accepted)) ||
        !SetNamedProperty(env, result, "negotiation", CreateVideoNegotiationPreviewValue(env, result_value.negotiation)) ||
        !SetNamedProperty(env, result, "suggestedSessionConfig", CreateSessionConfigPreviewValue(
            env, transport::BuildSessionConfigPreview(result_value.suggested_session_config))) ||
        !SetNamedProperty(env, result, "outboundMessage", CreateSessionTransportEnvelopeValue(env, result_value.outbound_message)) ||
        !SetNamedProperty(env, result, "notes", CreateStringArray(env, result_value.notes))) {
        return nullptr;
    }

    if (result_value.has_device_hello &&
        !SetNamedProperty(env, result, "deviceHello", CreateDeviceHelloPreviewValue(env, result_value.device_hello))) {
        return nullptr;
    }
    if (result_value.has_session_error &&
        !SetNamedProperty(env, result, "sessionError", CreateSessionErrorValue(env, result_value.session_error))) {
        return nullptr;
    }

    return result;
}

napi_value CreateSessionTransportConfigureResultValue(napi_env env, const core::SessionTransportConfigureResult &result_value)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "state", CreateSessionTransportStateValue(env, result_value.state)) ||
        !SetNamedProperty(env, result, "accepted", CreateBoolean(env, result_value.accepted)) ||
        !SetNamedProperty(env, result, "outboundMessage", CreateSessionTransportEnvelopeValue(env, result_value.outbound_message)) ||
        !SetNamedProperty(env, result, "pipelineStages", CreateStringArray(env, result_value.pipeline_stages)) ||
        !SetNamedProperty(env, result, "notes", CreateStringArray(env, result_value.notes))) {
        return nullptr;
    }

    if (result_value.has_session_ready &&
        !SetNamedProperty(env, result, "sessionReady", CreateSessionReadyPreviewValue(env, result_value.session_ready))) {
        return nullptr;
    }
    if (result_value.has_video_transport &&
        !SetNamedProperty(env, result, "videoTransport", CreateVideoTransportStateValue(env, result_value.video_transport))) {
        return nullptr;
    }
    if (result_value.has_session_error &&
        !SetNamedProperty(env, result, "sessionError", CreateSessionErrorValue(env, result_value.session_error))) {
        return nullptr;
    }

    return result;
}

napi_value CreateSessionTransportStopResultValue(napi_env env, const core::SessionTransportStopResult &result_value)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_object(env, &result), "napi_create_object")) {
        return nullptr;
    }

    if (!SetNamedProperty(env, result, "state", CreateSessionTransportStateValue(env, result_value.state)) ||
        !SetNamedProperty(env, result, "accepted", CreateBoolean(env, result_value.accepted)) ||
        !SetNamedProperty(env, result, "notes", CreateStringArray(env, result_value.notes))) {
        return nullptr;
    }

    return result;
}

bool ReadSessionNegotiationRequest(napi_env env, napi_callback_info info, core::SessionNegotiationRequest *request)
{
    size_t argc = 1;
    napi_value args[1] = {nullptr};
    if (!CheckStatus(env, napi_get_cb_info(env, info, &argc, args, nullptr, nullptr), "napi_get_cb_info")) {
        return false;
    }
    if (argc != 1) {
        napi_throw_type_error(env, nullptr, "previewSessionNegotiation expects one request object");
        return false;
    }

    napi_valuetype value_type = napi_undefined;
    if (!CheckStatus(env, napi_typeof(env, args[0], &value_type), "napi_typeof")) {
        return false;
    }
    if (value_type != napi_object) {
        napi_throw_type_error(env, nullptr, "previewSessionNegotiation expects an object request");
        return false;
    }

    if (!GetRequiredStringProperty(env, args[0], "sessionId", &request->session_id) ||
        !GetRequiredStringArrayProperty(env, args[0], "hostSupportedVideoCodecs", &request->host_supported_video_codecs) ||
        !GetRequiredStringArrayProperty(env, args[0], "preferredVideoCodecs", &request->preferred_video_codecs) ||
        !GetRequiredInt32Property(env, args[0], "videoMaxWidth", &request->video_max_width) ||
        !GetRequiredInt32Property(env, args[0], "videoMaxHeight", &request->video_max_height) ||
        !GetRequiredInt32Property(env, args[0], "videoMaxFps", &request->video_max_fps) ||
        !GetRequiredBoolProperty(env, args[0], "controlEnabled", &request->control_enabled) ||
        !GetOptionalInt32Property(env, args[0], "bitrateKbps", &request->video_bitrate_kbps, &request->has_video_bitrate_kbps) ||
        !GetOptionalInt32Property(
            env,
            args[0],
            "iframeIntervalMs",
            &request->video_iframe_interval_ms,
            &request->has_video_iframe_interval_ms)) {
        return false;
    }

    if (request->session_id.empty()) {
        napi_throw_type_error(env, nullptr, "sessionId must not be empty");
        return false;
    }
    if (request->host_supported_video_codecs.empty() || request->preferred_video_codecs.empty()) {
        napi_throw_type_error(env, nullptr, "hostSupportedVideoCodecs and preferredVideoCodecs must not be empty");
        return false;
    }
    if (request->video_max_width <= 0 || request->video_max_height <= 0 || request->video_max_fps <= 0) {
        napi_throw_type_error(env, nullptr, "videoMaxWidth, videoMaxHeight, and videoMaxFps must be positive");
        return false;
    }
    if (request->has_video_bitrate_kbps && request->video_bitrate_kbps <= 0) {
        napi_throw_type_error(env, nullptr, "bitrateKbps must be positive when provided");
        return false;
    }
    if (request->has_video_iframe_interval_ms && request->video_iframe_interval_ms <= 0) {
        napi_throw_type_error(env, nullptr, "iframeIntervalMs must be positive when provided");
        return false;
    }

    for (const std::string &preferred_codec : request->preferred_video_codecs) {
        if (!ContainsCodec(request->host_supported_video_codecs, preferred_codec)) {
            napi_throw_type_error(env, nullptr, "preferredVideoCodecs must be a subset of hostSupportedVideoCodecs");
            return false;
        }
    }
    if (!core::HasSharedVideoCodec(*request)) {
        napi_throw_type_error(env, nullptr, "No shared video codec exists between the host request and device preview");
        return false;
    }

    return true;
}

bool ReadGenericSessionConfigObject(napi_env env, napi_value value, core::SessionConfigRequest *request)
{
    if (!GetRequiredStringProperty(env, value, "sessionId", &request->session_id) ||
        !GetRequiredStringProperty(env, value, "selectedVideoCodec", &request->selected_video_codec) ||
        !GetRequiredInt32Property(env, value, "videoMaxWidth", &request->video_max_width) ||
        !GetRequiredInt32Property(env, value, "videoMaxHeight", &request->video_max_height) ||
        !GetRequiredInt32Property(env, value, "videoMaxFps", &request->video_max_fps) ||
        !GetRequiredBoolProperty(env, value, "controlEnabled", &request->control_enabled) ||
        !GetOptionalInt32Property(env, value, "bitrateKbps", &request->video_bitrate_kbps, &request->has_video_bitrate_kbps) ||
        !GetOptionalInt32Property(
            env,
            value,
            "iframeIntervalMs",
            &request->video_iframe_interval_ms,
            &request->has_video_iframe_interval_ms)) {
        return false;
    }

    if (request->session_id.empty() || request->selected_video_codec.empty()) {
        napi_throw_type_error(env, nullptr, "sessionId and selectedVideoCodec must not be empty");
        return false;
    }
    if (request->video_max_width <= 0 || request->video_max_height <= 0 || request->video_max_fps <= 0) {
        napi_throw_type_error(env, nullptr, "videoMaxWidth, videoMaxHeight, and videoMaxFps must be positive");
        return false;
    }
    if (request->has_video_bitrate_kbps && request->video_bitrate_kbps <= 0) {
        napi_throw_type_error(env, nullptr, "bitrateKbps must be positive when provided");
        return false;
    }
    if (request->has_video_iframe_interval_ms && request->video_iframe_interval_ms <= 0) {
        napi_throw_type_error(env, nullptr, "iframeIntervalMs must be positive when provided");
        return false;
    }

    return true;
}

bool ReadJpegSessionConfigRequest(napi_env env, napi_callback_info info, core::SessionConfigRequest *request)
{
    size_t argc = 1;
    napi_value args[1] = {nullptr};
    if (!CheckStatus(env, napi_get_cb_info(env, info, &argc, args, nullptr, nullptr), "napi_get_cb_info")) {
        return false;
    }
    if (argc != 1) {
        napi_throw_type_error(env, nullptr, "previewJpegSessionPath expects one request object");
        return false;
    }

    napi_valuetype value_type = napi_undefined;
    if (!CheckStatus(env, napi_typeof(env, args[0], &value_type), "napi_typeof")) {
        return false;
    }
    if (value_type != napi_object) {
        napi_throw_type_error(env, nullptr, "previewJpegSessionPath expects an object request");
        return false;
    }

    if (!ReadGenericSessionConfigObject(env, args[0], request)) {
        return false;
    }
    if (request->selected_video_codec != core::kVideoCodecJpeg) {
        napi_throw_type_error(env, nullptr, "selectedVideoCodec must be jpeg");
        return false;
    }
    request->has_video_bitrate_kbps = false;
    request->video_bitrate_kbps = 0;
    request->has_video_iframe_interval_ms = false;
    request->video_iframe_interval_ms = 0;
    return true;
}

bool ReadControlEventRequest(napi_env env, napi_value value, core::ControlEventRequest *request)
{
    if (!GetRequiredStringProperty(env, value, "type", &request->type) ||
        !GetRequiredStringProperty(env, value, "sessionId", &request->session_id) ||
        !GetRequiredStringProperty(env, value, "eventType", &request->event_type) ||
        !GetRequiredInt32Property(env, value, "sequence", &request->sequence)) {
        return false;
    }

    napi_value position_norm = nullptr;
    if (!GetOptionalObjectProperty(env, value, "positionNorm", &position_norm, &request->has_position_norm)) {
        return false;
    }
    if (request->has_position_norm) {
        bool has_x = false;
        bool has_y = false;
        if (!GetOptionalDoubleProperty(env, position_norm, "x", &request->position_norm.x, &has_x) ||
            !GetOptionalDoubleProperty(env, position_norm, "y", &request->position_norm.y, &has_y)) {
            return false;
        }
        if (!has_x || !has_y) {
            napi_throw_type_error(env, nullptr, "positionNorm requires both x and y when present");
            return false;
        }
    }

    if (!GetOptionalInt32Property(env, value, "pointerId", &request->pointer_id, &request->has_pointer_id) ||
        !GetOptionalInt32Property(env, value, "scrollDeltaX", &request->scroll_delta_x, &request->has_scroll_delta_x) ||
        !GetOptionalInt32Property(env, value, "scrollDeltaY", &request->scroll_delta_y, &request->has_scroll_delta_y)) {
        return false;
    }

    bool has_button = false;
    bool has_key_code = false;
    bool has_text = false;
    bool has_device_action = false;
    if (!GetOptionalStringProperty(env, value, "button", &request->button, &has_button) ||
        !GetOptionalStringProperty(env, value, "keyCode", &request->key_code, &has_key_code) ||
        !GetOptionalStringProperty(env, value, "text", &request->text, &has_text) ||
        !GetOptionalStringProperty(env, value, "deviceAction", &request->device_action, &has_device_action)) {
        return false;
    }

    return true;
}

bool ReadPreviewControlHandlingRequest(napi_env env, napi_callback_info info, core::ControlEventRequest *request)
{
    size_t argc = 1;
    napi_value args[1] = {nullptr};
    if (!CheckStatus(env, napi_get_cb_info(env, info, &argc, args, nullptr, nullptr), "napi_get_cb_info")) {
        return false;
    }
    if (argc != 1) {
        napi_throw_type_error(env, nullptr, "previewControlHandling expects one control_event object");
        return false;
    }
    return ReadControlEventRequest(env, args[0], request);
}

bool ReadHostHelloRequest(napi_env env, napi_callback_info info, core::HostHelloRequest *request)
{
    size_t argc = 1;
    napi_value args[1] = {nullptr};
    if (!CheckStatus(env, napi_get_cb_info(env, info, &argc, args, nullptr, nullptr), "napi_get_cb_info")) {
        return false;
    }
    if (argc != 1) {
        napi_throw_type_error(env, nullptr, "openSessionTransport expects one host_hello object");
        return false;
    }

    if (!GetRequiredStringProperty(env, args[0], "type", &request->type) ||
        !GetRequiredStringProperty(env, args[0], "sessionId", &request->session_id) ||
        !GetRequiredInt32Property(env, args[0], "protocolMajor", &request->protocol_major) ||
        !GetRequiredInt32Property(env, args[0], "protocolMinor", &request->protocol_minor) ||
        !GetRequiredStringProperty(env, args[0], "hostVersion", &request->host_version) ||
        !GetRequiredStringArrayProperty(env, args[0], "requestedFeatures", &request->requested_features) ||
        !GetRequiredStringArrayProperty(env, args[0], "supportedVideoCodecs", &request->supported_video_codecs) ||
        !GetRequiredStringArrayProperty(env, args[0], "preferredVideoCodecs", &request->preferred_video_codecs) ||
        !GetRequiredInt32Property(env, args[0], "videoMaxWidth", &request->video_max_width) ||
        !GetRequiredInt32Property(env, args[0], "videoMaxHeight", &request->video_max_height) ||
        !GetRequiredInt32Property(env, args[0], "videoMaxFps", &request->video_max_fps) ||
        !GetOptionalInt32Property(env, args[0], "bitrateKbps", &request->video_bitrate_kbps, &request->has_video_bitrate_kbps) ||
        !GetOptionalInt32Property(
            env,
            args[0],
            "iframeIntervalMs",
            &request->video_iframe_interval_ms,
            &request->has_video_iframe_interval_ms)) {
        return false;
    }

    if (request->type != core::kSessionMessageTypeHostHello) {
        napi_throw_type_error(env, nullptr, "type must be host_hello");
        return false;
    }
    if (request->session_id.empty() || request->host_version.empty()) {
        napi_throw_type_error(env, nullptr, "sessionId and hostVersion must not be empty");
        return false;
    }
    if (request->requested_features.empty() || request->supported_video_codecs.empty() || request->preferred_video_codecs.empty()) {
        napi_throw_type_error(env, nullptr, "requestedFeatures, supportedVideoCodecs, and preferredVideoCodecs must not be empty");
        return false;
    }
    if (request->video_max_width <= 0 || request->video_max_height <= 0 || request->video_max_fps <= 0) {
        napi_throw_type_error(env, nullptr, "videoMaxWidth, videoMaxHeight, and videoMaxFps must be positive");
        return false;
    }
    for (const std::string &preferred_codec : request->preferred_video_codecs) {
        if (!ContainsCodec(request->supported_video_codecs, preferred_codec)) {
            napi_throw_type_error(env, nullptr, "preferredVideoCodecs must be a subset of supportedVideoCodecs");
            return false;
        }
    }
    return true;
}

bool ReadSessionTransportState(napi_env env, napi_value value, core::SessionTransportState *state)
{
    napi_value authorization = nullptr;
    napi_value channel_layout = nullptr;
    if (!GetRequiredStringProperty(env, value, "sessionId", &state->session_id) ||
        !GetRequiredInt32Property(env, value, "protocolMajor", &state->protocol_major) ||
        !GetRequiredInt32Property(env, value, "protocolMinor", &state->protocol_minor) ||
        !GetRequiredStringProperty(env, value, "phase", &state->phase) ||
        !GetRequiredObjectProperty(env, value, "authorization", &authorization) ||
        !GetRequiredStringArrayProperty(env, value, "requestedFeatures", &state->requested_features) ||
        !GetRequiredStringArrayProperty(env, value, "sharedVideoCodecs", &state->shared_video_codecs) ||
        !ReadVideoCodecDescriptorArray(env, value, "availableVideoCodecs", &state->available_video_codecs) ||
        !GetRequiredStringProperty(env, value, "selectedVideoCodec", &state->selected_video_codec) ||
        !GetRequiredBoolProperty(env, value, "controlEnabled", &state->control_enabled) ||
        !GetRequiredObjectProperty(env, value, "channelLayout", &channel_layout)) {
        return false;
    }

    return ReadAuthorizationStateMap(env, authorization, &state->authorization) &&
        ReadChannelLayout(env, channel_layout, &state->channel_layout);
}

bool ReadVideoConfig(napi_env env, napi_value value, core::VideoConfig *config)
{
    return GetRequiredInt32Property(env, value, "maxWidth", &config->max_width) &&
        GetRequiredInt32Property(env, value, "maxHeight", &config->max_height) &&
        GetRequiredInt32Property(env, value, "maxFps", &config->max_fps) &&
        GetOptionalInt32Property(env, value, "bitrateKbps", &config->bitrate_kbps, &config->has_bitrate_kbps) &&
        GetOptionalInt32Property(
            env, value, "iframeIntervalMs", &config->iframe_interval_ms, &config->has_iframe_interval_ms);
}

bool ReadVideoTransportState(napi_env env, napi_value value, core::VideoTransportState *state)
{
    napi_value binding = nullptr;
    napi_value display = nullptr;
    napi_value config = nullptr;
    if (!GetRequiredStringProperty(env, value, "sessionId", &state->session_id) ||
        !GetRequiredStringProperty(env, value, "selectedVideoCodec", &state->selected_video_codec) ||
        !GetRequiredObjectProperty(env, value, "binding", &binding) ||
        !GetRequiredObjectProperty(env, value, "display", &display) ||
        !GetRequiredObjectProperty(env, value, "config", &config) ||
        !GetRequiredStringProperty(env, value, "selectedPathModule", &state->selected_path_module) ||
        !GetRequiredStringArrayProperty(env, value, "pipelineStages", &state->pipeline_stages) ||
        !GetRequiredBoolProperty(env, value, "active", &state->active)) {
        return false;
    }

    return ReadChannelBinding(env, binding, &state->binding) &&
        ReadDisplayInfo(env, display, &state->display) &&
        ReadVideoConfig(env, config, &state->config);
}

bool ReadConfigureSessionTransportArgs(
    napi_env env, napi_callback_info info, core::SessionTransportState *state, core::SessionConfigRequest *request)
{
    size_t argc = 2;
    napi_value args[2] = {nullptr, nullptr};
    if (!CheckStatus(env, napi_get_cb_info(env, info, &argc, args, nullptr, nullptr), "napi_get_cb_info")) {
        return false;
    }
    if (argc != 2) {
        napi_throw_type_error(env, nullptr, "configureSessionTransport expects state and session_config objects");
        return false;
    }
    return ReadSessionTransportState(env, args[0], state) && ReadGenericSessionConfigObject(env, args[1], request);
}

bool ReadDispatchControlEventArgs(
    napi_env env, napi_callback_info info, core::SessionTransportState *state, core::ControlEventRequest *request)
{
    size_t argc = 2;
    napi_value args[2] = {nullptr, nullptr};
    if (!CheckStatus(env, napi_get_cb_info(env, info, &argc, args, nullptr, nullptr), "napi_get_cb_info")) {
        return false;
    }
    if (argc != 2) {
        napi_throw_type_error(env, nullptr, "dispatchControlEvent expects state and control_event objects");
        return false;
    }
    return ReadSessionTransportState(env, args[0], state) && ReadControlEventRequest(env, args[1], request);
}

bool ReadActivateVideoTransportArgs(napi_env env, napi_callback_info info, core::VideoTransportState *state)
{
    size_t argc = 1;
    napi_value args[1] = {nullptr};
    if (!CheckStatus(env, napi_get_cb_info(env, info, &argc, args, nullptr, nullptr), "napi_get_cb_info")) {
        return false;
    }
    if (argc != 1) {
        napi_throw_type_error(env, nullptr, "activateVideoTransport expects one video transport state object");
        return false;
    }
    return ReadVideoTransportState(env, args[0], state);
}

bool ReadPrepareVideoTransportPacketArgs(
    napi_env env, napi_callback_info info, core::VideoTransportState *state, core::VideoPacketRequest *request)
{
    size_t argc = 2;
    napi_value args[2] = {nullptr, nullptr};
    if (!CheckStatus(env, napi_get_cb_info(env, info, &argc, args, nullptr, nullptr), "napi_get_cb_info")) {
        return false;
    }
    if (argc != 2) {
        napi_throw_type_error(env, nullptr, "prepareVideoTransportPacket expects state and packet objects");
        return false;
    }

    if (!ReadVideoTransportState(env, args[0], state) ||
        !GetRequiredStringProperty(env, args[1], "sessionId", &request->session_id) ||
        !GetRequiredStringProperty(env, args[1], "codec", &request->codec) ||
        !GetRequiredInt64Property(env, args[1], "ptsUs", &request->pts_us) ||
        !GetRequiredBoolProperty(env, args[1], "isKeyframe", &request->is_keyframe) ||
        !GetRequiredInt32Property(env, args[1], "payloadLength", &request->payload_length)) {
        return false;
    }

    return true;
}

bool ReadStopSessionTransportArgs(
    napi_env env, napi_callback_info info, core::SessionTransportState *state, core::StopSessionRequest *request)
{
    size_t argc = 2;
    napi_value args[2] = {nullptr, nullptr};
    if (!CheckStatus(env, napi_get_cb_info(env, info, &argc, args, nullptr, nullptr), "napi_get_cb_info")) {
        return false;
    }
    if (argc != 2) {
        napi_throw_type_error(env, nullptr, "stopSessionTransport expects state and stop_session objects");
        return false;
    }

    if (!ReadSessionTransportState(env, args[0], state) ||
        !GetRequiredStringProperty(env, args[1], "sessionId", &request->session_id) ||
        !GetOptionalStringProperty(env, args[1], "reason", &request->reason, &request->has_reason)) {
        return false;
    }

    return true;
}

bool ReadStartSessionListenerRuntimeArgs(napi_env env, napi_callback_info info, int32_t *session_port)
{
    size_t argc = 1;
    napi_value args[1] = {nullptr};
    if (!CheckStatus(env, napi_get_cb_info(env, info, &argc, args, nullptr, nullptr), "napi_get_cb_info")) {
        return false;
    }
    if (argc == 0) {
        *session_port = core::kDefaultSessionPort;
        return true;
    }
    if (argc != 1) {
        napi_throw_type_error(env, nullptr, "startSessionListenerRuntime expects zero or one numeric port argument");
        return false;
    }

    napi_valuetype value_type = napi_undefined;
    if (!CheckStatus(env, napi_typeof(env, args[0], &value_type), "napi_typeof")) {
        return false;
    }
    if (value_type != napi_number) {
        napi_throw_type_error(env, nullptr, "startSessionListenerRuntime expects a numeric port when an argument is provided");
        return false;
    }

    if (!CheckStatus(env, napi_get_value_int32(env, args[0], session_port), "napi_get_value_int32")) {
        return false;
    }
    if (*session_port <= 0) {
        napi_throw_type_error(env, nullptr, "session port must be positive");
        return false;
    }
    return true;
}

}  // namespace

napi_value GetCompanionDescriptor(napi_env env, napi_callback_info info)
{
    (void)info;
    return CreateCompanionDescriptorValue(env, core::BuildCompanionDescriptor());
}

napi_value PreviewSessionNegotiation(napi_env env, napi_callback_info info)
{
    core::SessionNegotiationRequest request = {};
    if (!ReadSessionNegotiationRequest(env, info, &request)) {
        return nullptr;
    }

    return CreateSessionNegotiationPreviewValue(env, core::PreviewSessionNegotiation(request));
}

napi_value PreviewJpegSessionPath(napi_env env, napi_callback_info info)
{
    core::SessionConfigRequest request = {};
    if (!ReadJpegSessionConfigRequest(env, info, &request)) {
        return nullptr;
    }

    core::SessionNegotiationRequest negotiation_request = {
        request.session_id,
        {core::kVideoCodecJpeg},
        {core::kVideoCodecJpeg},
        request.video_max_width,
        request.video_max_height,
        request.video_max_fps,
        false,
        0,
        false,
        0,
        request.control_enabled
    };

    return CreateSessionNegotiationPreviewValue(env, core::PreviewSessionNegotiation(negotiation_request));
}

napi_value PreviewControlHandling(napi_env env, napi_callback_info info)
{
    core::ControlEventRequest request = {};
    if (!ReadPreviewControlHandlingRequest(env, info, &request)) {
        return nullptr;
    }

    return CreateControlHandlingPreviewValue(env, core::PreviewControlHandling(request));
}

napi_value OpenSessionTransport(napi_env env, napi_callback_info info)
{
    core::HostHelloRequest request = {};
    if (!ReadHostHelloRequest(env, info, &request)) {
        return nullptr;
    }

    return CreateSessionTransportOpenResultValue(env, core::OpenSessionTransport(request));
}

napi_value ConfigureSessionTransport(napi_env env, napi_callback_info info)
{
    core::SessionTransportState state = {};
    core::SessionConfigRequest request = {};
    if (!ReadConfigureSessionTransportArgs(env, info, &state, &request)) {
        return nullptr;
    }

    return CreateSessionTransportConfigureResultValue(env, core::ConfigureSessionTransport(state, request));
}

napi_value StartSessionListenerRuntime(napi_env env, napi_callback_info info)
{
    int32_t session_port = core::kDefaultSessionPort;
    if (!ReadStartSessionListenerRuntimeArgs(env, info, &session_port)) {
        return nullptr;
    }

    return CreateSessionListenerRuntimeStatusValue(env, core::StartSessionListenerRuntime(session_port));
}

napi_value StopSessionListenerRuntime(napi_env env, napi_callback_info info)
{
    (void)info;
    return CreateSessionListenerRuntimeStatusValue(env, core::StopSessionListenerRuntime());
}

napi_value GetSessionListenerRuntimeStatus(napi_env env, napi_callback_info info)
{
    (void)info;
    return CreateSessionListenerRuntimeStatusValue(env, core::GetSessionListenerRuntimeStatus());
}

napi_value DispatchControlEvent(napi_env env, napi_callback_info info)
{
    core::SessionTransportState state = {};
    core::ControlEventRequest request = {};
    if (!ReadDispatchControlEventArgs(env, info, &state, &request)) {
        return nullptr;
    }

    return CreateControlHandlingPreviewValue(env, core::DispatchControlEvent(state, request));
}

napi_value ActivateVideoTransport(napi_env env, napi_callback_info info)
{
    core::VideoTransportState state = {};
    if (!ReadActivateVideoTransportArgs(env, info, &state)) {
        return nullptr;
    }

    return CreateVideoTransportActivationResultValue(env, core::ActivateVideoTransport(state));
}

napi_value PrepareVideoTransportPacket(napi_env env, napi_callback_info info)
{
    core::VideoTransportState state = {};
    core::VideoPacketRequest request = {};
    if (!ReadPrepareVideoTransportPacketArgs(env, info, &state, &request)) {
        return nullptr;
    }

    return CreateVideoPacketTransportResultValue(env, core::PrepareVideoTransportPacket(state, request));
}

napi_value StopSessionTransport(napi_env env, napi_callback_info info)
{
    core::SessionTransportState state = {};
    core::StopSessionRequest request = {};
    if (!ReadStopSessionTransportArgs(env, info, &state, &request)) {
        return nullptr;
    }

    return CreateSessionTransportStopResultValue(env, core::StopSessionTransport(state, request));
}

bool RegisterCompanionExports(napi_env env, napi_value exports)
{
    napi_property_descriptor descriptors[] = {
        {"getCompanionDescriptor", nullptr, GetCompanionDescriptor, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"previewSessionNegotiation", nullptr, PreviewSessionNegotiation, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"previewJpegSessionPath", nullptr, PreviewJpegSessionPath, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"previewControlHandling", nullptr, PreviewControlHandling, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"openSessionTransport", nullptr, OpenSessionTransport, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"configureSessionTransport", nullptr, ConfigureSessionTransport, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"startSessionListenerRuntime", nullptr, StartSessionListenerRuntime, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"stopSessionListenerRuntime", nullptr, StopSessionListenerRuntime, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"getSessionListenerRuntimeStatus", nullptr, GetSessionListenerRuntimeStatus, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"dispatchControlEvent", nullptr, DispatchControlEvent, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"activateVideoTransport", nullptr, ActivateVideoTransport, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"prepareVideoTransportPacket", nullptr, PrepareVideoTransportPacket, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"stopSessionTransport", nullptr, StopSessionTransport, nullptr, nullptr, nullptr, napi_default, nullptr}
    };

    return CheckStatus(
        env,
        napi_define_properties(env, exports, sizeof(descriptors) / sizeof(descriptors[0]), descriptors),
        "napi_define_properties");
}

}  // namespace bridge
}  // namespace hscrcpy
