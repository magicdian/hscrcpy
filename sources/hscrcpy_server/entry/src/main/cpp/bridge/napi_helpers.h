#ifndef HSCRCPY_SERVER_BRIDGE_NAPI_HELPERS_H
#define HSCRCPY_SERVER_BRIDGE_NAPI_HELPERS_H

#include <string>
#include <vector>

#include "napi/native_api.h"

namespace hscrcpy {
namespace bridge {

bool CheckStatus(napi_env env, napi_status status, const char *operation);
bool ThrowTypeError(napi_env env, const char *field_name, const char *expected_type);
napi_value CreateString(napi_env env, const std::string &value);
napi_value CreateBoolean(napi_env env, bool value);
napi_value CreateInt32(napi_env env, int32_t value);
napi_value CreateDouble(napi_env env, double value);
napi_value CreateStringArray(napi_env env, const std::vector<std::string> &items);
bool GetRequiredStringArrayProperty(
    napi_env env, napi_value object, const char *field_name, std::vector<std::string> *result);
bool GetRequiredStringProperty(napi_env env, napi_value object, const char *field_name, std::string *result);
bool GetRequiredBoolProperty(napi_env env, napi_value object, const char *field_name, bool *result);
bool GetRequiredInt32Property(napi_env env, napi_value object, const char *field_name, int32_t *result);
bool GetOptionalStringProperty(napi_env env, napi_value object, const char *field_name, std::string *result, bool *has_value);
bool GetOptionalInt32Property(napi_env env, napi_value object, const char *field_name, int32_t *result, bool *has_value);
bool GetOptionalDoubleProperty(napi_env env, napi_value object, const char *field_name, double *result, bool *has_value);
bool GetOptionalObjectProperty(napi_env env, napi_value object, const char *field_name, napi_value *result, bool *has_value);
bool SetNamedProperty(napi_env env, napi_value object, const char *field_name, napi_value value);

}  // namespace bridge
}  // namespace hscrcpy

#endif
