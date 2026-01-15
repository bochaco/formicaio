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

use std::{collections::HashSet, path::PathBuf};
use sysinfo::{DiskRefreshKind, Disks};

// Number of seconds before timing out an attempt to upgrade the node binary.
pub const UPGRADE_NODE_BIN_TIMEOUT_SECS: u64 = 8 * 60; // 8 mins

// Get the total and free space of only the mount points where nodes are storing data,
// i.e. ignore all other mount points which are not being used by nodes to store data.
async fn get_disks_usage(disks: &mut Disks, base_paths: HashSet<PathBuf>) -> (u64, u64) {
    disks.refresh_specifics(true, DiskRefreshKind::nothing().with_storage());
    let mut total_space = 0;
    let mut available_space = 0;
    for disk in disks.list().iter().filter(|d| {
        base_paths
            .iter()
            .find(|p| d.total_space() > 0 && p.starts_with(d.mount_point()))
            .is_some()
    }) {
        total_space += disk.total_space();
        available_space += disk.available_space();
    }

    (total_space, available_space)
}
