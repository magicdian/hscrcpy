#ifndef HSCRCPY_SERVER_TRANSPORT_VIDEO_CHANNEL_H
#define HSCRCPY_SERVER_TRANSPORT_VIDEO_CHANNEL_H

#include "core/companion_contracts.h"

namespace hscrcpy {
namespace transport {

core::VideoTransportState BuildVideoTransportState(
    const core::SessionTransportState &session_state,
    const core::SessionReadyPreview &session_ready,
    const core::SessionConfigRequest &request,
    const std::vector<std::string> &pipeline_stages);
core::VideoTransportActivationResult ActivateVideoChannel(const core::VideoTransportState &state);
bool StartVideoChannelRuntime(const core::VideoTransportState &state, std::string *error);
void StopVideoChannelRuntime();
core::VideoPacketTransportResult PrepareVideoPacket(
    const core::VideoTransportState &state, const core::VideoPacketRequest &request);

}  // namespace transport
}  // namespace hscrcpy

#endif
