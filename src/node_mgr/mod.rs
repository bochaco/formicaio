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
    let indexed_disks = disks
        .list()
        .iter()
        .enumerate()
        .filter(|(_, d)| d.total_space() > 0)
        .collect::<Vec<_>>();

    let mut total_space = 0;
    let mut available_space = 0;
    let mut disks_matched = std::collections::HashSet::<usize>::new();

    for base_path in base_paths.into_iter() {
        let mut best_match: Option<(&sysinfo::Disk, usize, usize)> = None;
        let canonical_path = if let Ok(p) = base_path.canonicalize() {
            p
        } else {
            base_path
        };

        for (index, disk) in indexed_disks.iter() {
            let mount_point = disk.mount_point();
            if canonical_path.starts_with(mount_point) {
                let match_len = mount_point.as_os_str().len();
                if best_match.is_none_or(|(_, len, _)| match_len > len) {
                    best_match = Some((disk, match_len, *index));
                }
            }
        }

        if let Some((disk, _, index)) = best_match {
            if disks_matched.insert(index) {
                total_space += disk.total_space();
                available_space += disk.available_space();
            }
        };
    }

    (total_space, available_space)
}
