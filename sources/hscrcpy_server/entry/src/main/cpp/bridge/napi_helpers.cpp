#include "bridge/napi_helpers.h"

#include <cstring>
#include <vector>

namespace hscrcpy {
namespace bridge {

namespace {

const char *GetLastErrorMessage(napi_env env)
{
    const napi_extended_error_info *error_info = nullptr;
    napi_get_last_error_info(env, &error_info);
    if (error_info == nullptr || error_info->error_message == nullptr) {
        return "unknown napi error";
    }
    return error_info->error_message;
}

}  // namespace

bool CheckStatus(napi_env env, napi_status status, const char *operation)
{
    if (status == napi_ok) {
        return true;
    }

    std::string message(operation);
    message.append(" failed: ");
    message.append(GetLastErrorMessage(env));
    napi_throw_error(env, nullptr, message.c_str());
    return false;
}

bool ThrowTypeError(napi_env env, const char *field_name, const char *expected_type)
{
    std::string message("Expected ");
    message.append(field_name);
    message.append(" to be ");
    message.append(expected_type);
    napi_throw_type_error(env, nullptr, message.c_str());
    return false;
}

napi_value CreateString(napi_env env, const std::string &value)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_string_utf8(env, value.c_str(), value.size(), &result), "napi_create_string_utf8")) {
        return nullptr;
    }
    return result;
}

napi_value CreateBoolean(napi_env env, bool value)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_get_boolean(env, value, &result), "napi_get_boolean")) {
        return nullptr;
    }
    return result;
}

napi_value CreateInt32(napi_env env, int32_t value)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_int32(env, value, &result), "napi_create_int32")) {
        return nullptr;
    }
    return result;
}

napi_value CreateDouble(napi_env env, double value)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_double(env, value, &result), "napi_create_double")) {
        return nullptr;
    }
    return result;
}

napi_value CreateStringArray(napi_env env, const std::vector<std::string> &items)
{
    napi_value result = nullptr;
    if (!CheckStatus(env, napi_create_array_with_length(env, items.size(), &result), "napi_create_array_with_length")) {
        return nullptr;
    }

    for (size_t index = 0; index < items.size(); ++index) {
        napi_value item = CreateString(env, items[index]);
        if (item == nullptr) {
            return nullptr;
        }
        if (!CheckStatus(env, napi_set_element(env, result, index, item), "napi_set_element")) {
            return nullptr;
        }
    }

    return result;
}

bool GetRequiredStringArrayProperty(
    napi_env env, napi_value object, const char *field_name, std::vector<std::string> *result)
{
    bool has_property = false;
    if (!CheckStatus(env, napi_has_named_property(env, object, field_name, &has_property), "napi_has_named_property")) {
        return false;
    }
    if (!has_property) {
        return ThrowTypeError(env, field_name, "an array of strings");
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
        return ThrowTypeError(env, field_name, "an array of strings");
    }

    uint32_t length = 0;
    if (!CheckStatus(env, napi_get_array_length(env, field, &length), "napi_get_array_length")) {
        return false;
    }

    result->clear();
    result->reserve(length);
    for (uint32_t index = 0; index < length; ++index) {
        napi_value item = nullptr;
        if (!CheckStatus(env, napi_get_element(env, field, index, &item), "napi_get_element")) {
            return false;
        }

        napi_valuetype value_type = napi_undefined;
        if (!CheckStatus(env, napi_typeof(env, item, &value_type), "napi_typeof")) {
            return false;
        }
        if (value_type != napi_string) {
            return ThrowTypeError(env, field_name, "an array of strings");
        }

        size_t string_length = 0;
        if (!CheckStatus(env, napi_get_value_string_utf8(env, item, nullptr, 0, &string_length), "napi_get_value_string_utf8")) {
            return false;
        }

        std::vector<char> buffer(string_length + 1, '\0');
        if (!CheckStatus(
                env,
                napi_get_value_string_utf8(env, item, buffer.data(), buffer.size(), &string_length),
                "napi_get_value_string_utf8")) {
            return false;
        }

        result->emplace_back(buffer.data(), string_length);
    }

    return true;
}

bool GetRequiredStringProperty(napi_env env, napi_value object, const char *field_name, std::string *result)
{
    bool has_property = false;
    if (!CheckStatus(env, napi_has_named_property(env, object, field_name, &has_property), "napi_has_named_property")) {
        return false;
    }
    if (!has_property) {
        return ThrowTypeError(env, field_name, "a string");
    }

    napi_value field = nullptr;
    if (!CheckStatus(env, napi_get_named_property(env, object, field_name, &field), "napi_get_named_property")) {
        return false;
    }

    napi_valuetype value_type = napi_undefined;
    if (!CheckStatus(env, napi_typeof(env, field, &value_type), "napi_typeof")) {
        return false;
    }
    if (value_type != napi_string) {
        return ThrowTypeError(env, field_name, "a string");
    }

    size_t length = 0;
    if (!CheckStatus(env, napi_get_value_string_utf8(env, field, nullptr, 0, &length), "napi_get_value_string_utf8")) {
        return false;
    }

    std::vector<char> buffer(length + 1, '\0');
    if (!CheckStatus(
            env,
            napi_get_value_string_utf8(env, field, buffer.data(), buffer.size(), &length),
            "napi_get_value_string_utf8")) {
        return false;
    }

    result->assign(buffer.data(), length);
    return true;
}

bool GetRequiredBoolProperty(napi_env env, napi_value object, const char *field_name, bool *result)
{
    bool has_property = false;
    if (!CheckStatus(env, napi_has_named_property(env, object, field_name, &has_property), "napi_has_named_property")) {
        return false;
    }
    if (!has_property) {
        return ThrowTypeError(env, field_name, "a boolean");
    }

    napi_value field = nullptr;
    if (!CheckStatus(env, napi_get_named_property(env, object, field_name, &field), "napi_get_named_property")) {
        return false;
    }

    napi_valuetype value_type = napi_undefined;
    if (!CheckStatus(env, napi_typeof(env, field, &value_type), "napi_typeof")) {
        return false;
    }
    if (value_type != napi_boolean) {
        return ThrowTypeError(env, field_name, "a boolean");
    }

    return CheckStatus(env, napi_get_value_bool(env, field, result), "napi_get_value_bool");
}

bool GetRequiredInt32Property(napi_env env, napi_value object, const char *field_name, int32_t *result)
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

    return CheckStatus(env, napi_get_value_int32(env, field, result), "napi_get_value_int32");
}

bool GetOptionalStringProperty(
    napi_env env, napi_value object, const char *field_name, std::string *result, bool *has_value)
{
    *has_value = false;

    bool has_property = false;
    if (!CheckStatus(env, napi_has_named_property(env, object, field_name, &has_property), "napi_has_named_property")) {
        return false;
    }
    if (!has_property) {
        return true;
    }

    napi_value field = nullptr;
    if (!CheckStatus(env, napi_get_named_property(env, object, field_name, &field), "napi_get_named_property")) {
        return false;
    }

    napi_valuetype value_type = napi_undefined;
    if (!CheckStatus(env, napi_typeof(env, field, &value_type), "napi_typeof")) {
        return false;
    }
    if (value_type != napi_string) {
        return ThrowTypeError(env, field_name, "a string");
    }

    size_t length = 0;
    if (!CheckStatus(env, napi_get_value_string_utf8(env, field, nullptr, 0, &length), "napi_get_value_string_utf8")) {
        return false;
    }

    std::vector<char> buffer(length + 1, '\0');
    if (!CheckStatus(
            env,
            napi_get_value_string_utf8(env, field, buffer.data(), buffer.size(), &length),
            "napi_get_value_string_utf8")) {
        return false;
    }

    result->assign(buffer.data(), length);
    *has_value = true;
    return true;
}

bool GetOptionalInt32Property(napi_env env, napi_value object, const char *field_name, int32_t *result, bool *has_value)
{
    *has_value = false;

    bool has_property = false;
    if (!CheckStatus(env, napi_has_named_property(env, object, field_name, &has_property), "napi_has_named_property")) {
        return false;
    }
    if (!has_property) {
        return true;
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

    if (!CheckStatus(env, napi_get_value_int32(env, field, result), "napi_get_value_int32")) {
        return false;
    }

    *has_value = true;
    return true;
}

bool GetOptionalDoubleProperty(napi_env env, napi_value object, const char *field_name, double *result, bool *has_value)
{
    *has_value = false;

    bool has_property = false;
    if (!CheckStatus(env, napi_has_named_property(env, object, field_name, &has_property), "napi_has_named_property")) {
        return false;
    }
    if (!has_property) {
        return true;
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

    if (!CheckStatus(env, napi_get_value_double(env, field, result), "napi_get_value_double")) {
        return false;
    }

    *has_value = true;
    return true;
}

bool GetOptionalObjectProperty(napi_env env, napi_value object, const char *field_name, napi_value *result, bool *has_value)
{
    *has_value = false;

    bool has_property = false;
    if (!CheckStatus(env, napi_has_named_property(env, object, field_name, &has_property), "napi_has_named_property")) {
        return false;
    }
    if (!has_property) {
        return true;
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

    *has_value = true;
    return true;
}

bool SetNamedProperty(napi_env env, napi_value object, const char *field_name, napi_value value)
{
    if (value == nullptr) {
        return false;
    }
    return CheckStatus(env, napi_set_named_property(env, object, field_name, value), "napi_set_named_property");
}

}  // namespace bridge
}  // namespace hscrcpy
