#ifndef HSCRCPY_SERVER_TRANSPORT_SESSION_LISTENER_RUNTIME_H
#define HSCRCPY_SERVER_TRANSPORT_SESSION_LISTENER_RUNTIME_H

#include "core/companion_contracts.h"

namespace hscrcpy {
namespace transport {

core::SessionListenerRuntimeStatus StartSessionListenerRuntime(int32_t session_port);
core::SessionListenerRuntimeStatus StopSessionListenerRuntime();
core::SessionListenerRuntimeStatus GetSessionListenerRuntimeStatus();

}  // namespace transport
}  // namespace hscrcpy

#endif
