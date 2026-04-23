#ifndef HSCRCPY_SERVER_BRIDGE_COMPANION_NAPI_H
#define HSCRCPY_SERVER_BRIDGE_COMPANION_NAPI_H

#include "napi/native_api.h"

namespace hscrcpy {
namespace bridge {

napi_value GetCompanionDescriptor(napi_env env, napi_callback_info info);
napi_value PreviewSessionNegotiation(napi_env env, napi_callback_info info);
napi_value PreviewJpegSessionPath(napi_env env, napi_callback_info info);
napi_value PreviewControlHandling(napi_env env, napi_callback_info info);
napi_value OpenSessionTransport(napi_env env, napi_callback_info info);
napi_value ConfigureSessionTransport(napi_env env, napi_callback_info info);
napi_value StartSessionListenerRuntime(napi_env env, napi_callback_info info);
napi_value StopSessionListenerRuntime(napi_env env, napi_callback_info info);
napi_value GetSessionListenerRuntimeStatus(napi_env env, napi_callback_info info);
napi_value DispatchControlEvent(napi_env env, napi_callback_info info);
napi_value ActivateVideoTransport(napi_env env, napi_callback_info info);
napi_value PrepareVideoTransportPacket(napi_env env, napi_callback_info info);
napi_value StopSessionTransport(napi_env env, napi_callback_info info);
bool RegisterCompanionExports(napi_env env, napi_value exports);

}  // namespace bridge
}  // namespace hscrcpy

#endif
