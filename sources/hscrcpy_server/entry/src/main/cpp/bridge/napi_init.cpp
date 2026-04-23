#include "bridge/companion_napi.h"

#include "napi/native_api.h"

EXTERN_C_START
static napi_value Init(napi_env env, napi_value exports)
{
    if (!hscrcpy::bridge::RegisterCompanionExports(env, exports)) {
        return nullptr;
    }
    return exports;
}
EXTERN_C_END

static napi_module g_entry_module = {
    .nm_version = 1,
    .nm_flags = 0,
    .nm_filename = nullptr,
    .nm_register_func = Init,
    .nm_modname = "entry",
    .nm_priv = nullptr,
    .reserved = {0},
};

extern "C" __attribute__((constructor)) void RegisterEntryModule(void)
{
    napi_module_register(&g_entry_module);
}
