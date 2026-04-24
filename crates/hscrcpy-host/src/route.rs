use std::error::Error;
use std::fmt;
use std::str::FromStr;

const ROUTE_UITEST: &str = "uitest";
const ROUTE_HSCRCPY_SERVER: &str = "hscrcpy-server";
const ROUTE_VALUES: [&str; 2] = [ROUTE_UITEST, ROUTE_HSCRCPY_SERVER];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRoute {
    Uitest,
    HscrcpyServer,
}

impl HostRoute {
    pub const fn as_cli_value(self) -> &'static str {
        match self {
            Self::Uitest => ROUTE_UITEST,
            Self::HscrcpyServer => ROUTE_HSCRCPY_SERVER,
        }
    }

    pub const fn cli_values() -> &'static [&'static str] {
        &ROUTE_VALUES
    }
}

impl Default for HostRoute {
    fn default() -> Self {
        Self::Uitest
    }
}

impl fmt::Display for HostRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_cli_value())
    }
}

impl FromStr for HostRoute {
    type Err = HostRouteParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim() {
            ROUTE_UITEST => Ok(Self::Uitest),
            ROUTE_HSCRCPY_SERVER => Ok(Self::HscrcpyServer),
            _ => Err(HostRouteParseError {
                raw: raw.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRouteParseError {
    raw: String,
}

impl fmt::Display for HostRouteParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsupported route `{}`; expected one of: {}",
            self.raw,
            HostRoute::cli_values().join(", ")
        )
    }
}

impl Error for HostRouteParseError {}

#[cfg(test)]
mod tests {
    use super::HostRoute;
    use std::str::FromStr;

    #[test]
    fn default_route_is_uitest() {
        assert_eq!(HostRoute::default(), HostRoute::Uitest);
    }

    #[test]
    fn parses_supported_route_values() {
        assert_eq!(
            HostRoute::from_str("hscrcpy-server").expect("route should parse"),
            HostRoute::HscrcpyServer
        );
        assert_eq!(
            HostRoute::from_str("uitest").expect("route should parse"),
            HostRoute::Uitest
        );
    }

    #[test]
    fn rejects_unsupported_route_values() {
        let err = HostRoute::from_str("grpc").expect_err("invalid route must fail");
        assert!(err.to_string().contains("hscrcpy-server"));
        assert!(err.to_string().contains("uitest"));
    }
}
