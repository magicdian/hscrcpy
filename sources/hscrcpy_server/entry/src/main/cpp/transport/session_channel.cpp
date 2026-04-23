#include "transport/session_channel.h"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdlib>
#include <limits>
#include <map>
#include <sstream>

#include "transport/video_channel.h"
#include "video/h264_video_path.h"
#include "video/jpeg_video_path.h"
#include "video/video_path_utils.h"

namespace hscrcpy {
namespace transport {

namespace {

bool ContainsValue(const std::vector<std::string> &values, const std::string &value)
{
    return std::find(values.begin(), values.end(), value) != values.end();
}

struct JsonValue {
    enum class Type {
        kNull,
        kBool,
        kNumber,
        kString,
        kObject,
        kArray
    };

    Type type = Type::kNull;
    bool bool_value = false;
    double number_value = 0.0;
    std::string string_value;
    std::map<std::string, JsonValue> object_values;
    std::vector<JsonValue> array_values;
};

class JsonParser {
public:
    explicit JsonParser(const std::string &input) : input_(input) {}

    bool Parse(JsonValue *value, std::string *error)
    {
        if (value == nullptr) {
            if (error != nullptr) {
                *error = "json parser requires a non-null output value";
            }
            return false;
        }

        SkipWhitespace();
        if (!ParseValue(value, error)) {
            return false;
        }
        SkipWhitespace();
        if (index_ != input_.size()) {
            if (error != nullptr) {
                *error = "unexpected trailing data after JSON payload";
            }
            return false;
        }
        return true;
    }

private:
    void SkipWhitespace()
    {
        while (index_ < input_.size() && std::isspace(static_cast<unsigned char>(input_[index_])) != 0) {
            ++index_;
        }
    }

    bool ParseValue(JsonValue *value, std::string *error)
    {
        if (index_ >= input_.size()) {
            if (error != nullptr) {
                *error = "unexpected end of JSON input";
            }
            return false;
        }

        const char current = input_[index_];
        if (current == '{') {
            return ParseObject(value, error);
        }
        if (current == '[') {
            return ParseArray(value, error);
        }
        if (current == '"') {
            value->type = JsonValue::Type::kString;
            return ParseString(&value->string_value, error);
        }
        if (current == 't' || current == 'f') {
            value->type = JsonValue::Type::kBool;
            return ParseBool(&value->bool_value, error);
        }
        if (current == 'n') {
            return ParseNull(value, error);
        }
        if (current == '-' || std::isdigit(static_cast<unsigned char>(current)) != 0) {
            value->type = JsonValue::Type::kNumber;
            return ParseNumber(&value->number_value, error);
        }

        if (error != nullptr) {
            *error = "unsupported JSON token";
        }
        return false;
    }

    bool ParseObject(JsonValue *value, std::string *error)
    {
        value->type = JsonValue::Type::kObject;
        value->object_values.clear();

        ++index_;
        SkipWhitespace();
        if (index_ < input_.size() && input_[index_] == '}') {
            ++index_;
            return true;
        }

        while (index_ < input_.size()) {
            std::string key;
            if (!ParseString(&key, error)) {
                return false;
            }

            SkipWhitespace();
            if (index_ >= input_.size() || input_[index_] != ':') {
                if (error != nullptr) {
                    *error = "expected ':' after object key";
                }
                return false;
            }
            ++index_;
            SkipWhitespace();

            JsonValue child;
            if (!ParseValue(&child, error)) {
                return false;
            }
            value->object_values[key] = child;

            SkipWhitespace();
            if (index_ >= input_.size()) {
                if (error != nullptr) {
                    *error = "unexpected end of object";
                }
                return false;
            }
            if (input_[index_] == '}') {
                ++index_;
                return true;
            }
            if (input_[index_] != ',') {
                if (error != nullptr) {
                    *error = "expected ',' between object fields";
                }
                return false;
            }
            ++index_;
            SkipWhitespace();
        }

        if (error != nullptr) {
            *error = "unterminated object";
        }
        return false;
    }

    bool ParseArray(JsonValue *value, std::string *error)
    {
        value->type = JsonValue::Type::kArray;
        value->array_values.clear();

        ++index_;
        SkipWhitespace();
        if (index_ < input_.size() && input_[index_] == ']') {
            ++index_;
            return true;
        }

        while (index_ < input_.size()) {
            JsonValue item;
            if (!ParseValue(&item, error)) {
                return false;
            }
            value->array_values.push_back(item);

            SkipWhitespace();
            if (index_ >= input_.size()) {
                if (error != nullptr) {
                    *error = "unexpected end of array";
                }
                return false;
            }
            if (input_[index_] == ']') {
                ++index_;
                return true;
            }
            if (input_[index_] != ',') {
                if (error != nullptr) {
                    *error = "expected ',' between array elements";
                }
                return false;
            }
            ++index_;
            SkipWhitespace();
        }

        if (error != nullptr) {
            *error = "unterminated array";
        }
        return false;
    }

    bool ParseString(std::string *value, std::string *error)
    {
        if (index_ >= input_.size() || input_[index_] != '"') {
            if (error != nullptr) {
                *error = "expected string";
            }
            return false;
        }

        ++index_;
        std::string result;
        while (index_ < input_.size()) {
            const char current = input_[index_++];
            if (current == '"') {
                *value = result;
                return true;
            }
            if (current != '\\') {
                result.push_back(current);
                continue;
            }
            if (index_ >= input_.size()) {
                if (error != nullptr) {
                    *error = "unterminated JSON escape sequence";
                }
                return false;
            }

            const char escaped = input_[index_++];
            switch (escaped) {
                case '"':
                case '\\':
                case '/':
                    result.push_back(escaped);
                    break;
                case 'b':
                    result.push_back('\b');
                    break;
                case 'f':
                    result.push_back('\f');
                    break;
                case 'n':
                    result.push_back('\n');
                    break;
                case 'r':
                    result.push_back('\r');
                    break;
                case 't':
                    result.push_back('\t');
                    break;
                default:
                    if (error != nullptr) {
                        *error = "unsupported JSON escape sequence";
                    }
                    return false;
            }
        }

        if (error != nullptr) {
            *error = "unterminated JSON string";
        }
        return false;
    }

    bool ParseBool(bool *value, std::string *error)
    {
        if (input_.compare(index_, 4, "true") == 0) {
            *value = true;
            index_ += 4;
            return true;
        }
        if (input_.compare(index_, 5, "false") == 0) {
            *value = false;
            index_ += 5;
            return true;
        }
        if (error != nullptr) {
            *error = "expected JSON boolean";
        }
        return false;
    }

    bool ParseNull(JsonValue *value, std::string *error)
    {
        if (input_.compare(index_, 4, "null") != 0) {
            if (error != nullptr) {
                *error = "expected JSON null";
            }
            return false;
        }
        value->type = JsonValue::Type::kNull;
        index_ += 4;
        return true;
    }

    bool ParseNumber(double *value, std::string *error)
    {
        const size_t start = index_;
        if (input_[index_] == '-') {
            ++index_;
        }
        while (index_ < input_.size() && std::isdigit(static_cast<unsigned char>(input_[index_])) != 0) {
            ++index_;
        }
        if (index_ < input_.size() && input_[index_] == '.') {
            ++index_;
            while (index_ < input_.size() && std::isdigit(static_cast<unsigned char>(input_[index_])) != 0) {
                ++index_;
            }
        }

        const std::string token = input_.substr(start, index_ - start);
        if (token.empty() || token == "-") {
            if (error != nullptr) {
                *error = "expected JSON number";
            }
            return false;
        }

        char *end = nullptr;
        *value = std::strtod(token.c_str(), &end);
        if (end == nullptr || *end != '\0') {
            if (error != nullptr) {
                *error = "invalid JSON number";
            }
            return false;
        }
        return true;
    }

    const std::string &input_;
    size_t index_ = 0;
};

const JsonValue *FindObjectField(const JsonValue &object, const std::string &field_name)
{
    if (object.type != JsonValue::Type::kObject) {
        return nullptr;
    }
    const auto iterator = object.object_values.find(field_name);
    if (iterator == object.object_values.end()) {
        return nullptr;
    }
    return &iterator->second;
}

bool GetRequiredObjectField(
    const JsonValue &object, const std::string &field_name, const JsonValue **field, std::string *error)
{
    *field = FindObjectField(object, field_name);
    if (*field == nullptr || (*field)->type != JsonValue::Type::kObject) {
        if (error != nullptr) {
            *error = "missing or invalid object field `" + field_name + "`";
        }
        return false;
    }
    return true;
}

bool GetOptionalObjectField(
    const JsonValue &object, const std::string &field_name, const JsonValue **field)
{
    *field = FindObjectField(object, field_name);
    return *field != nullptr && (*field)->type == JsonValue::Type::kObject;
}

bool GetRequiredStringField(
    const JsonValue &object, const std::string &field_name, std::string *value, std::string *error)
{
    const JsonValue *field = FindObjectField(object, field_name);
    if (field == nullptr || field->type != JsonValue::Type::kString) {
        if (error != nullptr) {
            *error = "missing or invalid string field `" + field_name + "`";
        }
        return false;
    }
    *value = field->string_value;
    return true;
}

bool GetOptionalStringField(
    const JsonValue &object, const std::string &field_name, std::string *value, bool *has_value)
{
    const JsonValue *field = FindObjectField(object, field_name);
    if (field == nullptr) {
        *has_value = false;
        return true;
    }
    if (field->type != JsonValue::Type::kString) {
        return false;
    }
    *has_value = true;
    *value = field->string_value;
    return true;
}

bool GetRequiredBoolField(
    const JsonValue &object, const std::string &field_name, bool *value, std::string *error)
{
    const JsonValue *field = FindObjectField(object, field_name);
    if (field == nullptr || field->type != JsonValue::Type::kBool) {
        if (error != nullptr) {
            *error = "missing or invalid bool field `" + field_name + "`";
        }
        return false;
    }
    *value = field->bool_value;
    return true;
}

bool GetRequiredDoubleField(
    const JsonValue &object, const std::string &field_name, double *value, std::string *error)
{
    const JsonValue *field = FindObjectField(object, field_name);
    if (field == nullptr || field->type != JsonValue::Type::kNumber) {
        if (error != nullptr) {
            *error = "missing or invalid number field `" + field_name + "`";
        }
        return false;
    }
    *value = field->number_value;
    return true;
}

bool GetOptionalDoubleField(
    const JsonValue &object, const std::string &field_name, double *value, bool *has_value)
{
    const JsonValue *field = FindObjectField(object, field_name);
    if (field == nullptr) {
        *has_value = false;
        return true;
    }
    if (field->type != JsonValue::Type::kNumber) {
        return false;
    }
    *has_value = true;
    *value = field->number_value;
    return true;
}

bool ReadIntegralNumberField(
    const JsonValue &object, const std::string &field_name, int32_t *value, bool required, std::string *error, bool *has_value = nullptr)
{
    const JsonValue *field = FindObjectField(object, field_name);
    if (field == nullptr) {
        if (has_value != nullptr) {
            *has_value = false;
        }
        if (!required) {
            return true;
        }
        if (error != nullptr) {
            *error = "missing number field `" + field_name + "`";
        }
        return false;
    }
    if (field->type != JsonValue::Type::kNumber) {
        if (error != nullptr) {
            *error = "invalid number field `" + field_name + "`";
        }
        return false;
    }

    double integer_part = 0.0;
    if (std::modf(field->number_value, &integer_part) != 0.0 ||
        integer_part < static_cast<double>(std::numeric_limits<int32_t>::min()) ||
        integer_part > static_cast<double>(std::numeric_limits<int32_t>::max())) {
        if (error != nullptr) {
            *error = "field `" + field_name + "` must be a 32-bit integer";
        }
        return false;
    }

    if (has_value != nullptr) {
        *has_value = true;
    }
    *value = static_cast<int32_t>(integer_part);
    return true;
}

bool GetRequiredInt32Field(
    const JsonValue &object, const std::string &field_name, int32_t *value, std::string *error)
{
    return ReadIntegralNumberField(object, field_name, value, true, error);
}

bool GetOptionalInt32Field(
    const JsonValue &object, const std::string &field_name, int32_t *value, bool *has_value, std::string *error)
{
    return ReadIntegralNumberField(object, field_name, value, false, error, has_value);
}

bool GetRequiredStringArrayField(
    const JsonValue &object, const std::string &field_name, std::vector<std::string> *values, std::string *error)
{
    const JsonValue *field = FindObjectField(object, field_name);
    if (field == nullptr || field->type != JsonValue::Type::kArray) {
        if (error != nullptr) {
            *error = "missing or invalid array field `" + field_name + "`";
        }
        return false;
    }

    values->clear();
    values->reserve(field->array_values.size());
    for (const JsonValue &item : field->array_values) {
        if (item.type != JsonValue::Type::kString) {
            if (error != nullptr) {
                *error = "array field `" + field_name + "` must contain only strings";
            }
            return false;
        }
        values->push_back(item.string_value);
    }
    return true;
}

bool ParseJsonRoot(const std::string &message_json, JsonValue *root, std::string *error)
{
    JsonParser parser(message_json);
    if (!parser.Parse(root, error)) {
        return false;
    }
    if (root->type != JsonValue::Type::kObject) {
        if (error != nullptr) {
            *error = "session transport messages must be JSON objects";
        }
        return false;
    }
    return true;
}

bool ParseHostHelloFromJson(const JsonValue &root, core::HostHelloRequest *request, std::string *error)
{
    const JsonValue *video_limits = nullptr;
    if (!GetRequiredStringField(root, "type", &request->type, error) ||
        !GetRequiredStringField(root, "session_id", &request->session_id, error) ||
        !GetRequiredInt32Field(root, "protocol_major", &request->protocol_major, error) ||
        !GetRequiredInt32Field(root, "protocol_minor", &request->protocol_minor, error) ||
        !GetRequiredStringField(root, "host_version", &request->host_version, error) ||
        !GetRequiredStringArrayField(root, "requested_features", &request->requested_features, error) ||
        !GetRequiredStringArrayField(root, "supported_video_codecs", &request->supported_video_codecs, error) ||
        !GetRequiredStringArrayField(root, "preferred_video_codecs", &request->preferred_video_codecs, error) ||
        !GetRequiredObjectField(root, "video_limits", &video_limits, error) ||
        !GetRequiredInt32Field(*video_limits, "max_width", &request->video_max_width, error) ||
        !GetRequiredInt32Field(*video_limits, "max_height", &request->video_max_height, error) ||
        !GetRequiredInt32Field(*video_limits, "max_fps", &request->video_max_fps, error) ||
        !GetOptionalInt32Field(
            *video_limits, "bitrate_kbps", &request->video_bitrate_kbps, &request->has_video_bitrate_kbps, error)) {
        return false;
    }

    request->has_video_iframe_interval_ms = false;
    request->video_iframe_interval_ms = 0;
    return true;
}

bool ParseSessionConfigFromJson(const JsonValue &root, core::SessionConfigRequest *request, std::string *error)
{
    const JsonValue *video = nullptr;
    const JsonValue *control = nullptr;
    if (!GetRequiredStringField(root, "session_id", &request->session_id, error) ||
        !GetRequiredStringField(root, "selected_video_codec", &request->selected_video_codec, error) ||
        !GetRequiredObjectField(root, "video", &video, error) ||
        !GetRequiredObjectField(root, "control", &control, error) ||
        !GetRequiredInt32Field(*video, "max_width", &request->video_max_width, error) ||
        !GetRequiredInt32Field(*video, "max_height", &request->video_max_height, error) ||
        !GetRequiredInt32Field(*video, "max_fps", &request->video_max_fps, error) ||
        !GetOptionalInt32Field(*video, "bitrate_kbps", &request->video_bitrate_kbps, &request->has_video_bitrate_kbps, error) ||
        !GetOptionalInt32Field(
            *video,
            "iframe_interval_ms",
            &request->video_iframe_interval_ms,
            &request->has_video_iframe_interval_ms,
            error) ||
        !GetRequiredBoolField(*control, "enabled", &request->control_enabled, error)) {
        return false;
    }
    return true;
}

bool ParseStopSessionFromJson(const JsonValue &root, core::StopSessionRequest *request, std::string *error)
{
    if (!GetRequiredStringField(root, "session_id", &request->session_id, error)) {
        return false;
    }
    if (!GetOptionalStringField(root, "reason", &request->reason, &request->has_reason)) {
        if (error != nullptr) {
            *error = "invalid string field `reason`";
        }
        return false;
    }
    return true;
}

bool ParseControlEventFromJson(const JsonValue &root, core::ControlEventRequest *request, std::string *error)
{
    const JsonValue *position_norm = nullptr;
    if (!GetRequiredStringField(root, "type", &request->type, error) ||
        !GetRequiredStringField(root, "session_id", &request->session_id, error) ||
        !GetRequiredStringField(root, "event_type", &request->event_type, error) ||
        !GetRequiredInt32Field(root, "sequence", &request->sequence, error) ||
        !GetOptionalInt32Field(root, "pointer_id", &request->pointer_id, &request->has_pointer_id, error) ||
        !GetOptionalInt32Field(root, "scroll_delta_x", &request->scroll_delta_x, &request->has_scroll_delta_x, error) ||
        !GetOptionalInt32Field(root, "scroll_delta_y", &request->scroll_delta_y, &request->has_scroll_delta_y, error)) {
        return false;
    }

    bool has_button = false;
    if (!GetOptionalStringField(root, "button", &request->button, &has_button)) {
        if (error != nullptr) {
            *error = "invalid string field `button`";
        }
        return false;
    }
    if (!has_button) {
        request->button.clear();
    }

    bool has_key_code = false;
    if (!GetOptionalStringField(root, "key_code", &request->key_code, &has_key_code)) {
        if (error != nullptr) {
            *error = "invalid string field `key_code`";
        }
        return false;
    }
    if (!has_key_code) {
        request->key_code.clear();
    }

    bool has_text = false;
    if (!GetOptionalStringField(root, "text", &request->text, &has_text)) {
        if (error != nullptr) {
            *error = "invalid string field `text`";
        }
        return false;
    }
    if (!has_text) {
        request->text.clear();
    }

    bool has_device_action = false;
    if (!GetOptionalStringField(root, "device_action", &request->device_action, &has_device_action)) {
        if (error != nullptr) {
            *error = "invalid string field `device_action`";
        }
        return false;
    }
    if (!has_device_action) {
        request->device_action.clear();
    }

    request->has_position_norm = GetOptionalObjectField(root, "position_norm", &position_norm);
    if (request->has_position_norm) {
        bool has_x = false;
        bool has_y = false;
        if (!GetOptionalDoubleField(*position_norm, "x", &request->position_norm.x, &has_x) ||
            !GetOptionalDoubleField(*position_norm, "y", &request->position_norm.y, &has_y)) {
            if (error != nullptr) {
                *error = "invalid number field in `position_norm`";
            }
            return false;
        }
        if (!has_x || !has_y) {
            if (error != nullptr) {
                *error = "position_norm requires both x and y";
            }
            return false;
        }
    }

    return true;
}

std::vector<core::VideoCodecDescriptor> BuildAvailableVideoCodecs()
{
    return {
        video::h264::BuildH264CodecDescriptor(),
        video::jpeg::BuildJpegCodecDescriptor()
    };
}

std::vector<std::string> BuildSharedVideoCodecs(
    const std::vector<std::string> &supported_video_codecs, const std::vector<core::VideoCodecDescriptor> &available_video_codecs)
{
    std::vector<std::string> shared_video_codecs;
    shared_video_codecs.reserve(available_video_codecs.size());
    for (const core::VideoCodecDescriptor &descriptor : available_video_codecs) {
        if (ContainsValue(supported_video_codecs, descriptor.codec)) {
            shared_video_codecs.push_back(descriptor.codec);
        }
    }
    return shared_video_codecs;
}

std::string SelectSuggestedCodec(const core::HostHelloRequest &request, const std::vector<std::string> &shared_video_codecs)
{
    for (const std::string &codec : request.preferred_video_codecs) {
        if (ContainsValue(shared_video_codecs, codec)) {
            return codec;
        }
    }
    if (!shared_video_codecs.empty()) {
        return shared_video_codecs.front();
    }
    return "";
}

core::AuthorizationStateMap BuildAuthorizationStateMap()
{
    return {
        core::kAuthorizationGranted,
        core::kAuthorizationGranted
    };
}

bool IsGranted(const std::string &state)
{
    return state == core::kAuthorizationGranted;
}

std::string MapAuthorizationStateForFeature(
    const core::AuthorizationStateMap &authorization, const std::string &feature)
{
    if (feature == core::kFeatureVideo) {
        return authorization.video_capture;
    }
    if (feature == core::kFeatureControl) {
        return authorization.input_injection;
    }
    return core::kAuthorizationUnsupported;
}

std::string EscapeJsonString(const std::string &value)
{
    std::string escaped;
    escaped.reserve(value.size());
    for (char ch : value) {
        switch (ch) {
            case '\\':
                escaped.append("\\\\");
                break;
            case '"':
                escaped.append("\\\"");
                break;
            case '\n':
                escaped.append("\\n");
                break;
            case '\r':
                escaped.append("\\r");
                break;
            case '\t':
                escaped.append("\\t");
                break;
            default:
                escaped.push_back(ch);
                break;
        }
    }
    return escaped;
}

void AppendJsonString(std::ostringstream &stream, const std::string &value)
{
    stream << '"' << EscapeJsonString(value) << '"';
}

void AppendJsonStringArray(std::ostringstream &stream, const std::vector<std::string> &values)
{
    stream << '[';
    for (size_t index = 0; index < values.size(); ++index) {
        if (index > 0) {
            stream << ',';
        }
        AppendJsonString(stream, values[index]);
    }
    stream << ']';
}

std::string SerializeAuthorization(const core::AuthorizationStateMap &authorization)
{
    std::ostringstream stream;
    stream << '{';
    stream << "\"video_capture\":";
    AppendJsonString(stream, authorization.video_capture);
    stream << ",\"input_injection\":";
    AppendJsonString(stream, authorization.input_injection);
    stream << '}';
    return stream.str();
}

std::string SerializeDisplay(const core::DisplayInfo &display)
{
    std::ostringstream stream;
    stream << '{';
    stream << "\"width\":" << display.width;
    stream << ",\"height\":" << display.height;
    stream << ",\"rotation\":" << display.rotation;
    stream << '}';
    return stream.str();
}

std::string SerializeVideoCodecDescriptors(const std::vector<core::VideoCodecDescriptor> &codecs)
{
    std::ostringstream stream;
    stream << '[';
    for (size_t index = 0; index < codecs.size(); ++index) {
        if (index > 0) {
            stream << ',';
        }
        const core::VideoCodecDescriptor &codec = codecs[index];
        stream << '{';
        stream << "\"codec\":";
        AppendJsonString(stream, codec.codec);
        stream << ",\"encoder_kind\":";
        AppendJsonString(stream, codec.encoder_kind);
        stream << ",\"max_width\":" << codec.max_width;
        stream << ",\"max_height\":" << codec.max_height;
        stream << ",\"max_fps\":" << codec.max_fps;
        if (!codec.bitrate_control.empty()) {
            stream << ",\"bitrate_control\":";
            AppendJsonString(stream, codec.bitrate_control);
        }
        stream << '}';
    }
    stream << ']';
    return stream.str();
}

std::string SerializeChannelBinding(const core::ChannelBinding &binding)
{
    std::ostringstream stream;
    stream << '{';
    stream << "\"name\":";
    AppendJsonString(stream, binding.name);
    stream << ",\"state\":";
    AppendJsonString(stream, binding.state);
    stream << ",\"payload_type\":";
    AppendJsonString(stream, binding.payload_type);
    stream << ",\"activation\":";
    AppendJsonString(stream, binding.activation);
    stream << '}';
    return stream.str();
}

std::string SerializeChannelLayout(const core::ChannelLayout &layout)
{
    std::ostringstream stream;
    stream << '{';
    stream << "\"session\":" << SerializeChannelBinding(layout.session);
    stream << ",\"video\":" << SerializeChannelBinding(layout.video);
    stream << '}';
    return stream.str();
}

core::SessionTransportEnvelope BuildEnvelope(const std::string &message_type, const std::string &payload_json)
{
    return {
        core::kChannelNameSession,
        core::kPayloadTypeUtf8Json,
        message_type,
        payload_json
    };
}

core::DeviceHelloPreview BuildDeviceHelloPreview(
    const core::HostHelloRequest &request,
    const core::AuthorizationStateMap &authorization,
    const std::vector<core::VideoCodecDescriptor> &available_video_codecs)
{
    return {
        core::kSessionMessageTypeDeviceHello,
        request.session_id,
        core::kProtocolMajor,
        core::kProtocolMinor,
        core::kNativeCoreVersion,
        "HarmonyOS Companion Runtime Device",
        authorization,
        {core::kFeatureVideo, core::kFeatureControl},
        available_video_codecs,
        video::BuildDeviceDisplayInfo()
    };
}

std::string SerializeDeviceHello(const core::DeviceHelloPreview &device_hello)
{
    std::ostringstream stream;
    stream << '{';
    stream << "\"type\":";
    AppendJsonString(stream, device_hello.type);
    stream << ",\"session_id\":";
    AppendJsonString(stream, device_hello.session_id);
    stream << ",\"protocol_major\":" << device_hello.protocol_major;
    stream << ",\"protocol_minor\":" << device_hello.protocol_minor;
    stream << ",\"companion_version\":";
    AppendJsonString(stream, device_hello.companion_version);
    stream << ",\"device_name\":";
    AppendJsonString(stream, device_hello.device_name);
    stream << ",\"authorization\":" << SerializeAuthorization(device_hello.authorization);
    stream << ",\"available_features\":";
    AppendJsonStringArray(stream, device_hello.available_features);
    stream << ",\"available_video_codecs\":";
    stream << SerializeVideoCodecDescriptors(device_hello.available_video_codecs);
    stream << ",\"display\":" << SerializeDisplay(device_hello.display);
    stream << '}';
    return stream.str();
}

core::VideoNegotiationPreview BuildVideoNegotiationPreview(
    const core::HostHelloRequest &request, const std::vector<std::string> &shared_video_codecs, const std::string &selected_codec)
{
    const bool fallback_available = ContainsValue(shared_video_codecs, core::kVideoCodecJpeg);
    const bool fallback_used = selected_codec == core::kVideoCodecJpeg &&
        ContainsValue(request.preferred_video_codecs, core::kVideoCodecH264) &&
        !ContainsValue(shared_video_codecs, core::kVideoCodecH264);

    return {
        selected_codec,
        fallback_available,
        fallback_available ? core::kVideoCodecJpeg : "",
        fallback_used,
        "host_preference_order",
        fallback_used ? "jpeg_fallback_after_h264_miss" : "preferred_common_codec",
        request.supported_video_codecs,
        request.preferred_video_codecs,
        shared_video_codecs,
        selected_codec == core::kVideoCodecH264 ? "video/h264_video_path" : "video/jpeg_video_path"
    };
}

core::SessionConfigRequest BuildSuggestedSessionConfig(
    const core::HostHelloRequest &request, const core::VideoNegotiationPreview &negotiation)
{
    const bool uses_bitrate_controls = negotiation.selected_video_codec == core::kVideoCodecH264;
    return {
        request.session_id,
        negotiation.selected_video_codec,
        request.video_max_width,
        request.video_max_height,
        request.video_max_fps,
        uses_bitrate_controls && request.has_video_bitrate_kbps,
        request.video_bitrate_kbps,
        uses_bitrate_controls && request.has_video_iframe_interval_ms,
        request.video_iframe_interval_ms,
        ContainsValue(request.requested_features, core::kFeatureControl)
    };
}

core::SessionErrorMessage BuildSessionError(
    const std::string &session_id,
    const std::string &code,
    const std::string &message,
    bool retryable)
{
    return {
        core::kSessionMessageTypeSessionError,
        session_id,
        code,
        message,
        retryable
    };
}

std::string SerializeSessionError(const core::SessionErrorMessage &session_error)
{
    std::ostringstream stream;
    stream << '{';
    stream << "\"type\":";
    AppendJsonString(stream, session_error.type);
    stream << ",\"session_id\":";
    AppendJsonString(stream, session_error.session_id);
    stream << ",\"code\":";
    AppendJsonString(stream, session_error.code);
    stream << ",\"message\":";
    AppendJsonString(stream, session_error.message);
    stream << ",\"retryable\":" << (session_error.retryable ? "true" : "false");
    stream << '}';
    return stream.str();
}

core::SessionReadyPreview BuildSessionReadyPreview(const core::SessionConfigRequest &request)
{
    const bool use_h264_path = request.selected_video_codec == core::kVideoCodecH264;
    return {
        core::kSessionMessageTypeSessionReady,
        request.session_id,
        request.selected_video_codec,
        use_h264_path ? video::h264::BuildChannelLayout() : video::jpeg::BuildChannelLayout(),
        use_h264_path ? video::h264::ResolveSessionDisplay(request) : video::jpeg::ResolveSessionDisplay(request)
    };
}

std::string SerializeSessionReady(const core::SessionReadyPreview &session_ready)
{
    std::ostringstream stream;
    stream << '{';
    stream << "\"type\":";
    AppendJsonString(stream, session_ready.type);
    stream << ",\"session_id\":";
    AppendJsonString(stream, session_ready.session_id);
    stream << ",\"selected_video_codec\":";
    AppendJsonString(stream, session_ready.selected_video_codec);
    stream << ",\"channel_layout\":";
    stream << SerializeChannelLayout(session_ready.channel_layout);
    stream << ",\"display\":";
    stream << SerializeDisplay(session_ready.display);
    stream << '}';
    return stream.str();
}

std::vector<std::string> BuildPipelineStages(const core::SessionConfigRequest &request)
{
    if (request.selected_video_codec == core::kVideoCodecH264) {
        return video::h264::BuildPipelineStages();
    }
    return video::jpeg::BuildPipelineStages();
}

core::SessionTransportState BuildInitialState(
    const core::HostHelloRequest &request,
    const core::AuthorizationStateMap &authorization,
    const std::vector<std::string> &shared_video_codecs,
    const std::vector<core::VideoCodecDescriptor> &available_video_codecs)
{
    return {
        request.session_id,
        core::kProtocolMajor,
        core::kProtocolMinor,
        core::kSessionPhaseAwaitingSessionConfig,
        authorization,
        request.requested_features,
        shared_video_codecs,
        available_video_codecs,
        "",
        false,
        video::BuildChannelLayout("opens after session_ready once the host binds the negotiated video channel")
    };
}

bool RequiresGrantedAuthorization(const std::vector<std::string> &requested_features, const core::AuthorizationStateMap &authorization)
{
    for (const std::string &feature : requested_features) {
        if (!IsGranted(MapAuthorizationStateForFeature(authorization, feature))) {
            return false;
        }
    }
    return true;
}

core::SessionErrorMessage BuildAuthorizationFailure(
    const std::string &session_id, const std::vector<std::string> &requested_features, const core::AuthorizationStateMap &authorization)
{
    for (const std::string &feature : requested_features) {
        const std::string state = MapAuthorizationStateForFeature(authorization, feature);
        if (state == core::kAuthorizationUnsupported) {
            return BuildSessionError(
                session_id,
                core::kSessionErrorFeatureUnsupported,
                "Required feature authorization is unsupported on the device runtime.",
                false);
        }
        if (state == core::kAuthorizationDenied) {
            return BuildSessionError(
                session_id,
                core::kSessionErrorAuthorizationDenied,
                "Required feature authorization is denied for this session.",
                false);
        }
        if (state == core::kAuthorizationNeedsUserAction) {
            return BuildSessionError(
                session_id,
                core::kSessionErrorAuthorizationPending,
                "Required feature authorization still needs user action before session_config.",
                true);
        }
    }

    return BuildSessionError(
        session_id,
        core::kSessionErrorSessionConfigRejected,
        "Required authorization is not ready for session_config.",
        false);
}

core::SessionTransportConfigureResult BuildRejectedConfigureResult(
    const core::SessionTransportState &state, const core::SessionErrorMessage &session_error, const std::vector<std::string> &notes)
{
    core::SessionTransportState failed_state = state;
    failed_state.phase = core::kSessionPhaseFailed;
    return {
        failed_state,
        false,
        false,
        {},
        false,
        {},
        true,
        session_error,
        BuildEnvelope(core::kSessionMessageTypeSessionError, SerializeSessionError(session_error)),
        {},
        notes
    };
}

}  // namespace

bool TryParseSessionMessageType(const std::string &message_json, std::string *message_type, std::string *error)
{
    JsonValue root;
    if (!ParseJsonRoot(message_json, &root, error)) {
        return false;
    }
    return GetRequiredStringField(root, "type", message_type, error);
}

bool TryParseHostHelloMessage(
    const std::string &message_json, core::HostHelloRequest *request, std::string *error)
{
    JsonValue root;
    if (!ParseJsonRoot(message_json, &root, error) || !ParseHostHelloFromJson(root, request, error)) {
        return false;
    }
    if (request->type != core::kSessionMessageTypeHostHello) {
        if (error != nullptr) {
            *error = "expected host_hello on the session channel";
        }
        return false;
    }
    return true;
}

bool TryParseSessionConfigMessage(
    const std::string &message_json, core::SessionConfigRequest *request, std::string *error)
{
    JsonValue root;
    std::string message_type;
    if (!ParseJsonRoot(message_json, &root, error) ||
        !GetRequiredStringField(root, "type", &message_type, error) ||
        !ParseSessionConfigFromJson(root, request, error)) {
        return false;
    }
    if (message_type != core::kSessionMessageTypeSessionConfig) {
        if (error != nullptr) {
            *error = "expected session_config on the session channel";
        }
        return false;
    }
    return true;
}

bool TryParseStopSessionMessage(
    const std::string &message_json, core::StopSessionRequest *request, std::string *error)
{
    JsonValue root;
    std::string message_type;
    if (!ParseJsonRoot(message_json, &root, error) ||
        !GetRequiredStringField(root, "type", &message_type, error) ||
        !ParseStopSessionFromJson(root, request, error)) {
        return false;
    }
    if (message_type != core::kSessionMessageTypeStopSession) {
        if (error != nullptr) {
            *error = "expected stop_session on the session channel";
        }
        return false;
    }
    return true;
}

bool TryParseControlEventMessage(
    const std::string &message_json, core::ControlEventRequest *request, std::string *error)
{
    JsonValue root;
    if (!ParseJsonRoot(message_json, &root, error) || !ParseControlEventFromJson(root, request, error)) {
        return false;
    }
    if (request->type != core::kControlEventType) {
        if (error != nullptr) {
            *error = "expected control_event on the session channel";
        }
        return false;
    }
    return true;
}

std::string SerializeSessionErrorMessage(const core::SessionErrorMessage &session_error)
{
    return SerializeSessionError(session_error);
}

core::HostHelloRequest BuildHostHelloRequest(const core::SessionNegotiationRequest &request)
{
    core::HostHelloRequest host_hello = {};
    host_hello.type = core::kSessionMessageTypeHostHello;
    host_hello.session_id = request.session_id;
    host_hello.protocol_major = core::kProtocolMajor;
    host_hello.protocol_minor = core::kProtocolMinor;
    host_hello.host_version = "shell-preview";
    host_hello.requested_features = {core::kFeatureVideo};
    if (request.control_enabled) {
        host_hello.requested_features.push_back(core::kFeatureControl);
    }
    host_hello.supported_video_codecs = request.host_supported_video_codecs;
    host_hello.preferred_video_codecs = request.preferred_video_codecs;
    host_hello.video_max_width = request.video_max_width;
    host_hello.video_max_height = request.video_max_height;
    host_hello.video_max_fps = request.video_max_fps;
    host_hello.has_video_bitrate_kbps = request.has_video_bitrate_kbps;
    host_hello.video_bitrate_kbps = request.video_bitrate_kbps;
    host_hello.has_video_iframe_interval_ms = request.has_video_iframe_interval_ms;
    host_hello.video_iframe_interval_ms = request.video_iframe_interval_ms;
    return host_hello;
}

core::SessionConfigPreview BuildSessionConfigPreview(const core::SessionConfigRequest &request)
{
    return {
        core::kSessionMessageTypeSessionConfig,
        request.session_id,
        request.selected_video_codec,
        {
            request.video_max_width,
            request.video_max_height,
            request.video_max_fps,
            request.has_video_bitrate_kbps,
            request.video_bitrate_kbps,
            request.has_video_iframe_interval_ms,
            request.video_iframe_interval_ms
        },
        {
            request.control_enabled
        }
    };
}

core::SessionTransportOpenResult OpenSessionChannel(const core::HostHelloRequest &request)
{
    const core::AuthorizationStateMap authorization = BuildAuthorizationStateMap();
    const std::vector<core::VideoCodecDescriptor> available_video_codecs = BuildAvailableVideoCodecs();
    const std::vector<std::string> shared_video_codecs =
        BuildSharedVideoCodecs(request.supported_video_codecs, available_video_codecs);

    core::SessionTransportState state = BuildInitialState(request, authorization, shared_video_codecs, available_video_codecs);

    if (request.protocol_major != core::kProtocolMajor) {
        const core::SessionErrorMessage session_error = BuildSessionError(
            request.session_id,
            core::kSessionErrorProtocolMajorMismatch,
            "host_hello protocol_major does not match the device runtime transport contract.",
            false);
        state.phase = core::kSessionPhaseFailed;
        return {
            state,
            false,
            false,
            {},
            true,
            session_error,
            {},
            {},
            BuildEnvelope(core::kSessionMessageTypeSessionError, SerializeSessionError(session_error)),
            {
                "The session channel rejects host_hello before device_hello when protocol_major differs.",
                "ArkTS remains uninvolved while the native transport runtime reports the incompatibility."
            }
        };
    }

    for (const std::string &feature : request.requested_features) {
        if (feature != core::kFeatureVideo && feature != core::kFeatureControl) {
            const core::SessionErrorMessage session_error = BuildSessionError(
                request.session_id,
                core::kSessionErrorFeatureUnsupported,
                "host_hello requested a feature outside the current MVP device contract.",
                false);
            state.phase = core::kSessionPhaseFailed;
            return {
                state,
                false,
                false,
                {},
                true,
                session_error,
                {},
                {},
                BuildEnvelope(core::kSessionMessageTypeSessionError, SerializeSessionError(session_error)),
                {
                    "The session runtime only accepts the MVP requested_features set of video and control."
                }
            };
        }
    }

    if (shared_video_codecs.empty()) {
        const core::SessionErrorMessage session_error = BuildSessionError(
            request.session_id,
            core::kSessionErrorNoSharedVideoCodec,
            "No shared video codec exists between host_hello.supported_video_codecs and device runtime capabilities.",
            false);
        state.phase = core::kSessionPhaseFailed;
        return {
            state,
            false,
            false,
            {},
            true,
            session_error,
            {},
            {},
            BuildEnvelope(core::kSessionMessageTypeSessionError, SerializeSessionError(session_error)),
            {
                "The device transport runtime aborts before session_config when the shared codec set is empty."
            }
        };
    }

    const core::DeviceHelloPreview device_hello =
        BuildDeviceHelloPreview(request, authorization, available_video_codecs);
    const std::string selected_codec = SelectSuggestedCodec(request, shared_video_codecs);
    const core::VideoNegotiationPreview negotiation =
        BuildVideoNegotiationPreview(request, shared_video_codecs, selected_codec);
    const core::SessionConfigRequest suggested_session_config =
        BuildSuggestedSessionConfig(request, negotiation);

    return {
        state,
        true,
        true,
        device_hello,
        false,
        {},
        negotiation,
        suggested_session_config,
        BuildEnvelope(core::kSessionMessageTypeDeviceHello, SerializeDeviceHello(device_hello)),
        {
            "The session channel opens first and emits device_hello as UTF-8 JSON on the session transport.",
            "The video channel remains pending_open until the host later sends session_config and receives session_ready.",
            "H.264 remains the preferred shared codec while JPEG stays available as the required fallback."
        }
    };
}

core::SessionTransportConfigureResult ConfigureSessionChannel(
    const core::SessionTransportState &state, const core::SessionConfigRequest &request)
{
    if (state.phase != core::kSessionPhaseAwaitingSessionConfig) {
        return BuildRejectedConfigureResult(
            state,
            BuildSessionError(
                request.session_id,
                core::kSessionErrorSessionConfigRejected,
                "session_config is only accepted while the session channel is awaiting configuration.",
                false),
            {
                "The device transport runtime refuses session_config after the session has failed, stopped, or already become ready."
            });
    }

    if (request.session_id != state.session_id) {
        return BuildRejectedConfigureResult(
            state,
            BuildSessionError(
                request.session_id,
                core::kSessionErrorSessionConfigRejected,
                "session_config.session_id must match the active session channel state.",
                false),
            {
                "Both logical channels are bound by one session_id in the native runtime."
            });
    }

    if (!ContainsValue(state.shared_video_codecs, request.selected_video_codec)) {
        return BuildRejectedConfigureResult(
            state,
            BuildSessionError(
                request.session_id,
                core::kSessionErrorSessionConfigRejected,
                "session_config.selected_video_codec is not present in the negotiated shared codec set.",
                false),
            {
                "The device only accepts final codec selection from the shared host/device capability intersection."
            });
    }

    if (request.control_enabled && !ContainsValue(state.requested_features, core::kFeatureControl)) {
        return BuildRejectedConfigureResult(
            state,
            BuildSessionError(
                request.session_id,
                core::kSessionErrorSessionConfigRejected,
                "session_config.control.enabled cannot enable control when host_hello did not request it.",
                false),
            {
                "Control enablement stays on the session channel and must be declared in host_hello.requested_features first."
            });
    }

    if (!RequiresGrantedAuthorization(state.requested_features, state.authorization)) {
        return BuildRejectedConfigureResult(
            state,
            BuildAuthorizationFailure(request.session_id, state.requested_features, state.authorization),
            {
                "The device runtime checks per-feature authorization before returning session_ready.",
                "Video must never activate while authorization is pending, denied, or unsupported."
            });
    }

    core::SessionTransportState ready_state = state;
    ready_state.phase = core::kSessionPhaseReady;
    ready_state.selected_video_codec = request.selected_video_codec;
    ready_state.control_enabled = request.control_enabled;

    const core::SessionReadyPreview session_ready = BuildSessionReadyPreview(request);
    ready_state.channel_layout = session_ready.channel_layout;
    const core::VideoTransportState video_transport =
        BuildVideoTransportState(ready_state, session_ready, BuildPipelineStages(request));

    return {
        ready_state,
        true,
        true,
        session_ready,
        true,
        video_transport,
        false,
        {},
        BuildEnvelope(core::kSessionMessageTypeSessionReady, SerializeSessionReady(session_ready)),
        video_transport.pipeline_stages,
        {
            "session_ready is emitted on the session channel before the video transport may become active.",
            "The session channel remains open for control_event and teardown traffic after session_ready.",
            "The video channel stays binary-only and waits for an explicit host-side open before packet delivery."
        }
    };
}

core::SessionTransportStopResult StopSessionChannel(
    const core::SessionTransportState &state, const core::StopSessionRequest &request)
{
    core::SessionTransportState stopped_state = state;
    if (request.session_id != state.session_id) {
        return {
            stopped_state,
            false,
            {
                "stop_session.session_id must match the current device session state."
            }
        };
    }

    stopped_state.phase = core::kSessionPhaseStopped;
    stopped_state.control_enabled = false;
    stopped_state.channel_layout.video.state = core::kChannelStatePendingOpen;

    std::vector<std::string> notes = {
        "stop_session is handled on the session channel and tears down the device-side runtime state."
    };
    if (request.has_reason && !request.reason.empty()) {
        notes.push_back("The host supplied a stop reason for logs or higher-level teardown reporting.");
    }

    return {
        stopped_state,
        true,
        notes
    };
}

}  // namespace transport
}  // namespace hscrcpy
