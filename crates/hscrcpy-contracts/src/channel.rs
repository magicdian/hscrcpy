#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportKind {
    HdcForward,
    Tcp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelEndpoint {
    pub transport: TransportKind,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelBinding {
    pub name: String,
    pub state: String,
    pub payload_type: String,
    pub activation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelLayout {
    pub session: ChannelBinding,
    pub video: ChannelBinding,
}
