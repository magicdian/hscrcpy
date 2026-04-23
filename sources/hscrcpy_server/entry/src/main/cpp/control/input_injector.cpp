#include "control/input_injector.h"

namespace hscrcpy {
namespace control {

namespace {

static constexpr const char *kInjectorBackend = "harmony_input_injector_scaffold";
static constexpr const char *kExecutionMode = "validated_scaffold";
static constexpr const char *kStatusPendingPlatformBinding = "pending_platform_binding";
static constexpr const char *kStatusUnsupportedInBaseline = "unsupported_in_baseline";
static constexpr const char *kTargetPointerSurface = "pointer_surface";
static constexpr const char *kTargetSystemNavigation = "system_navigation";
static constexpr const char *kTargetUnsupported = "unsupported";

bool IsPointerEventType(const std::string &event_type)
{
    return event_type == core::kControlEventPointerDown || event_type == core::kControlEventPointerMove ||
           event_type == core::kControlEventPointerUp;
}

}  // namespace

core::ControlBaselinePreview BuildControlBaselinePreview()
{
    return {
        core::kChannelNameSession,
        kInjectorBackend,
        kExecutionMode,
        {
            core::kControlEventPointerDown,
            core::kControlEventPointerMove,
            core::kControlEventPointerUp,
            core::kControlEventDeviceAction
        },
        {
            core::kControlEventScroll,
            core::kControlEventKeyDown,
            core::kControlEventKeyUp
        },
        {
            core::kDeviceActionBack,
            core::kDeviceActionHome
        },
        {
            "transport/session_channel",
            "control/control_message_handler",
            "control/input_injector"
        },
        {
            "control_event stays on the session channel and never enters the video path.",
            "Pointer and device-action events are validated and routed through a native injection scaffold.",
            "Scroll and keyboard events remain explicitly unsupported until HarmonyOS bindings land."
        }
    };
}

InputInjectionPlan PlanInputInjection(const core::ControlEventRequest &event)
{
    InputInjectionPlan plan;
    plan.accepted = false;
    plan.status = kStatusUnsupportedInBaseline;
    plan.target = kTargetUnsupported;
    plan.steps = {
        "resolve input injector backend",
        "check baseline support for the requested control_event"
    };

    if (IsPointerEventType(event.event_type)) {
        plan.accepted = true;
        plan.status = kStatusPendingPlatformBinding;
        plan.target = kTargetPointerSurface;
        plan.steps.push_back("map normalized coordinates onto the active device display");
        plan.steps.push_back("hand pointer intent to the HarmonyOS injector scaffold");
        plan.notes = {
            "The baseline accepts pointer events and keeps the coordinate mapping on-device.",
            "Replacing the scaffold with concrete HarmonyOS injection APIs is a follow-up task."
        };
        return plan;
    }

    if (event.event_type == core::kControlEventDeviceAction &&
        (event.device_action == core::kDeviceActionBack || event.device_action == core::kDeviceActionHome)) {
        plan.accepted = true;
        plan.status = kStatusPendingPlatformBinding;
        plan.target = kTargetSystemNavigation;
        plan.steps.push_back("map the host device_action to a device navigation intent");
        plan.steps.push_back("hand navigation intent to the HarmonyOS injector scaffold");
        plan.notes = {
            "The baseline currently recognizes back and home as the only device_action values.",
            "System-action execution still stops at the injector scaffold in this task."
        };
        return plan;
    }

    plan.notes = {
        "This control_event is contract-visible but not supported by the HarmonyOS baseline injector yet.",
        "Unsupported cases must fail explicitly instead of silently pretending control succeeded."
    };
    return plan;
}

}  // namespace control
}  // namespace hscrcpy
