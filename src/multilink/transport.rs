use serde::Deserializer;
use serde_derive::Deserialize;

/// The `Transport` enum is used to easy differentiate and compare protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    /// Represents the `UDP` protocol
    Udp,
    /// Represents the `TCP` protocol
    Tcp,
    /// Represents possible other protocols apart from `UDP` and `TCP`
    Unknown,
}

/// The `TransportInformation` struct represent a protocol flow consisting of the used transport protocol, the source and the destination port
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub struct TransportInformation {
    /// The `transport_protocol` holds information about the used transport protocol
    pub transport_protocol: Transport,
    /// The `dst_port` consists of an Option of type `u16` defining the destination port of the protocol flow. `None` when the destination port is dynamic.
    #[serde(deserialize_with = "deserialize_optional_port")]
    pub dst_port: Option<u16>,
    /// The `src_port` consists of an Option of type `u16` defining the source port of the protocol flow. `None` when the source port is dynamic.
    #[serde(deserialize_with = "deserialize_optional_port")]
    pub src_port: Option<u16>,
}

fn deserialize_optional_port<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::{de::Error, Deserialize};
    use serde_yaml::Value;

    let value: Value = Deserialize::deserialize(deserializer)?;
    match value {
        Value::String(s) => {
            if s == "*" {
                Ok(None)
            } else {
                Err(Error::custom("port can be either number or '*'"))
            }
        }
        Value::Number(port) => Ok(Some(port.as_u64().unwrap() as u16)),
        _ => Err(Error::custom("port can be either number or '*'")),
    }
}

impl TransportInformation {
    /// A basic compare method for two objects of type `TransportInformation`.
    /// Contrary to a fully equal compare it tests only for a matching transport protocol and one of the port types matching. Prioritizing the dst_port.
    pub fn compare(&self, other: &TransportInformation) -> bool {
        if self.transport_protocol != other.transport_protocol {
            return false;
        }
        if self.dst_port.is_some()
            && other.dst_port.is_some()
            && self.dst_port.unwrap() == other.dst_port.unwrap()
        {
            return true;
        }

        if self.src_port.is_some()
            && other.dst_port.is_some()
            && self.src_port.unwrap() == other.src_port.unwrap()
        {
            return true;
        }
        false
    }
}

pub fn parse_transport_header(packet: &[u8]) -> TransportInformation {
    match etherparse::PacketHeaders::from_ip_slice(packet) {
        Err(_) => TransportInformation {
            transport_protocol: Transport::Unknown,
            dst_port: None,
            src_port: None,
        },
        Ok(value) => match &value.transport {
            Some(etherparse::TransportHeader::Udp(header)) => TransportInformation {
                transport_protocol: Transport::Udp,
                dst_port: Some(header.destination_port),
                src_port: Some(header.source_port),
            },
            Some(etherparse::TransportHeader::Tcp(header)) => TransportInformation {
                transport_protocol: Transport::Tcp,
                dst_port: Some(header.destination_port),
                src_port: Some(header.source_port),
            },
            _ => TransportInformation {
                transport_protocol: Transport::Unknown,
                dst_port: None,
                src_port: None,
            },
        },
    }
}
