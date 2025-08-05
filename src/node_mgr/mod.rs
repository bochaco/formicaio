#[cfg(not(feature = "native"))]
mod docker;
#[cfg(not(feature = "native"))]
mod docker_client;
#[cfg(not(feature = "native"))]
mod docker_msgs;
#[cfg(feature = "native")]
mod native;
#[cfg(feature = "native")]
mod native_nodes;

#[cfg(not(feature = "native"))]
pub use docker::NodeManager;
#[cfg(feature = "native")]
pub use native::NodeManager;

// Number of seconds before timing out an attempt to upgrade the node binary.
pub const UPGRADE_NODE_BIN_TIMEOUT_SECS: u64 = 8 * 60; // 8 mins
