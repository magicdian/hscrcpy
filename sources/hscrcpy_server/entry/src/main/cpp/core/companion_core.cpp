#include "core/companion_core.h"

#include "control/control_message_handler.h"
#include "control/input_injector.h"
#include "transport/session_channel.h"
#include "transport/session_listener_runtime.h"
#include "transport/video_channel.h"
#include "video/h264_video_path.h"
#include "video/jpeg_video_path.h"

namespace hscrcpy {
namespace core {

namespace {

VideoUnitPreview BuildVideoUnitPreview(const std::string &codec)
{
    if (codec == kVideoCodecH264) {
        return video::h264::BuildVideoUnitPreview();
    }
    return video::jpeg::BuildVideoUnitPreview();
}

std::vector<std::string> BuildPreviewNotes(
    const std::vector<std::string> &open_notes,
    const std::vector<std::string> &configure_notes,
    const SessionNegotiationRequest &request,
    const VideoNegotiationPreview &negotiation,
    const SessionConfigRequest &session_config)
{
    std::vector<std::string> notes = open_notes;
    notes.insert(notes.end(), configure_notes.begin(), configure_notes.end());
    notes.push_back("control_event validation and injection planning stay in native code; ArkTS only renders the preview state.");

    if (negotiation.selected_video_codec == kVideoCodecH264) {
        notes.push_back("H.264 remains the preferred runtime mainline whenever the shared codec set allows it.");
        if (negotiation.fallback_available) {
            notes.push_back("JPEG remains ready as an explicit device-side fallback path without changing channel roles.");
        }
    } else {
        notes.push_back("JPEG is used only because H.264 is absent from the shared codec set for this session.");
    }

    if (request.has_video_bitrate_kbps && !session_config.has_video_bitrate_kbps) {
        notes.push_back("video.bitrate_kbps is ignored when the negotiated codec is JPEG.");
    }
    if (request.has_video_iframe_interval_ms && !session_config.has_video_iframe_interval_ms) {
        notes.push_back("video.iframe_interval_ms is ignored when the negotiated codec is JPEG.");
    }

    return notes;
}

ControlHandlingPreview BuildRejectedControlPreview(
    const SessionTransportState &state, const ControlEventRequest &event, const std::string &note)
{
    return {
        kControlEventType,
        event.session_id.empty() ? state.session_id : event.session_id,
        event.event_type,
        event.sequence,
        "session/control_event -> control/control_message_handler",
        false,
        "rejected",
        "not_planned",
        "unsupported",
        {
            "validate runtime session state before control dispatch"
        },
        {note}
    };
}

}  // namespace

CompanionDescriptor BuildCompanionDescriptor()
{
    CompanionDescriptor descriptor;
    descriptor.shell_api_version = kShellApiVersion;
    descriptor.protocol_major = kProtocolMajor;
    descriptor.protocol_minor = kProtocolMinor;
    descriptor.companion_name = kCompanionName;
    descriptor.native_core_version = kNativeCoreVersion;
    descriptor.shell_responsibilities = {
        "UIAbility lifecycle and window loading",
        "permission and configuration surfaces",
        "typed bridge calls into the native core"
    };
    descriptor.native_responsibilities = {
        "session ownership, startup validation, and host-order video negotiation preview",
        "session listener accept-loop ownership plus session/video transport runtime setup behind explicit native seams",
        "H.264 mainline capability advertisement with explicit JPEG fallback shaping",
        "control_event validation plus input injection scaffolding behind a thin N-API bridge"
    };
    descriptor.planned_modules = {
        "bridge",
        "capture/h264_screen_capture_source",
        "codec/h264_encoder_capability",
        "control/control_message_handler",
        "control/input_injector",
        "video/h264_video_path",
        "video/jpeg_video_path",
        "video/video_path_utils",
        "transport/session_channel",
        "transport/video_channel"
    };
    descriptor.supported_session_channels = {
        kChannelNameSession,
        kChannelNameVideo
    };
    descriptor.supported_video_codecs = {
        kVideoCodecH264,
        kVideoCodecJpeg
    };
    return descriptor;
}

bool HasSharedVideoCodec(const SessionNegotiationRequest &request)
{
    return transport::OpenSessionChannel(transport::BuildHostHelloRequest(request)).accepted;
}

SessionTransportOpenResult OpenSessionTransport(const HostHelloRequest &request)
{
    return transport::OpenSessionChannel(request);
}

SessionTransportConfigureResult ConfigureSessionTransport(
    const SessionTransportState &state, const SessionConfigRequest &request)
{
    return transport::ConfigureSessionChannel(state, request);
}

SessionTransportStopResult StopSessionTransport(const SessionTransportState &state, const StopSessionRequest &request)
{
    return transport::StopSessionChannel(state, request);
}

SessionListenerRuntimeStatus StartSessionListenerRuntime(int32_t session_port)
{
    return transport::StartSessionListenerRuntime(session_port);
}

SessionListenerRuntimeStatus StopSessionListenerRuntime()
{
    return transport::StopSessionListenerRuntime();
}

SessionListenerRuntimeStatus GetSessionListenerRuntimeStatus()
{
    return transport::GetSessionListenerRuntimeStatus();
}

SessionNegotiationPreview PreviewSessionNegotiation(const SessionNegotiationRequest &request)
{
    const HostHelloRequest host_hello = transport::BuildHostHelloRequest(request);
    const SessionTransportOpenResult open_result = OpenSessionTransport(host_hello);

    SessionNegotiationPreview preview = {};
    preview.phase = kSessionPhaseVideoNegotiation;
    preview.session_id = request.session_id;
    preview.control_baseline = control::BuildControlBaselinePreview();

    if (!open_result.accepted) {
        preview.notes = open_result.notes;
        if (open_result.has_session_error) {
            preview.notes.push_back(open_result.session_error.code + ": " + open_result.session_error.message);
        }
        return preview;
    }

    const SessionTransportConfigureResult configure_result =
        ConfigureSessionTransport(open_result.state, open_result.suggested_session_config);

    preview.device_hello = open_result.device_hello;
    preview.negotiation = open_result.negotiation;
    preview.session_config = transport::BuildSessionConfigPreview(open_result.suggested_session_config);
    preview.video_unit = BuildVideoUnitPreview(open_result.suggested_session_config.selected_video_codec);
    preview.pipeline_stages = configure_result.pipeline_stages;
    preview.notes = BuildPreviewNotes(
        open_result.notes,
        configure_result.notes,
        request,
        open_result.negotiation,
        open_result.suggested_session_config);

    if (configure_result.has_session_ready) {
        preview.session_ready = configure_result.session_ready;
    } else if (configure_result.has_session_error) {
        preview.notes.push_back(configure_result.session_error.code + ": " + configure_result.session_error.message);
    }

    return preview;
}

ControlHandlingPreview PreviewControlHandling(const ControlEventRequest &event)
{
    return control::PreviewControlHandling(event);
}

ControlHandlingPreview DispatchControlEvent(const SessionTransportState &state, const ControlEventRequest &event)
{
    if (state.phase != kSessionPhaseReady) {
        return BuildRejectedControlPreview(state, event, "control_event is only accepted after session_ready.");
    }
    if (event.session_id != state.session_id) {
        return BuildRejectedControlPreview(state, event, "control_event.session_id must match the active runtime session.");
    }
    if (!state.control_enabled) {
        return BuildRejectedControlPreview(state, event, "control_event delivery is disabled for the active session_config.");
    }

    return control::PreviewControlHandling(event);
}

VideoTransportActivationResult ActivateVideoTransport(const VideoTransportState &state)
{
    return transport::ActivateVideoChannel(state);
}

VideoPacketTransportResult PrepareVideoTransportPacket(
    const VideoTransportState &state, const VideoPacketRequest &request)
{
    return transport::PrepareVideoPacket(state, request);
}

}  // namespace core
}  // namespace hscrcpy
