//! Foundation host/device contracts.
//!
//! These types are intentionally small and provisional until
//! `04-22-hos-arch-contracts` publishes the stable architecture artifact.

mod capability;
mod channel;
mod session;

pub use capability::{CapabilityReport, VideoCodec};
pub use channel::{ChannelBinding, ChannelEndpoint, ChannelLayout, TransportKind};
pub use session::{
    AuthorizationState, AuthorizationUpdate, ControlEvent, ControlEventType, DeviceAction,
    DeviceHello, DisplayInfo, FeatureAuthorization, HostHello, HostVideoLimits,
    InboundSessionMessage, NormalizedPosition, NormalizedPositionError, PointerButton,
    SessionConfig, SessionControlConfig, SessionControlState, SessionError, SessionFeature,
    SessionMessage, SessionReady, SessionStartRequest, SessionStartResponse, SessionVideoConfig,
    StopSession, VideoCodecDescriptor, PROTOCOL_MAJOR_MVP, PROTOCOL_MINOR_MVP,
};
