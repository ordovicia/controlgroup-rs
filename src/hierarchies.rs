//! This module represents the various control group hierarchies the Linux kernel supports.
//!
//! Currently, we only support the cgroupv1 hierarchy, but in the future we will add support for
//! the Unified Hierarchy.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::{
    cgroup::Cgroup,
    cpu::CpuController,
    {Controllers, Hierarchy, Subsystem},
};

/// The standard, original cgroup implementation. Often referred to as "cgroupv1".
pub struct V1 {
    mount_point: String,
}

impl Hierarchy for V1 {
    fn subsystems(&self) -> Vec<Subsystem> {
        let mut subs = vec![];
        if self.check_support(Controllers::Cpu) {
            subs.push(Subsystem::Cpu(CpuController::new(self.root())));
        }

        subs
    }

    fn root_control_group(&self) -> Cgroup<'_> {
        Cgroup::load(self, "".to_string())
    }

    fn check_support(&self, sub: Controllers) -> bool {
        let root = self.root().read_dir().unwrap();
        for entry in root {
            if let Ok(entry) = entry {
                if entry.file_name().into_string().unwrap() == sub.to_string() {
                    return true;
                }
            }
        }
        return false;
    }

    fn root(&self) -> PathBuf {
        PathBuf::from(self.mount_point.clone())
    }
}

impl V1 {
    /// Finds where control groups are mounted to and returns a hierarchy in which control groups
    /// can be created.
    pub fn new() -> Self {
        let mount_point = find_v1_mount().unwrap();
        V1 {
            mount_point: mount_point,
        }
    }
}

fn find_v1_mount() -> Option<String> {
    // Open mountinfo so we can get a parseable mount list
    let mountinfo_path = Path::new("/proc/self/mountinfo");

    // If /proc isn't mounted, or something else happens, then bail out
    if mountinfo_path.exists() == false {
        return None;
    }

    let mountinfo_file = File::open(mountinfo_path).unwrap();
    let mountinfo_reader = BufReader::new(&mountinfo_file);
    for _line in mountinfo_reader.lines() {
        let line = _line.unwrap();
        let mut fields = line.split_whitespace();
        let index = line.find(" - ").unwrap();
        let more_fields = line[index + 3..].split_whitespace().collect::<Vec<_>>();
        let fstype = more_fields[0];
        if fstype == "tmpfs" && more_fields[2].contains("ro") {
            let cgroups_mount = fields.nth(4).unwrap();
            log::info!("found cgroups at {:?}", cgroups_mount);
            return Some(cgroups_mount.to_string());
        }
    }

    None
}
