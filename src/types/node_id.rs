use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

// Length of generated node ids, however we allow Ids to
// be created with longer length, e.g. when using Docker container Ids.
const NODE_ID_LENGTH: usize = 12;
// Length of nodes id's prefix to be returned when requested a short version
const NODE_ID_PREFIX_LEN: usize = 12;

// Hex-encoded node id
#[derive(Clone, Default, Debug, Deserialize, Eq, Hash, Ord, PartialOrd, PartialEq, Serialize)]
pub struct NodeId(String);

impl NodeId {
    pub fn new(id: String) -> Self {
        Self(id)
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

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for NodeId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}
