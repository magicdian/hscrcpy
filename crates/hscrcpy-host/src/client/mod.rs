use crate::{HostError, HostResult};

pub trait ClientEntryPoint {
    fn run(&self) -> HostResult<()>;
}

pub struct StubClientEntry;

impl ClientEntryPoint for StubClientEntry {
    fn run(&self) -> HostResult<()> {
        Err(HostError::NotImplemented("client entrypoint"))
    }
}
