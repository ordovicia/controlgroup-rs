//! Operations on a Memory subsystem.
//!
//! [`Subsystem`] implements [`Cgroup`] trait and subsystem-specific operations.
//!
//! For more information about this subsystem, see the kernel's documentation
//! [Documentation/cgroup-v1/memory.txt].
//!
//! # Examples
//!
//! ```no_run
//! # fn main() -> controlgroup::Result<()> {
//! use std::path::PathBuf;
//! use controlgroup::{Pid, v1::{self, memory, Cgroup, CgroupPath, SubsystemKind}};
//!
//! let mut mem_cgroup = memory::Subsystem::new(
//!     CgroupPath::new(SubsystemKind::Memory, PathBuf::from("students/charlie")));
//! mem_cgroup.create()?;
//!
//! // Define a resource limit about what amount and how a cgroup can use memory.
//! const GB: i64 = 1 << 30;
//! let resources = memory::Resources {
//!     limit_in_bytes: Some(4 * GB),
//!     soft_limit_in_bytes: Some(3 * GB),
//!     use_hierarchy: Some(true),
//!     ..memory::Resources::default()
//! };
//!
//! // Apply the resource limit to this cgroup.
//! mem_cgroup.apply(&resources.into())?;
//!
//! // Add tasks to this cgroup.
//! let pid = Pid::from(std::process::id());
//! mem_cgroup.add_task(pid)?;
//!
//! // Do something ...
//!
//! // Get the statistics about the memory usage of this cgroup.
//! println!("{:?}", mem_cgroup.stat()?);
//!
//! mem_cgroup.remove_task(pid)?;
//! mem_cgroup.delete()?;
//! # Ok(())
//! # }
//! ```
//!
//! [`Subsystem`]: struct.Subsystem.html
//! [`Cgroup`]: ../trait.Cgroup.html
//!
//! [Documentation/cgroup-v1/memory.txt]: https://www.kernel.org/doc/Documentation/cgroup-v1/memory.txt

use std::{
    io::{self, BufRead},
    path::PathBuf,
};

use crate::{
    parse::{parse, parse_01_bool, parse_next},
    v1::{self, cgroup::CgroupHelper, Cgroup, CgroupPath},
    Error, ErrorKind, Result,
};

/// Handler of a Memory subsystem.
#[derive(Debug)]
pub struct Subsystem {
    path: CgroupPath,
}

/// Resource limit on what amount and how a cgroup can use memory.
///
/// See the kernel's documentation for more information about the fields.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Resources {
    /// Limit the memory usage of this cgroup. Setting -1 removes the current limit.
    pub limit_in_bytes: Option<i64>,
    /// Limit the total of memory and swap usage by this cgroup. Setting -1 removes the current
    /// limit.
    pub memsw_limit_in_bytes: Option<i64>,
    /// Limit the usage of kernel memory by this cgroup. Setting -1 removes the current limit.
    pub kmem_limit_in_bytes: Option<i64>,
    /// Limit the usage of kernel memory for TCP by this cgroup. Setting -1 removes the current
    /// limit.
    pub kmem_tcp_limit_in_bytes: Option<i64>,
    /// Soft limit on memory usage of this cgroup. Setting -1 removes the current limit.
    pub soft_limit_in_bytes: Option<i64>,
    /// Kernel's tendency to swap out pages consumed by this cgroup.
    pub swappiness: Option<u64>,
    /// Whether pages may be recharged to the new cgroup when a task is moved.
    pub move_charge_at_immigrate: Option<bool>,
    /// Whether the OOM killer tries to reclaim memory from the self and descendant cgroups.
    pub use_hierarchy: Option<bool>,
}

/// Statistics of memory usage of a cgroup.
///
/// Some fields only present on some systems, so these fields are `Option`.
///
/// See the kernel's documentation for more information about the fields.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct Stat {
    pub cache: u64,
    pub rss: u64,
    pub rss_huge: u64,
    pub shmem: u64,
    pub mapped_file: u64,
    pub dirty: u64,
    pub writeback: u64,
    pub swap: Option<u64>,
    pub pgpgin: u64,
    pub pgpgout: u64,
    pub pgfault: u64,
    pub pgmajfault: u64,
    pub active_anon: u64,
    pub inactive_anon: u64,
    pub active_file: u64,
    pub inactive_file: u64,
    pub unevictable: u64,
    pub hierarchical_memory_limit: u64,
    pub hierarchical_memsw_limit: Option<u64>,

    pub total_cache: u64,
    pub total_rss: u64,
    pub total_rss_huge: u64,
    pub total_shmem: u64,
    pub total_mapped_file: u64,
    pub total_dirty: u64,
    pub total_writeback: u64,
    pub total_swap: Option<u64>,
    pub total_pgpgin: u64,
    pub total_pgpgout: u64,
    pub total_pgfault: u64,
    pub total_pgmajfault: u64,
    pub total_active_anon: u64,
    pub total_inactive_anon: u64,
    pub total_active_file: u64,
    pub total_inactive_file: u64,
    pub total_unevictable: u64,
}

/// Statistics of memory usage per NUMA node.
///
/// The first element of each pair is the system-wide value, and the second is per-node values.
///
/// See the kernel's documentation for more information about the fields.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct NumaStat {
    pub total: (u64, Vec<u64>),
    pub file: (u64, Vec<u64>),
    pub anon: (u64, Vec<u64>),
    pub unevictable: (u64, Vec<u64>),

    pub hierarchical_total: (u64, Vec<u64>),
    pub hierarchical_file: (u64, Vec<u64>),
    pub hierarchical_anon: (u64, Vec<u64>),
    pub hierarchical_unevictable: (u64, Vec<u64>),
}

/// OOM status and controls.
///
/// See the kernel's documentation for more information about the fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OomControl {
    /// Whether the OOM killer is disabled for this cgroup.
    pub oom_kill_disable: bool,
    /// Whether this cgroup is currently suspended (not killed) because OOM killer is disabled.
    pub under_oom: bool,
    /// Number of times tasks were killed by the OOM killer so far.
    pub oom_kill: Option<u64>,
}

impl_cgroup! {
    Subsystem, Memory,

    /// Applies the `Some` fields in `resources.memory`. `limit_in_bytes` field is set before
    /// `memsw_limit_in_bytes` is.
    fn apply(&mut self, resources: &v1::Resources) -> Result<()> {
        macro_rules! a {
            ($field: ident, $setter: ident) => {
                if let Some(r) = resources.memory.$field {
                    self.$setter(r)?;
                }
            };
        }

        a!(limit_in_bytes, set_limit_in_bytes);
        a!(memsw_limit_in_bytes, set_memsw_limit_in_bytes);
        a!(kmem_limit_in_bytes, set_kmem_limit_in_bytes);
        a!(kmem_tcp_limit_in_bytes, set_kmem_tcp_limit_in_bytes);
        a!(soft_limit_in_bytes, set_soft_limit_in_bytes);
        a!(swappiness, set_swappiness);
        a!(move_charge_at_immigrate, set_move_charge_at_immigrate);
        a!(use_hierarchy, set_use_hierarchy);

        Ok(())
    }
}

macro_rules! def_file {
    ($var: ident, $name: literal) => {
        const $var: &str = concat!("memory.", $name);
    };
}

def_file!(STAT, "stat");
def_file!(NUMA_STAT, "numa_stat");

def_file!(USAGE_IN_BYTES, "usage_in_bytes");
def_file!(MEMSW_USAGE_IN_BYTES, "memsw.usage_in_bytes");
def_file!(KMEM_USAGE_IN_BYTES, "kmem.usage_in_bytes");
def_file!(KMEM_TCP_USAGE_IN_BYTES, "kmem.tcp.usage_in_bytes");

def_file!(MAX_USAGE_IN_BYTES, "max_usage_in_bytes");
def_file!(MEMSW_MAX_USAGE_IN_BYTES, "memsw.max_usage_in_bytes");
def_file!(KMEM_MAX_USAGE_IN_BYTES, "kmem.max_usage_in_bytes");
def_file!(KMEM_TCP_MAX_USAGE_IN_BYTES, "kmem.tcp.max_usage_in_bytes");

def_file!(LIMIT_IN_BYTES, "limit_in_bytes");
def_file!(MEMSW_LIMIT_IN_BYTES, "memsw.limit_in_bytes");
def_file!(KMEM_LIMIT_IN_BYTES, "kmem.limit_in_bytes");
def_file!(KMEM_TCP_LIMIT_IN_BYTES, "kmem.tcp.limit_in_bytes");

def_file!(SOFT_LIMIT_IN_BYTES, "soft_limit_in_bytes");

def_file!(FAILCNT, "failcnt");
def_file!(MEMSW_FAILCNT, "memsw.failcnt");
def_file!(KMEM_FAILCNT, "kmem.failcnt");
def_file!(KMEM_TCP_FAILCNT, "kmem.tcp.failcnt");

def_file!(SWAPPINESS, "swappiness");
def_file!(OOM_CONTROL, "oom_control");
def_file!(MOVE_CHARGE_AT_IMMIGRATE, "move_charge_at_immigrate");
def_file!(USE_HIERARCHY, "use_hierarchy");
def_file!(FORCE_EMPTY, "force_empty");

impl Subsystem {
    /// Reads the statistics of memory usage of this cgroup from `memory.stat` file.
    pub fn stat(&self) -> Result<Stat> {
        self.open_file_read(STAT).and_then(parse_stat)
    }

    /// Reads the statistics of memory usage per NUMA node of this cgroup, from `memory.num_stat`
    /// file.
    pub fn numa_stat(&self) -> Result<NumaStat> {
        self.open_file_read(NUMA_STAT).and_then(parse_numa_stat)
    }

    /// Reads the memory usage of this cgroup from `memory.usage_in_bytes` file.
    pub fn usage_in_bytes(&self) -> Result<u64> {
        self.open_file_read(USAGE_IN_BYTES).and_then(parse)
    }

    /// Reads from `memory.memsw.usage_in_bytes` file. See [`usage_in_bytes`] method for more
    /// information.
    ///
    /// [`usage_in_bytes`]: #method.usage_in_bytes
    pub fn memsw_usage_in_bytes(&self) -> Result<u64> {
        self.open_file_read(MEMSW_USAGE_IN_BYTES).and_then(parse)
    }

    /// Reads from `memory.kmem.usage_in_bytes` file. See [`usage_in_bytes`] method for more
    /// information.
    ///
    /// [`usage_in_bytes`]: #method.usage_in_bytes
    pub fn kmem_usage_in_bytes(&self) -> Result<u64> {
        self.open_file_read(KMEM_USAGE_IN_BYTES).and_then(parse)
    }

    /// Reads from `memory.kmem.tcp.usage_in_bytes` file. See [`usage_in_bytes`] method for more
    /// information.
    ///
    /// [`usage_in_bytes`]: #method.usage_in_bytes
    pub fn kmem_tcp_usage_in_bytes(&self) -> Result<u64> {
        self.open_file_read(KMEM_TCP_USAGE_IN_BYTES).and_then(parse)
    }

    /// Reads the maximum memory usage of this cgroup from `memory.max_usage_in_bytes` file.
    pub fn max_usage_in_bytes(&self) -> Result<u64> {
        self.open_file_read(MAX_USAGE_IN_BYTES).and_then(parse)
    }

    /// Reads from `memory.memsw.max_usage_in_bytes` file. See [`max_usage_in_bytes`] method for
    /// more information.
    ///
    /// [`max_usage_in_bytes`]: #method.max_usage_in_bytes
    pub fn memsw_max_usage_in_bytes(&self) -> Result<u64> {
        self.open_file_read(MEMSW_MAX_USAGE_IN_BYTES)
            .and_then(parse)
    }

    /// Reads from `memory.kmem.max_usage_in_bytes` file. See [`max_usage_in_bytes`] method for
    /// more information.
    ///
    /// [`max_usage_in_bytes`]: #method.max_usage_in_bytes
    pub fn kmem_max_usage_in_bytes(&self) -> Result<u64> {
        self.open_file_read(KMEM_MAX_USAGE_IN_BYTES).and_then(parse)
    }

    /// Reads from `memory.kmem.tcp.max_usage_in_bytes` file. See [`max_usage_in_bytes`] method for
    /// more information.
    ///
    /// [`max_usage_in_bytes`]: #method.max_usage_in_bytes
    pub fn kmem_tcp_max_usage_in_bytes(&self) -> Result<u64> {
        self.open_file_read(KMEM_TCP_MAX_USAGE_IN_BYTES)
            .and_then(parse)
    }

    /// Reads the limit on memory usage (including file cache) of this cgroup from `memory.limit_in_bytes` file.
    pub fn limit_in_bytes(&self) -> Result<u64> {
        self.open_file_read(LIMIT_IN_BYTES).and_then(parse)
    }

    /// Reads from `memory.memsw.limit_in_bytes` file. See [`limit_in_bytes`] method for more
    /// information.
    ///
    /// [`limit_in_bytes`]: #method.limit_in_bytes
    pub fn memsw_limit_in_bytes(&self) -> Result<u64> {
        self.open_file_read(MEMSW_LIMIT_IN_BYTES).and_then(parse)
    }

    /// Reads from `memory.kmem.limit_in_bytes` file. See [`limit_in_bytes`] method for more
    /// information.
    ///
    /// [`limit_in_bytes`]: #method.limit_in_bytes
    pub fn kmem_limit_in_bytes(&self) -> Result<u64> {
        self.open_file_read(KMEM_LIMIT_IN_BYTES).and_then(parse)
    }

    /// Reads from `memory.kmem.tcp.limit_in_bytes` file. See [`limit_in_bytes`] method for more
    /// information.
    ///
    /// [`limit_in_bytes`]: #method.limit_in_bytes
    pub fn kmem_tcp_limit_in_bytes(&self) -> Result<u64> {
        self.open_file_read(KMEM_TCP_LIMIT_IN_BYTES).and_then(parse)
    }

    /// Sets a limit on memory usage of this group by writing to `memory.limit_in_bytes` file.
    /// Setting -1 removes the current limit.
    ///
    /// # Errors
    ///
    /// This field is configurable only for non-root cgroups. If you call this method on the root
    /// cgroup, an error is returned with kind [`ErrorKind::InvalidOperation`].
    ///
    /// [`ErrorKind::InvalidOperation`]: ../../enum.ErrorKind.html#variant.InvalidOperation
    pub fn set_limit_in_bytes(&mut self, limit: i64) -> Result<()> {
        if self.is_root() {
            return Err(Error::new(ErrorKind::InvalidOperation));
        }
        self.write_file(LIMIT_IN_BYTES, limit)
    }

    /// Writes to `memory.memsw.limit_in_bytes` file. See [`set_limit_in_bytes`] method for more
    /// information.
    ///
    /// [`set_limit_in_bytes`]: #method.set_limit_in_bytes
    ///
    /// # Errors
    ///
    /// This field is configurable only for non-root cgroups. If you call this method on the root
    /// cgroup, an error is returned with kind [`ErrorKind::InvalidOperation`].
    ///
    /// [`ErrorKind::InvalidOperation`]: ../../enum.ErrorKind.html#variant.InvalidOperation
    pub fn set_memsw_limit_in_bytes(&mut self, limit: i64) -> Result<()> {
        if self.is_root() {
            return Err(Error::new(ErrorKind::InvalidOperation));
        }
        self.write_file(MEMSW_LIMIT_IN_BYTES, limit)
    }

    /// Writes to `memory.kmem.limit_in_bytes` file. See [`set_limit_in_bytes`] method for more
    /// information.
    ///
    /// [`set_limit_in_bytes`]: #method.set_limit_in_bytes
    ///
    /// # Errors
    ///
    /// This field is configurable only for non-root cgroups. If you call this method on the root
    /// cgroup, an error is returned with kind [`ErrorKind::InvalidOperation`].
    ///
    /// [`ErrorKind::InvalidOperation`]: ../../enum.ErrorKind.html#variant.InvalidOperation
    pub fn set_kmem_limit_in_bytes(&mut self, limit: i64) -> Result<()> {
        if self.is_root() {
            return Err(Error::new(ErrorKind::InvalidOperation));
        }
        self.write_file(KMEM_LIMIT_IN_BYTES, limit)
    }

    /// Writes to `memory.kmem.tcp.limit_in_bytes` file. See [`set_limit_in_bytes`] method for more
    /// information.
    ///
    /// [`set_limit_in_bytes`]: #method.set_limit_in_bytes
    ///
    /// # Errors
    ///
    /// This field is configurable only for non-root cgroups. If you call this method on the root
    /// cgroup, an error is returned with kind [`ErrorKind::InvalidOperation`].
    ///
    /// [`ErrorKind::InvalidOperation`]: ../../enum.ErrorKind.html#variant.InvalidOperation
    pub fn set_kmem_tcp_limit_in_bytes(&mut self, limit: i64) -> Result<()> {
        if self.is_root() {
            return Err(Error::new(ErrorKind::InvalidOperation));
        }
        self.write_file(KMEM_TCP_LIMIT_IN_BYTES, limit)
    }

    /// Reads the soft limit on memory usage of this cgroup from `memory.soft_limit_in_bytes` file.
    pub fn soft_limit_in_bytes(&self) -> Result<u64> {
        self.open_file_read(SOFT_LIMIT_IN_BYTES).and_then(parse)
    }

    /// Sets a soft limit on memory usage of this cgroup by writing to `memory.soft_limit_in_bytes`
    /// file. Setting -1 removes the current limit.
    ///
    /// # Errors
    ///
    /// This field is configurable only for non-root cgroups. If you call this method on the root
    /// cgroup, an error is returned with kind [`ErrorKind::InvalidOperation`].
    ///
    /// [`ErrorKind::InvalidOperation`]: ../../enum.ErrorKind.html#variant.InvalidOperation
    pub fn set_soft_limit_in_bytes(&mut self, limit: i64) -> Result<()> {
        if self.is_root() {
            return Err(Error::new(ErrorKind::InvalidOperation));
        }
        self.write_file(SOFT_LIMIT_IN_BYTES, limit)
    }

    /// Reads the number of memory allocation failure due to the limit from `memory.failcnt` file.
    pub fn failcnt(&self) -> Result<u64> {
        self.open_file_read(FAILCNT).and_then(parse)
    }

    /// Reads `memory.memsw.failcnt` file. See [`failcnt`] method for more information.
    ///
    /// [`failcnt`]: #method.failcnt
    pub fn memsw_failcnt(&self) -> Result<u64> {
        self.open_file_read(MEMSW_FAILCNT).and_then(parse)
    }

    /// Reads `memory.kmem.failcnt` file. See [`failcnt`] method for more information.
    ///
    /// [`failcnt`]: #method.failcnt
    pub fn kmem_failcnt(&self) -> Result<u64> {
        self.open_file_read(KMEM_FAILCNT).and_then(parse)
    }

    /// Reads `memory.kmem.tcp.failcnt` file. See [`failcnt`] method for more information.
    ///
    /// [`failcnt`]: #method.failcnt
    pub fn kmem_tcp_failcnt(&self) -> Result<u64> {
        self.open_file_read(KMEM_TCP_FAILCNT).and_then(parse)
    }

    /// Reads the tendency of the kernel to swap out pages consumed by this cgroup, from
    /// `memory.swappiness` file.
    pub fn swappiness(&self) -> Result<u64> {
        self.open_file_read(SWAPPINESS).and_then(parse)
    }

    /// Sets a tendency of the kernel to swap out pages consumed by this cgroup, by writing to
    /// `memory.swappiness` file.
    pub fn set_swappiness(&mut self, swappiness: u64) -> Result<()> {
        self.write_file(SWAPPINESS, swappiness)
    }

    /// Reads the status of OOM killer on this cgroup from `memory.oom_control` file.
    pub fn oom_control(&self) -> Result<OomControl> {
        self.open_file_read(OOM_CONTROL).and_then(parse_oom_control)
    }

    /// Sets whether the OOM killer is disabled for this cgroup, by writing to `memory.oom_control`
    /// file.
    pub fn disable_oom_killer(&mut self, disable: bool) -> Result<()> {
        self.write_file(OOM_CONTROL, disable as i32)
    }

    /// Reads whether pages may be recharged to the new cgroup when a task is moved, from
    /// `memory.move_charge_at_immigrate` file.
    pub fn move_charge_at_immigrate(&self) -> Result<bool> {
        self.open_file_read(MOVE_CHARGE_AT_IMMIGRATE)
            .and_then(parse_01_bool)
    }

    /// Sets whether pages may be recharged to the new cgroup when a task is moved, by writing to
    /// `memory.move_charge_at_immigrate` file.
    pub fn set_move_charge_at_immigrate(&mut self, move_: bool) -> Result<()> {
        self.write_file(MOVE_CHARGE_AT_IMMIGRATE, move_ as i32)
    }

    /// Reads whether the OOM killer tries to reclaim memory from the self and descendant cgroups,
    /// from `memory.use_hierarchy` file.
    pub fn use_hierarchy(&self) -> Result<bool> {
        self.open_file_read(USE_HIERARCHY).and_then(parse_01_bool)
    }

    /// Sets whether the OOM killer tries to reclaim memory from the self and descendant cgroups, by
    /// writing to `memory.use_hierarchy` file.
    pub fn set_use_hierarchy(&mut self, use_: bool) -> Result<()> {
        self.write_file(USE_HIERARCHY, use_ as i32)
    }

    /// Makes this cgroup's memory usage empty, by writing to `memory.force_empty` file.
    pub fn force_empty(&mut self) -> Result<()> {
        self.write_file(FORCE_EMPTY, 0)
    }

    // kmem.slabinfo
}

impl Into<v1::Resources> for Resources {
    fn into(self) -> v1::Resources {
        v1::Resources {
            memory: self,
            ..v1::Resources::default()
        }
    }
}

fn parse_stat(reader: impl io::Read) -> Result<Stat> {
    #![allow(clippy::unnecessary_unwrap)]

    let buf = io::BufReader::new(reader);

    macro_rules! g {
        ([ $( $key: ident ),* ], [ $( $key_opt: ident ),* ]) => {
            $( let mut $key: Option<u64> = None; )*
            $( let mut $key_opt: Option<u64> = None; )*

            for line in buf.lines() {
                let line = line?;
                let mut entry = line.split_whitespace();

                match entry.next() {
                    $(
                        Some(stringify!($key)) => {
                            if $key.is_some() { bail_parse!(); }
                            $key = Some(parse_next(&mut entry)?);
                        }
                    )*
                    $(
                        Some(stringify!($key_opt)) => {
                            if $key_opt.is_some() { bail_parse!(); }
                            $key_opt = Some(parse_next(&mut entry)?);
                        }
                    )*
                    _ => { bail_parse!(); }
                }

                if entry.next().is_some() { bail_parse!(); }
            }

            if $( $key.is_some() &&)* true {
                Ok(Stat {
                    $( $key: $key.unwrap(), )*
                    $( $key_opt, )*
                })
            } else {
                bail_parse!();
            }
        }
    }

    g! {
        [
            cache,
            rss,
            rss_huge,
            shmem,
            mapped_file,
            dirty,
            writeback,
            pgpgin,
            pgpgout,
            pgfault,
            pgmajfault,
            active_anon,
            inactive_anon,
            active_file,
            inactive_file,
            unevictable,
            hierarchical_memory_limit,
            total_cache,
            total_rss,
            total_rss_huge,
            total_shmem,
            total_mapped_file,
            total_dirty,
            total_writeback,
            total_pgpgin,
            total_pgpgout,
            total_pgfault,
            total_pgmajfault,
            total_active_anon,
            total_inactive_anon,
            total_active_file,
            total_inactive_file,
            total_unevictable
        ],
        [
            swap,
            total_swap,
            hierarchical_memsw_limit
        ]
    }
}

fn parse_numa_stat(reader: impl io::Read) -> Result<NumaStat> {
    #![allow(clippy::unnecessary_unwrap)]

    let buf = io::BufReader::new(reader);

    macro_rules! g {
        ($key0: ident, $( $key: ident ),*) => {
            let mut $key0 = None;
            $( let mut $key = None; )*

            g!(_parse_keys; $key0, $( $key ),*);

            if $( $key.is_some() && )* $key0.is_some() {
                let $key0 = $key0.unwrap();
                $( let $key = $key.unwrap(); )*

                let len = $key0.1.len();
                $( if $key.1.len() != len { bail_parse!(); } )*

                Ok(NumaStat {
                    $key0,
                    $( $key, )*
                })
            } else {
                bail_parse!();
            }
        };

        (_parse_keys; $( $key: ident ),*) => {
            for line in buf.lines() {
                let line = line?;
                match line.split('=').next() {
                    $(
                        Some(stringify!($key)) => {
                            let mut entry = line.split(|c| c == ' ' || c == '=');

                            let total = parse_next(entry.by_ref().skip(1))?;
                            // FIXME: validate keys
                            let nodes = entry
                                .skip(1)
                                .step_by(2)
                                .map(|n| n.parse::<u64>())
                                .collect::<std::result::Result<Vec<_>, std::num::ParseIntError>>()?;

                            $key = Some((total, nodes));
                        }
                    )*
                    _ => { bail_parse!(); }
                }
            }

        };
    }

    g! {
        total,
        file,
        anon,
        unevictable,
        hierarchical_total,
        hierarchical_file,
        hierarchical_anon,
        hierarchical_unevictable
    }
}

fn parse_oom_control(reader: impl io::Read) -> Result<OomControl> {
    let buf = io::BufReader::new(reader);

    let mut oom_kill_disable = None;
    let mut under_oom = None;
    let mut oom_kill = None;

    for line in buf.lines() {
        let line = line?;
        let mut entry = line.split_whitespace();

        match entry.next() {
            Some("oom_kill_disable") => {
                if oom_kill_disable.is_some() {
                    bail_parse!();
                }
                oom_kill_disable = Some(parse_01_bool_option(entry.next())?);
            }
            Some("under_oom") => {
                if under_oom.is_some() {
                    bail_parse!();
                }
                under_oom = Some(parse_01_bool_option(entry.next())?);
            }
            Some("oom_kill") => {
                if oom_kill.is_some() {
                    bail_parse!();
                }
                oom_kill = Some(parse_next(&mut entry)?);
            }
            _ => {
                bail_parse!();
            }
        }

        if entry.next().is_some() {
            bail_parse!();
        }
    }

    match (oom_kill_disable, under_oom) {
        (Some(oom_kill_disable), Some(under_oom)) => Ok(OomControl {
            oom_kill_disable,
            under_oom,
            oom_kill,
        }),
        _ => {
            bail_parse!();
        }
    }
}

fn parse_01_bool_option(s: Option<&str>) -> Result<bool> {
    match s {
        Some(s) => parse_01_bool(s.as_bytes()),
        None => bail_parse!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v1::SubsystemKind;

    const LIMIT_DEFAULT: u64 = 0x7FFF_FFFF_FFFF_F000;

    #[test]
    fn test_subsystem_create_file_exists_delete() -> Result<()> {
        gen_test_subsystem_create_delete!(
            Memory,
            STAT,
            NUMA_STAT,

            USAGE_IN_BYTES,
            // MEMSW_USAGE_IN_BYTES,
            KMEM_USAGE_IN_BYTES,
            KMEM_TCP_USAGE_IN_BYTES,

            MAX_USAGE_IN_BYTES,
            // MEMSW_MAX_USAGE_IN_BYTES,
            KMEM_MAX_USAGE_IN_BYTES,
            KMEM_TCP_MAX_USAGE_IN_BYTES,

            LIMIT_IN_BYTES,
            // MEMSW_LIMIT_IN_BYTES,
            KMEM_LIMIT_IN_BYTES,
            KMEM_TCP_LIMIT_IN_BYTES,

            SOFT_LIMIT_IN_BYTES,

            FAILCNT,
            // MEMSW_FAILCNT,
            KMEM_FAILCNT,
            KMEM_TCP_FAILCNT,

            SWAPPINESS,
            OOM_CONTROL,
            MOVE_CHARGE_AT_IMMIGRATE,
            USE_HIERARCHY,
            FORCE_EMPTY,

            USAGE_IN_BYTES,
            MAX_USAGE_IN_BYTES,
            LIMIT_IN_BYTES,
        )
    }

    #[test]
    fn test_subsystem_apply() -> Result<()> {
        #![allow(clippy::identity_op)]

        const GB: i64 = 1 << 30;

        gen_test_subsystem_apply!(
            Memory,
            Resources {
                limit_in_bytes: Some(1 * GB),
                memsw_limit_in_bytes: None, // Some(1 * GB),
                kmem_limit_in_bytes: Some(1 * GB),
                kmem_tcp_limit_in_bytes: Some(1 * GB),
                soft_limit_in_bytes: Some(2 * GB),
                swappiness: Some(100),
                move_charge_at_immigrate: Some(true),
                use_hierarchy: None, // Some(false),
            },
            (limit_in_bytes, 1 * GB as u64),
            (kmem_limit_in_bytes, 1 * GB as u64),
            (kmem_tcp_limit_in_bytes, 1 * GB as u64),
            (soft_limit_in_bytes, 2 * GB as u64),
            (swappiness, 100),
            (move_charge_at_immigrate, true),
        )
    }

    #[test]
    fn test_subsystem_stat() -> Result<()> {
        let mut cgroup = Subsystem::new(CgroupPath::new(SubsystemKind::Memory, gen_cgroup_name!()));
        cgroup.create()?;

        let stat = cgroup.stat()?;

        macro_rules! assert_0 {
            ( $( $r: ident ),* $(, )? ) => { $( assert_eq!(stat.$r, 0); )* }
        }

        assert_0!(
            cache,
            rss,
            rss_huge,
            shmem,
            mapped_file,
            dirty,
            writeback,
            pgpgin,
            pgpgout,
            pgfault,
            pgmajfault,
            active_anon,
            inactive_anon,
            active_file,
            inactive_file,
            unevictable,
        );
        assert_eq!(stat.swap.unwrap_or(0), 0);
        assert_eq!(stat.hierarchical_memory_limit, LIMIT_DEFAULT);
        assert_eq!(
            stat.hierarchical_memsw_limit.unwrap_or(LIMIT_DEFAULT),
            LIMIT_DEFAULT
        );

        assert_0!(
            total_cache,
            total_rss,
            total_rss_huge,
            total_shmem,
            total_mapped_file,
            total_dirty,
            total_writeback,
            total_pgpgin,
            total_pgpgout,
            total_pgfault,
            total_pgmajfault,
            total_active_anon,
            total_inactive_anon,
            total_active_file,
            total_inactive_file,
            total_unevictable,
        );
        assert_eq!(stat.total_swap.unwrap_or(0), 0);

        cgroup.delete()
    }

    #[test]
    fn test_subsystem_numa_stat() -> Result<()> {
        // Assuming non-NUMA systems

        gen_test_subsystem_get!(
            Memory,
            numa_stat,
            NumaStat {
                total: (0, vec![0]),
                file: (0, vec![0]),
                anon: (0, vec![0]),
                unevictable: (0, vec![0]),

                hierarchical_total: (0, vec![0]),
                hierarchical_file: (0, vec![0]),
                hierarchical_anon: (0, vec![0]),
                hierarchical_unevictable: (0, vec![0]),
            }
        )
    }

    macro_rules! _gen_test_getters {
        ($getter: ident, $memsw: ident, $kmem: ident, $tcp: ident, $val: expr) => {{
            let mut cgroup =
                Subsystem::new(CgroupPath::new(SubsystemKind::Memory, gen_cgroup_name!()));
            cgroup.create()?;

            assert_eq!(cgroup.$getter()?, $val);
            if cgroup.file_exists(concat!(stringify!(memory), ".", stringify!($memsw))) {
                assert_eq!(cgroup.$memsw()?, $val);
            }
            assert_eq!(cgroup.$kmem()?, $val);
            assert_eq!(cgroup.$tcp()?, $val);

            cgroup.delete()
        }};
    }

    #[test]
    fn test_subsystem_usage_in_bytes() -> Result<()> {
        _gen_test_getters!(
            usage_in_bytes,
            memsw_usage_in_bytes,
            kmem_usage_in_bytes,
            kmem_tcp_usage_in_bytes,
            0
        )
    }

    #[test]
    fn test_subsystem_max_usage_in_bytes() -> Result<()> {
        _gen_test_getters!(
            max_usage_in_bytes,
            memsw_max_usage_in_bytes,
            kmem_max_usage_in_bytes,
            kmem_tcp_max_usage_in_bytes,
            0
        )
    }

    #[test]
    fn test_subsystem_limit_in_bytes() -> Result<()> {
        _gen_test_getters!(
            limit_in_bytes,
            memsw_limit_in_bytes,
            kmem_limit_in_bytes,
            kmem_tcp_limit_in_bytes,
            LIMIT_DEFAULT
        )
    }

    #[test]
    fn test_subsystem_failcnt() -> Result<()> {
        _gen_test_getters!(failcnt, memsw_failcnt, kmem_failcnt, kmem_tcp_failcnt, 0)
    }

    #[test]
    fn test_subsystem_swappiness() -> Result<()> {
        gen_test_subsystem_get_set!(Memory, swappiness, 60, set_swappiness, 100)
    }

    #[test]
    fn test_subsystem_oom_control() -> Result<()> {
        let mut cgroup = Subsystem::new(CgroupPath::new(SubsystemKind::Memory, gen_cgroup_name!()));
        cgroup.create()?;
        assert_eq!(
            cgroup.oom_control()?,
            OomControl {
                oom_kill_disable: false,
                under_oom: false,
                oom_kill: Some(0),
            }
        );

        cgroup.disable_oom_killer(true)?;
        assert_eq!(
            cgroup.oom_control()?,
            OomControl {
                oom_kill_disable: true,
                under_oom: false,
                oom_kill: Some(0),
            }
        );

        cgroup.delete()
    }

    #[test]
    fn test_subsystem_move_charge_at_immigrate() -> Result<()> {
        gen_test_subsystem_get_set!(
            Memory,
            move_charge_at_immigrate,
            false,
            set_move_charge_at_immigrate,
            true
        )
    }

    #[test]
    fn test_subsystem_use_hierarchy() -> Result<()> {
        let mut cgroup = Subsystem::new(CgroupPath::new(SubsystemKind::Memory, gen_cgroup_name!()));
        cgroup.create()?;

        assert!(cgroup.use_hierarchy()?);

        // Disabling fails if the parent cgroup has already enabled
        if !cgroup.root_cgroup().use_hierarchy()? {
            cgroup.set_use_hierarchy(false)?;
            assert!(!cgroup.use_hierarchy()?);
        }

        cgroup.delete()
    }

    #[test]
    fn test_subsystem_force_empty() -> Result<()> {
        let mut cgroup = Subsystem::new(CgroupPath::new(SubsystemKind::Memory, gen_cgroup_name!()));
        cgroup.create()?;

        cgroup.force_empty()?;

        cgroup.delete()
    }

    #[test]
    #[ignore] // must not be executed in parallel
    fn test_subsystem_stat_throttled() -> Result<()> {
        #![allow(clippy::identity_op)]

        const LIMIT: usize = 1 * (1 << 20);

        let mut cgroup = Subsystem::new(CgroupPath::new(SubsystemKind::Memory, gen_cgroup_name!()));
        cgroup.create()?;

        cgroup.set_limit_in_bytes(LIMIT as i64)?;

        let mut child = std::process::Command::new("bash")
            .arg("-c")
            .arg(&format!(
                "sleep 1; ary=(); for ((i=0; i<{}; i++)); do ary+=(0); done",
                LIMIT / 8
            ))
            .spawn()
            .unwrap();

        let child_pid = crate::Pid::from(&child);
        cgroup.add_proc(child_pid)?;

        child.wait().unwrap();

        let stat = cgroup.stat()?;
        assert!(stat.pgpgin > 0 && stat.pgpgout > 0 && stat.pgfault > 0);
        // assert!(cgroup.usage_in_bytes()? > 0);
        assert_eq!(cgroup.max_usage_in_bytes()?, LIMIT as u64);
        assert!(cgroup.failcnt()? > 0);

        cgroup.delete()
    }

    #[test]
    fn test_parse_stat() -> Result<()> {
        #![allow(clippy::unreadable_literal)]

        const CONTENT_OK: &str = "\
cache 806506496
rss 6950912
rss_huge 0
shmem 434176
mapped_file 12664832
dirty 32768
writeback 0
pgpgin 596219
pgpgout 397621
pgfault 609057
pgmajfault 186
inactive_anon 3731456
active_anon 3653632
inactive_file 220020736
active_file 586051584
unevictable 0
hierarchical_memory_limit 9223372036854771712
total_cache 7228424192
total_rss 7746449408
total_rss_huge 0
total_shmem 943890432
total_mapped_file 1212370944
total_dirty 7065600
total_writeback 0
total_pgpgin 3711221840
total_pgpgout 3707566876
total_pgfault 4750639337
total_pgmajfault 82700
total_inactive_anon 1127153664
total_active_anon 7428182016
total_inactive_file 2238832640
total_active_file 4166680576
total_unevictable 14004224
";

        let stat = parse_stat(CONTENT_OK.as_bytes())?;

        assert_eq!(
            stat,
            Stat {
                cache: 806506496,
                rss: 6950912,
                rss_huge: 0,
                shmem: 434176,
                mapped_file: 12664832,
                dirty: 32768,
                writeback: 0,
                swap: None,
                pgpgin: 596219,
                pgpgout: 397621,
                pgfault: 609057,
                pgmajfault: 186,
                inactive_anon: 3731456,
                active_anon: 3653632,
                inactive_file: 220020736,
                active_file: 586051584,
                unevictable: 0,
                hierarchical_memory_limit: 9223372036854771712,
                hierarchical_memsw_limit: None,
                total_cache: 7228424192,
                total_rss: 7746449408,
                total_rss_huge: 0,
                total_shmem: 943890432,
                total_mapped_file: 1212370944,
                total_dirty: 7065600,
                total_writeback: 0,
                total_swap: None,
                total_pgpgin: 3711221840,
                total_pgpgout: 3707566876,
                total_pgfault: 4750639337,
                total_pgmajfault: 82700,
                total_inactive_anon: 1127153664,
                total_active_anon: 7428182016,
                total_inactive_file: 2238832640,
                total_active_file: 4166680576,
                total_unevictable: 14004224,
            }
        );

        assert_eq!(
            parse_stat("".as_bytes()).unwrap_err().kind(),
            ErrorKind::Parse
        );

        Ok(())
    }

    #[test]
    fn test_parse_numa_stat() -> Result<()> {
        #![allow(clippy::unreadable_literal)]

        const CONTENT_OK: &str = "\
total=200910 N0=200910 N1=0
file=199107 N0=199107 N1=1
anon=1803 N0=1803 N1=2
unevictable=0 N0=0 N1=3
hierarchical_total=3596692 N0=3596692 N1=4
hierarchical_file=1383803 N0=1383803 N1=5
hierarchical_anon=2209488 N0=2209492 N1=6
hierarchical_unevictable=3419 N0=3419 N1=7
";

        let numa_stat = parse_numa_stat(CONTENT_OK.as_bytes())?;

        assert_eq!(
            numa_stat,
            NumaStat {
                total: (200910, vec![200910, 0]),
                file: (199107, vec![199107, 1]),
                anon: (1803, vec![1803, 2]),
                unevictable: (0, vec![0, 3]),
                hierarchical_total: (3596692, vec![3596692, 4]),
                hierarchical_file: (1383803, vec![1383803, 5]),
                hierarchical_anon: (2209488, vec![2209492, 6]),
                hierarchical_unevictable: (3419, vec![3419, 7]),
            }
        );

        assert_eq!(
            parse_numa_stat("".as_bytes()).unwrap_err().kind(),
            ErrorKind::Parse
        );

        Ok(())
    }

    #[test]
    fn test_parse_oom_control() -> Result<()> {
        const CONTENT_OK_WITH_OOM_KILL: &str = "\
oom_kill_disable 1
under_oom 1
oom_kill 42
";

        assert_eq!(
            parse_oom_control(CONTENT_OK_WITH_OOM_KILL.as_bytes())?,
            OomControl {
                oom_kill_disable: true,
                under_oom: true,
                oom_kill: Some(42),
            }
        );

        const CONTENT_OK_WITHOUT_OOM_KILL: &str = "\
oom_kill_disable 1
under_oom 1
";

        assert_eq!(
            parse_oom_control(CONTENT_OK_WITHOUT_OOM_KILL.as_bytes())?,
            OomControl {
                oom_kill_disable: true,
                under_oom: true,
                oom_kill: None,
            }
        );

        const CONTENT_NG_NOT_INT: &str = "\
oom_kill_disable 1
under_oom invalid
";

        const CONTENT_NG_MISSING_DATA: &str = "\
oom_kill_disable 1
under_oom invalid
oom_kill
";

        const CONTENT_NG_EXTRA_DATA: &str = "\
oom_kill_disable 1 invalid
under_oom invalid
oom_kill 0
";

        const CONTENT_NG_EXTRA_ROW: &str = "\
oom_kill_disable 1 invalid
under_oom invalid
oom_kill 0
invalid 0
";

        for case in &[
            CONTENT_NG_NOT_INT,
            CONTENT_NG_MISSING_DATA,
            CONTENT_NG_EXTRA_DATA,
            CONTENT_NG_EXTRA_ROW,
        ] {
            assert_eq!(
                parse_oom_control(case.as_bytes()).unwrap_err().kind(),
                ErrorKind::Parse
            );
        }

        Ok(())
    }

    #[test]
    fn test_parse_01_bool_option() {
        assert_eq!(parse_01_bool_option(Some("0")).unwrap(), false);
        assert_eq!(parse_01_bool_option(Some("1")).unwrap(), true);

        assert_eq!(
            parse_01_bool_option(Some("invalid")).unwrap_err().kind(),
            ErrorKind::Parse
        );
        assert_eq!(
            parse_01_bool_option(None).unwrap_err().kind(),
            ErrorKind::Parse
        );
    }
}
