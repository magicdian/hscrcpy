use crate::{HostError, HostResult};
use hscrcpy_contracts::{
    ControlEvent, DeviceAction, NormalizedPosition, PointerButton, SessionMessage,
};

pub trait SessionMessageSink {
    fn emit(&mut self, message: SessionMessage) -> HostResult<()>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutboundControlEvent {
    PointerDown {
        position_norm: NormalizedPosition,
        pointer_id: u32,
        button: PointerButton,
    },
    PointerMove {
        position_norm: NormalizedPosition,
        pointer_id: u32,
    },
    PointerUp {
        position_norm: NormalizedPosition,
        pointer_id: u32,
        button: PointerButton,
    },
    Scroll {
        position_norm: NormalizedPosition,
        scroll_delta_x: i32,
        scroll_delta_y: i32,
    },
    KeyDown {
        key_code: String,
    },
    KeyUp {
        key_code: String,
    },
    DeviceAction {
        action: DeviceAction,
    },
}

impl OutboundControlEvent {
    fn to_contract_event(self, session_id: &str, sequence: u64) -> ControlEvent {
        match self {
            Self::PointerDown {
                position_norm,
                pointer_id,
                button,
            } => ControlEvent::pointer_down(
                session_id.to_string(),
                sequence,
                position_norm,
                pointer_id,
                button,
            ),
            Self::PointerMove {
                position_norm,
                pointer_id,
            } => ControlEvent::pointer_move(
                session_id.to_string(),
                sequence,
                position_norm,
                pointer_id,
            ),
            Self::PointerUp {
                position_norm,
                pointer_id,
                button,
            } => ControlEvent::pointer_up(
                session_id.to_string(),
                sequence,
                position_norm,
                pointer_id,
                button,
            ),
            Self::Scroll {
                position_norm,
                scroll_delta_x,
                scroll_delta_y,
            } => ControlEvent::scroll(
                session_id.to_string(),
                sequence,
                position_norm,
                scroll_delta_x,
                scroll_delta_y,
            ),
            Self::KeyDown { key_code } => {
                ControlEvent::key_down(session_id.to_string(), sequence, key_code)
            }
            Self::KeyUp { key_code } => {
                ControlEvent::key_up(session_id.to_string(), sequence, key_code)
            }
            Self::DeviceAction { action } => {
                ControlEvent::device_action(session_id.to_string(), sequence, action)
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct BufferedSessionMessageSink {
    messages: Vec<SessionMessage>,
}

impl BufferedSessionMessageSink {
    pub fn messages(&self) -> &[SessionMessage] {
        &self.messages
    }

    pub fn into_messages(self) -> Vec<SessionMessage> {
        self.messages
    }
}

impl SessionMessageSink for BufferedSessionMessageSink {
    fn emit(&mut self, message: SessionMessage) -> HostResult<()> {
        self.messages.push(message);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ControlMessageEmitter {
    session_id: String,
    next_sequence: u64,
}

impl ControlMessageEmitter {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            next_sequence: 1,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn emit<S: SessionMessageSink>(
        &mut self,
        sink: &mut S,
        event: OutboundControlEvent,
    ) -> HostResult<ControlEvent> {
        let sequence = self.next_sequence;
        let control_event = event.to_contract_event(&self.session_id, sequence);
        sink.emit(SessionMessage::ControlEvent(control_event.clone()))?;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| HostError::ContractViolation("control sequence overflow".to_string()))?;
        Ok(control_event)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BufferedSessionMessageSink, ControlMessageEmitter, OutboundControlEvent, SessionMessageSink,
    };
    use crate::{HostError, HostResult};
    use hscrcpy_contracts::{ControlEventType, DeviceAction, NormalizedPosition, SessionMessage};

    #[test]
    fn emits_control_events_with_monotonic_sequence() {
        let mut sink = BufferedSessionMessageSink::default();
        let mut emitter = ControlMessageEmitter::new("session-1");

        let down = emitter
            .emit(
                &mut sink,
                OutboundControlEvent::PointerDown {
                    position_norm: NormalizedPosition::new(0.4, 0.5).expect("valid position"),
                    pointer_id: 0,
                    button: hscrcpy_contracts::PointerButton::Primary,
                },
            )
            .expect("pointer down should be emitted");
        let key = emitter
            .emit(
                &mut sink,
                OutboundControlEvent::KeyDown {
                    key_code: "KEYCODE_BACK".to_string(),
                },
            )
            .expect("key down should be emitted");

        assert_eq!(down.sequence, 1);
        assert_eq!(down.event_type, ControlEventType::PointerDown);
        assert_eq!(key.sequence, 2);
        assert_eq!(key.event_type, ControlEventType::KeyDown);
        assert_eq!(emitter.next_sequence(), 3);
        assert_eq!(sink.messages().len(), 2);
    }

    #[test]
    fn preserves_sequence_when_sink_rejects_event() {
        struct FailingSink;

        impl SessionMessageSink for FailingSink {
            fn emit(&mut self, _message: SessionMessage) -> HostResult<()> {
                Err(HostError::HdcFailure("session pipe closed".to_string()))
            }
        }

        let mut sink = FailingSink;
        let mut emitter = ControlMessageEmitter::new("session-2");

        let err = emitter
            .emit(
                &mut sink,
                OutboundControlEvent::DeviceAction {
                    action: DeviceAction::Back,
                },
            )
            .expect_err("sink failure should propagate");
        assert!(
            err.to_string().contains("session pipe closed"),
            "unexpected error: {err}"
        );
        assert_eq!(emitter.next_sequence(), 1);
    }
}
