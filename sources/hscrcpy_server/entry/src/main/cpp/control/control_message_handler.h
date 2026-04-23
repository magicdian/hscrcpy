#ifndef HSCRCPY_SERVER_CONTROL_CONTROL_MESSAGE_HANDLER_H
#define HSCRCPY_SERVER_CONTROL_CONTROL_MESSAGE_HANDLER_H

#include "core/companion_contracts.h"

namespace hscrcpy {
namespace control {

core::ControlHandlingPreview PreviewControlHandling(const core::ControlEventRequest &event);

}  // namespace control
}  // namespace hscrcpy

#endif
