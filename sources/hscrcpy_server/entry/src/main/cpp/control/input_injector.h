#ifndef HSCRCPY_SERVER_CONTROL_INPUT_INJECTOR_H
#define HSCRCPY_SERVER_CONTROL_INPUT_INJECTOR_H

#include <string>
#include <vector>

#include "core/companion_contracts.h"

namespace hscrcpy {
namespace control {

struct InputInjectionPlan {
    bool accepted;
    std::string status;
    std::string target;
    std::vector<std::string> steps;
    std::vector<std::string> notes;
};

core::ControlBaselinePreview BuildControlBaselinePreview();
InputInjectionPlan PlanInputInjection(const core::ControlEventRequest &event);

}  // namespace control
}  // namespace hscrcpy

#endif
