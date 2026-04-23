#ifndef HSCRCPY_SERVER_TRANSPORT_SESSION_CHANNEL_H
#define HSCRCPY_SERVER_TRANSPORT_SESSION_CHANNEL_H

#include "core/companion_contracts.h"

namespace hscrcpy {
namespace transport {

core::HostHelloRequest BuildHostHelloRequest(const core::SessionNegotiationRequest &request);
core::SessionConfigPreview BuildSessionConfigPreview(const core::SessionConfigRequest &request);
core::SessionTransportOpenResult OpenSessionChannel(const core::HostHelloRequest &request);
core::SessionTransportConfigureResult ConfigureSessionChannel(
    const core::SessionTransportState &state, const core::SessionConfigRequest &request);
core::SessionTransportStopResult StopSessionChannel(
    const core::SessionTransportState &state, const core::StopSessionRequest &request);
bool TryParseSessionMessageType(const std::string &message_json, std::string *message_type, std::string *error);
bool TryParseHostHelloMessage(
    const std::string &message_json, core::HostHelloRequest *request, std::string *error);
bool TryParseSessionConfigMessage(
    const std::string &message_json, core::SessionConfigRequest *request, std::string *error);
bool TryParseStopSessionMessage(
    const std::string &message_json, core::StopSessionRequest *request, std::string *error);
bool TryParseControlEventMessage(
    const std::string &message_json, core::ControlEventRequest *request, std::string *error);
std::string SerializeSessionErrorMessage(const core::SessionErrorMessage &session_error);

}  // namespace transport
}  // namespace hscrcpy

#endif
