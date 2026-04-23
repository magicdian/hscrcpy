#ifndef HSCRCPY_SERVER_CORE_COMPANION_CORE_H
#define HSCRCPY_SERVER_CORE_COMPANION_CORE_H

#include "core/companion_contracts.h"

namespace hscrcpy {
namespace core {

CompanionDescriptor BuildCompanionDescriptor();
bool HasSharedVideoCodec(const SessionNegotiationRequest &request);
SessionNegotiationPreview PreviewSessionNegotiation(const SessionNegotiationRequest &request);
ControlHandlingPreview PreviewControlHandling(const ControlEventRequest &event);
SessionTransportOpenResult OpenSessionTransport(const HostHelloRequest &request);
SessionTransportConfigureResult ConfigureSessionTransport(
    const SessionTransportState &state, const SessionConfigRequest &request);
SessionTransportStopResult StopSessionTransport(const SessionTransportState &state, const StopSessionRequest &request);
SessionListenerRuntimeStatus StartSessionListenerRuntime(int32_t session_port);
SessionListenerRuntimeStatus StopSessionListenerRuntime();
SessionListenerRuntimeStatus GetSessionListenerRuntimeStatus();
ControlHandlingPreview DispatchControlEvent(const SessionTransportState &state, const ControlEventRequest &event);
VideoTransportActivationResult ActivateVideoTransport(const VideoTransportState &state);
VideoPacketTransportResult PrepareVideoTransportPacket(
    const VideoTransportState &state, const VideoPacketRequest &request);

}  // namespace core
}  // namespace hscrcpy

#endif
