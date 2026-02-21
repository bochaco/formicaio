use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt, str::FromStr};

// Length of node ids when generated in native mode.
pub(crate) const NODE_ID_LENGTH: usize = 12;
// Length of node ids when generated from Docker container IDs.
const DOCKER_CONTAINER_ID_LENGTH: usize = 64;
// Length of nodes id's prefix to be returned when requested a short version
const NODE_ID_PREFIX_LEN: usize = 12;

// Hex-encoded node id
#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Serialize)]
pub struct NodeId(String);

impl NodeId {
    pub fn new(id: impl AsRef<str>) -> Result<Self, String> {
        Self::from_str(id.as_ref())
    }

    fn validate(id: &str) -> Result<(), String> {
        let is_hex = id.as_bytes().iter().all(u8::is_ascii_hexdigit);
        let is_random_format = id.len() == NODE_ID_LENGTH;
        let is_docker_format = id.len() == DOCKER_CONTAINER_ID_LENGTH;

        if !is_hex || (!is_docker_format && !is_random_format) {
            return Err(format!(
                "Invalid NodeId string: must be {NODE_ID_LENGTH} hex chars (Formicaio native node ID) \
                 or {DOCKER_CONTAINER_ID_LENGTH} hex chars (Docker container ID)"
            ));
        }

        Ok(())
    }

    pub fn random() -> Self {
        // Generate a random string as node id
        let random_str = Alphanumeric.sample_string(&mut rand::rng(), NODE_ID_LENGTH / 2);
        let node_id = hex::encode(random_str);
        Self(node_id)
    }

    pub fn short_node_id(&self) -> String {
        if self.0.len() > NODE_ID_PREFIX_LEN {
            self.0[..NODE_ID_PREFIX_LEN].to_string()
        } else {
            self.0.clone()
        }
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::random()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for NodeId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::validate(s)?;
        Ok(Self(s.to_string()))
    }
}

impl TryFrom<&str> for NodeId {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
}

impl TryFrom<String> for NodeId {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        NodeId::from_str(&value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::NodeId;
    use std::str::FromStr;

    #[test]
    fn rejects_empty_node_id() {
        assert!(NodeId::from_str("").is_err());
        assert!(NodeId::new("").is_err());
    }

    #[test]
    fn rejects_invalid_length_random_id() {
        assert!(NodeId::from_str("12345678901").is_err());
        assert!(NodeId::new("12345678901").is_err());
        assert!(NodeId::new("1234567890123").is_err());
    }

    #[test]
    fn rejects_non_hex_random_format() {
        assert!(NodeId::from_str("zzzzzzzzzzzz").is_err());
        assert!(NodeId::new("zzzzzzzzzzzz").is_err());
    }

    #[test]
    fn accepts_random_format_node_id() {
        assert!(NodeId::from_str("616263646566").is_ok());
        assert!(NodeId::new("616263646566").is_ok());
    }

    #[test]
    fn rejects_invalid_length_docker_format() {
        assert!(
            NodeId::from_str("afb7790b2f65e3c99eb9982592f07045802ad376d0ca2e0482be50abac0d91e")
                .is_err()
        );
        assert!(
            NodeId::new("afb7790b2f65e3c99eb9982592f07045802ad376d0ca2e0482be50abac0d91e").is_err()
        );
        assert!(
            NodeId::new("aaafb7790b2f65e3c99eb9982592f07045802ad376d0ca2e0482be50abac0d91e")
                .is_err()
        );
    }

    #[test]
    fn rejects_non_hex_docker_format() {
        assert!(
            NodeId::from_str("QQfb7790b2f65e3c99eb9982592f07045802ad376d0ca2e0482be50abac0d91e")
                .is_err()
        );
        assert!(
            NodeId::new("QQfb7790b2f65e3c99eb9982592f07045802ad376d0ca2e0482be50abac0d91e")
                .is_err()
        );
    }

    #[test]
    fn accepts_docker_format_node_id() {
        assert!(
            NodeId::from_str("39fb7790b2f65e3c99eb9982592f07045802ad376d0ca2e0482be50abac0d91e")
                .is_ok()
        );
        assert!(
            NodeId::new("39fb7790b2f65e3c99eb9982592f07045802ad376d0ca2e0482be50abac0d91e").is_ok()
        );
    }
}
