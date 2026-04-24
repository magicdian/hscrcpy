use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    NotImplemented(&'static str),
    CompanionUnavailable(String),
    HdcFailure(String),
    PayloadSelection(String),
    TransportFailure(String),
    RenderFailure(String),
    ContractViolation(String),
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented(scope) => {
                write!(f, "{scope} is not implemented in the foundation scaffold")
            }
            Self::CompanionUnavailable(reason) => write!(f, "companion unavailable: {reason}"),
            Self::HdcFailure(reason) => write!(f, "hdc failure: {reason}"),
            Self::PayloadSelection(reason) => write!(f, "payload selection failed: {reason}"),
            Self::TransportFailure(reason) => write!(f, "transport failure: {reason}"),
            Self::RenderFailure(reason) => write!(f, "render failure: {reason}"),
            Self::ContractViolation(reason) => write!(f, "contract violation: {reason}"),
        }
    }
}

impl std::error::Error for HostError {}

pub type HostResult<T> = Result<T, HostError>;
