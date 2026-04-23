#include "control/control_message_handler.h"

#include <cmath>

#include "control/input_injector.h"

namespace hscrcpy {
namespace control {

namespace {

bool IsPointerEventType(const std::string &event_type)
{
    return event_type == core::kControlEventPointerDown || event_type == core::kControlEventPointerMove ||
           event_type == core::kControlEventPointerUp;
}

bool IsAllowedButton(const std::string &button)
{
    return button.empty() || button == core::kControlButtonPrimary || button == core::kControlButtonSecondary ||
           button == core::kControlButtonMiddle;
}

bool IsNormalizedCoordinate(double value)
{
    return std::isfinite(value) && value >= 0.0 && value <= 1.0;
}

core::ControlHandlingPreview BuildRejectedPreview(const core::ControlEventRequest &event, const std::string &note)
{
    return {
        core::kControlEventType,
        event.session_id,
        event.event_type,
        event.sequence,
        "session/control_event -> control/control_message_handler",
        false,
        "rejected",
        "not_planned",
        "unsupported",
        {
            "read control_event envelope",
            "validate message against the host-device MVP contract"
        },
        {note}
    };
}

}  // namespace

core::ControlHandlingPreview PreviewControlHandling(const core::ControlEventRequest &event)
{
    if (event.type != core::kControlEventType) {
        return BuildRejectedPreview(event, "Only control_event messages are accepted on the device control path.");
    }
    if (event.session_id.empty()) {
        return BuildRejectedPreview(event, "session_id must be present before device control handling starts.");
    }
    if (event.sequence < 0) {
        return BuildRejectedPreview(event, "sequence must be zero or greater for device-side control ordering.");
    }
    if (event.event_type.empty()) {
        return BuildRejectedPreview(event, "event_type is required for every control_event payload.");
    }

    if (IsPointerEventType(event.event_type)) {
        if (!event.has_position_norm) {
            return BuildRejectedPreview(event, "Pointer events require position_norm.x and position_norm.y.");
        }
        if (!IsNormalizedCoordinate(event.position_norm.x) || !IsNormalizedCoordinate(event.position_norm.y)) {
            return BuildRejectedPreview(event, "Pointer position_norm coordinates must stay within [0.0, 1.0].");
        }
        if (!event.has_pointer_id || event.pointer_id < 0) {
            return BuildRejectedPreview(event, "Pointer events require a non-negative pointer_id.");
        }
        if (!IsAllowedButton(event.button)) {
            return BuildRejectedPreview(event, "button must be primary, secondary, middle, or omitted.");
        }
    } else if (event.event_type == core::kControlEventScroll) {
        if (!event.has_scroll_delta_x && !event.has_scroll_delta_y) {
            return BuildRejectedPreview(event, "Scroll events require scroll_delta_x, scroll_delta_y, or both.");
        }
    } else if (event.event_type == core::kControlEventKeyDown || event.event_type == core::kControlEventKeyUp) {
        if (event.key_code.empty()) {
            return BuildRejectedPreview(event, "Keyboard control events require key_code.");
        }
    } else if (event.event_type == core::kControlEventDeviceAction) {
        if (event.device_action.empty()) {
            return BuildRejectedPreview(event, "device_action events require device_action.");
        }
    } else {
        return BuildRejectedPreview(event, "event_type is outside the current MVP control_event contract.");
    }

    const InputInjectionPlan plan = PlanInputInjection(event);
    core::ControlHandlingPreview preview = {
        core::kControlEventType,
        event.session_id,
        event.event_type,
        event.sequence,
        "session/control_event -> control/control_message_handler -> control/input_injector",
        plan.accepted,
        "accepted",
        plan.status,
        plan.target,
        {
            "read control_event envelope",
            "validate message against the host-device MVP contract"
        },
        {}
    };

    preview.steps.insert(preview.steps.end(), plan.steps.begin(), plan.steps.end());
    preview.notes.insert(preview.notes.end(), plan.notes.begin(), plan.notes.end());
    if (!event.text.empty()) {
        preview.notes.push_back("text is reserved for later input work and is ignored by the current baseline.");
    }

    return preview;
}

}  // namespace control
}  // namespace hscrcpy
