//! Operations on a hugetlb subsystem.
//!
//! [`Subsystem`] implements [`Cgroup`] trait and subsystem-specific behaviors.
//!
//! For more information about this subsystem, see the kernel's documentation
//! [Documentation/cgroup-v1/hugetlb.txt].
//!
//! # Examples
//!
//! ```no_run
//! # fn main() -> cgroups::Result<()> {
//! use std::path::PathBuf;
//! use cgroups::{Pid, v1::{self, hugetlb, Cgroup, CgroupPath, SubsystemKind}};
//!
//! let mut hugetlb_cgroup = hugetlb::Subsystem::new(
//!     CgroupPath::new(SubsystemKind::HugeTlb, PathBuf::from("students/charlie")));
//! hugetlb_cgroup.create()?;
//!
//! // Define a resource limit about how many hugepage TLB a cgroup can use.
//! let resources = hugetlb::Resources {
//!     limit_2mb: Some(hugetlb::Limit::Pages(1)),
//!     limit_1gb: Some(hugetlb::Limit::Pages(1)),
//! };
//!
//! // Apply the resource limit.
//! hugetlb_cgroup.apply(&resources.into())?;
//!
//! // Add tasks to this cgroup.
//! let pid = Pid::from(std::process::id());
//! hugetlb_cgroup.add_task(pid)?;
//!
//! // Do something ...
//!
//! hugetlb_cgroup.remove_task(pid)?;
//! hugetlb_cgroup.delete()?;
//! # Ok(())
//! # }
//! ```
//!
//! [`Subsystem`]: struct.Subsystem.html
//! [`Cgroup`]: ../trait.Cgroup.html
//!
//! [Documentation/cgroup-v1/hugetlb.txt]: https://www.kernel.org/doc/Documentation/cgroup-v1/hugetlb.txt

use std::{fmt, path::PathBuf};

use crate::{
    parse::parse,
    v1::{self, cgroup::CgroupHelper, Cgroup, CgroupPath},
    Result,
};

/// Handler of a hugetlb subsystem.
#[derive(Debug)]
pub struct Subsystem {
    path: CgroupPath,
}

/// Resource limit no how many hugepage TLBs a cgroup can use.
///
/// See the kernel's documentation for more information about the fields.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Resources {
    /// How many 2 MB size hugepage TLBs this cgroup can use.
    pub limit_2mb: Option<Limit>,
    /// How many 1 GB size hugepage TLBs this cgroup can use.
    pub limit_1gb: Option<Limit>,
}

/// Limit on hugepage TLB usage in different units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// Limit hugepage TLB usage in bytes.
    Bytes(u64),
    /// Limit hugepage TLB usage in pages.
    Pages(u64),
}

/// Supported hugepage sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HugepageSize {
    /// 2 MB hugepage.
    Mb2,
    /// 1 GB hugepage.
    Gb1,
}

impl_cgroup! {
    Subsystem, HugeTlb,

    /// Applies the `Some` fields in `resources.hugetlb`.
    ///
    /// See [`Cgroup::apply`] for general information.
    ///
    /// [`Cgroup::apply`]: ../trait.Cgroup.html#tymethod.apply
    fn apply(&mut self, resources: &v1::Resources) -> Result<()> {
        if let Some(limit) = resources.hugetlb.limit_2mb {
            self.set_limit(HugepageSize::Mb2, limit)?;
        }
        if let Some(limit) = resources.hugetlb.limit_1gb {
            self.set_limit(HugepageSize::Gb1, limit)?;
        }

        Ok(())
    }
}

macro_rules! _gen_getter {
    ($desc: literal, $in_bytes: ident, $in_pages: ident) => {
        with_doc! { concat!(
            "Reads ", $desc, " in bytes from",
            " `hugetlb.<hugepage size>.", stringify!($in_bytes), "` file.\n\n",
            gen_doc!(see),
            "# Errors\n\n",
            "Returns an error if failed to read and parse",
            " `hugetlb.<hugepage size>.", stringify!($in_bytes), "` file of this cgroup.\n\n",
            gen_doc!(eg_read; hugetlb, $in_bytes, hugetlb::HugepageSize::Mb2)),
            pub fn $in_bytes(&self, size: HugepageSize) -> Result<u64> {
                self.open_file_read(&format!("hugetlb.{}.{}", size, stringify!($in_bytes)))
                    .and_then(parse)
            }
        }

        with_doc! { concat!(
            "Reads ", $desc, " in pages. See [`", stringify!($in_bytes), "`](#method.",
            stringify!($in_bytes), ") for more information."),
            pub fn $in_pages(&self, size: HugepageSize) -> Result<u64> {
                self.$in_bytes(size).map(|b| bytes_to_pages(b, size))
            }
        }
    };
}

const LIMIT_IN_BYTES: &str = "limit_in_bytes";
const USAGE_IN_BYTES: &str = "usage_in_bytes";
const MAX_USAGE_IN_BYTES: &str = "max_usage_in_bytes";
const FAILCNT: &str = "failcnt";

impl Subsystem {
    /// Returns whether the system supports hugepage in `size`.
    ///
    /// Note that this method returns `false` if the directory of this cgroup is not created yet.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # fn main() -> cgroups::Result<()> {
    /// use std::path::PathBuf;
    /// use cgroups::v1::{hugetlb::{self, HugepageSize}, Cgroup, CgroupPath, SubsystemKind};
    ///
    /// let mut cgroup = hugetlb::Subsystem::new(
    ///     CgroupPath::new(SubsystemKind::HugeTlb, PathBuf::from("students/charlie")));
    /// cgroup.create()?;
    ///
    /// let support_2mb = cgroup.size_supported(HugepageSize::Mb2);
    /// let support_1gb = cgroup.size_supported(HugepageSize::Gb1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn size_supported(&self, size: HugepageSize) -> bool {
        self.file_exists(&format!("hugetlb.{}.{}", size, LIMIT_IN_BYTES))
            && self.file_exists(&format!("hugetlb.{}.{}", size, USAGE_IN_BYTES))
            && self.file_exists(&format!("hugetlb.{}.{}", size, MAX_USAGE_IN_BYTES))
            && self.file_exists(&format!("hugetlb.{}.{}", size, FAILCNT))
    }

    _gen_getter!(
        "the limit of hugepage TLB usage",
        limit_in_bytes,
        limit_in_pages
    );

    with_doc! { concat!(
        "Sets a limit of hugepage TLB usage by writing to",
        " `hugetlb.<hugepage size>.limit_in_bytes` file.\n\n",
        gen_doc!(see),
        "# Errors\n\n",
        "Returns an error if failed to write to",
        " `hugetlb.<hugepage size>.limit_in_bytes` file of this cgroup.\n\n",
        gen_doc!(
            eg_write; hugetlb,
            set_limit, hugetlb::HugepageSize::Mb2, hugetlb::Limit::Pages(4)
        )),
        pub fn set_limit(&mut self, size: HugepageSize, limit: Limit) -> Result<()> {
            match limit {
                Limit::Bytes(bytes) => self.set_limit_in_bytes(size, bytes),
                Limit::Pages(pages) => self.set_limit_in_pages(size, pages),
            }
        }
    }

    /// Sets a limit of hugepage TLB usage in bytes. See [`set_limit`] for more information.
    ///
    /// [`set_limit`]: #method.set_limit
    pub fn set_limit_in_bytes(&mut self, size: HugepageSize, bytes: u64) -> Result<()> {
        self.write_file(&format!("hugetlb.{}.{}", size, LIMIT_IN_BYTES), bytes)
    }

    /// Sets a limit of hugepage TLB usage in pages. See [`set_limit`] for more information.
    ///
    /// [`set_limit`]: #method.set_limit
    pub fn set_limit_in_pages(&mut self, size: HugepageSize, pages: u64) -> Result<()> {
        self.set_limit_in_bytes(size, pages_to_bytes(pages, size))
    }

    _gen_getter!(
        "the current usage of hugepage TLB",
        usage_in_bytes,
        usage_in_pages
    );

    _gen_getter!(
        "the maximum recorded usage of hugepage TLB",
        max_usage_in_bytes,
        max_usage_in_pages
    );

    with_doc! { concat!(
        "Reads the number of allocation failure due to the limit, from",
        " `hugetlb.<hugepage size>.failcnt` file.\n\n",
        gen_doc!(see),
        "# Error\n\n",
        "Returns an error if failed to read and parse `hugetlb.<hugepage size>.failcnt` file of",
        " this cgroup.\n\n",
        gen_doc!(eg_read; hugetlb, failcnt, hugetlb::HugepageSize::Mb2)),
        pub fn failcnt(&self, size: HugepageSize) -> Result<u64> {
            self.open_file_read(&format!("hugetlb.{}.{}", size, FAILCNT))
                .and_then(parse)
        }
    }
}

const MB2_BYTES_PER_PAGE: u64 = 2 << 20;
const GB1_BYTES_PER_PAGE: u64 = 1 << 30;

fn bytes_to_pages(bytes: u64, size: HugepageSize) -> u64 {
    match size {
        HugepageSize::Mb2 => bytes / MB2_BYTES_PER_PAGE,
        HugepageSize::Gb1 => bytes / GB1_BYTES_PER_PAGE,
    }
}

fn pages_to_bytes(pages: u64, size: HugepageSize) -> u64 {
    match size {
        HugepageSize::Mb2 => pages * MB2_BYTES_PER_PAGE,
        HugepageSize::Gb1 => pages * GB1_BYTES_PER_PAGE,
    }
}

impl Into<v1::Resources> for Resources {
    fn into(self) -> v1::Resources {
        v1::Resources {
            hugetlb: self,
            ..v1::Resources::default()
        }
    }
}

impl fmt::Display for HugepageSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mb2 => write!(f, "2MB"),
            Self::Gb1 => write!(f, "1GB"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v1::SubsystemKind;
    use HugepageSize::*;

    const LIMIT_2MB_BYTES_DEFAULT: u64 = 0x7FFF_FFFF_FFE0_0000;
    const LIMIT_1GB_BYTES_DEFAULT: u64 = 0x7FFF_FFFF_C000_0000;

    #[test]
    fn test_subsystem_create_file_exists() -> Result<()> {
        let mut cgroup =
            Subsystem::new(CgroupPath::new(SubsystemKind::HugeTlb, gen_cgroup_name!()));
        cgroup.create()?;
        for size in &[Mb2, Gb1] {
            for f in &[LIMIT_IN_BYTES, USAGE_IN_BYTES, MAX_USAGE_IN_BYTES, FAILCNT] {
                assert!(cgroup.file_exists(&format!("hugetlb.{}.{}", size, f)));
            }
        }
        assert!(!cgroup.file_exists("does_not_exist"));

        cgroup.delete()?;
        for size in &[Mb2, Gb1] {
            for f in &[LIMIT_IN_BYTES, USAGE_IN_BYTES, MAX_USAGE_IN_BYTES, FAILCNT] {
                assert!(!cgroup.file_exists(&format!("hugetlb.{}.{}", size, f)));
            }
        }

        Ok(())
    }

    #[test]
    fn test_subsystem_size_supported() -> Result<()> {
        let mut cgroup =
            Subsystem::new(CgroupPath::new(SubsystemKind::HugeTlb, gen_cgroup_name!()));

        assert!(!cgroup.size_supported(Mb2));
        assert!(!cgroup.size_supported(Gb1));

        cgroup.create()?;

        assert!(cgroup.size_supported(Mb2));
        assert!(cgroup.size_supported(Gb1));

        cgroup.delete()
    }

    #[test]
    fn test_subsystem_limit_in_bytes() -> Result<()> {
        let mut cgroup =
            Subsystem::new(CgroupPath::new(SubsystemKind::HugeTlb, gen_cgroup_name!()));
        cgroup.create()?;
        assert_eq!(cgroup.limit_in_bytes(Mb2)?, LIMIT_2MB_BYTES_DEFAULT);
        assert_eq!(cgroup.limit_in_bytes(Gb1)?, LIMIT_1GB_BYTES_DEFAULT);

        cgroup.set_limit_in_bytes(Mb2, 4 * (1 << 21))?;
        assert_eq!(cgroup.limit_in_bytes(Mb2)?, 4 * (1 << 21));

        cgroup.set_limit_in_bytes(Gb1, 4 * (1 << 30))?;
        assert_eq!(cgroup.limit_in_bytes(Gb1)?, 4 * (1 << 30));

        cgroup.delete()
    }

    #[test]
    fn test_subsystem_limit_in_pages() -> Result<()> {
        let mut cgroup =
            Subsystem::new(CgroupPath::new(SubsystemKind::HugeTlb, gen_cgroup_name!()));
        cgroup.create()?;
        assert_eq!(cgroup.limit_in_pages(Mb2)?, LIMIT_2MB_BYTES_DEFAULT >> 21);
        assert_eq!(cgroup.limit_in_pages(Gb1)?, LIMIT_1GB_BYTES_DEFAULT >> 30);

        cgroup.set_limit_in_pages(Mb2, 4)?;
        assert_eq!(cgroup.limit_in_pages(Mb2)?, 4);

        cgroup.set_limit_in_pages(Gb1, 4)?;
        assert_eq!(cgroup.limit_in_pages(Gb1)?, 4);

        cgroup.delete()
    }

    #[test]
    fn test_subsystem_set_limit() -> Result<()> {
        let mut cgroup =
            Subsystem::new(CgroupPath::new(SubsystemKind::HugeTlb, gen_cgroup_name!()));
        cgroup.create()?;

        cgroup.set_limit(Mb2, Limit::Bytes(4 * (1 << 21)))?;
        assert_eq!(cgroup.limit_in_bytes(Mb2)?, 4 * (1 << 21));

        cgroup.set_limit(Mb2, Limit::Pages(4))?;
        assert_eq!(cgroup.limit_in_pages(Mb2)?, 4);

        cgroup.set_limit(Gb1, Limit::Bytes(4 * (1 << 30)))?;
        assert_eq!(cgroup.limit_in_bytes(Gb1)?, 4 * (1 << 30));

        cgroup.set_limit(Gb1, Limit::Pages(4))?;
        assert_eq!(cgroup.limit_in_pages(Gb1)?, 4);

        cgroup.delete()
    }

    // TODO: test adding tasks

    #[test]
    fn test_subsystem_usage() -> Result<()> {
        let mut cgroup =
            Subsystem::new(CgroupPath::new(SubsystemKind::HugeTlb, gen_cgroup_name!()));
        cgroup.create()?;

        assert_eq!(cgroup.usage_in_bytes(Mb2)?, 0);
        assert_eq!(cgroup.usage_in_bytes(Gb1)?, 0);

        assert_eq!(cgroup.usage_in_pages(Mb2)?, 0);
        assert_eq!(cgroup.usage_in_pages(Gb1)?, 0);

        cgroup.delete()
    }

    #[test]
    fn test_subsystem_max_usage() -> Result<()> {
        let mut cgroup =
            Subsystem::new(CgroupPath::new(SubsystemKind::HugeTlb, gen_cgroup_name!()));
        cgroup.create()?;

        assert_eq!(cgroup.max_usage_in_bytes(Mb2)?, 0);
        assert_eq!(cgroup.max_usage_in_bytes(Gb1)?, 0);

        assert_eq!(cgroup.max_usage_in_pages(Mb2)?, 0);
        assert_eq!(cgroup.max_usage_in_pages(Gb1)?, 0);

        cgroup.delete()
    }

    #[test]
    fn test_subsystem_failcnt() -> Result<()> {
        let mut cgroup =
            Subsystem::new(CgroupPath::new(SubsystemKind::HugeTlb, gen_cgroup_name!()));
        cgroup.create()?;

        assert_eq!(cgroup.failcnt(Mb2)?, 0);
        assert_eq!(cgroup.failcnt(Gb1)?, 0);

        cgroup.delete()
    }

    #[test]
    fn test_bytes_to_pages() {
        assert_eq!(bytes_to_pages(1 * (1 << 20), Mb2), 0);
        assert_eq!(bytes_to_pages(1 * (1 << 21), Mb2), 1);
        assert_eq!(bytes_to_pages(4 * (1 << 21), Mb2), 4);

        assert_eq!(bytes_to_pages(1 * (1 << 29), Gb1), 0);
        assert_eq!(bytes_to_pages(1 * (1 << 30), Gb1), 1);
        assert_eq!(bytes_to_pages(4 * (1 << 30), Gb1), 4);
    }

    #[test]
    fn test_pages_to_bytes() {
        assert_eq!(pages_to_bytes(0, Mb2), 0);
        assert_eq!(pages_to_bytes(1, Mb2), 1 * (1 << 21));
        assert_eq!(pages_to_bytes(4, Mb2), 4 * (1 << 21));

        assert_eq!(pages_to_bytes(0, Gb1), 0);
        assert_eq!(pages_to_bytes(1, Gb1), 1 * (1 << 30));
        assert_eq!(pages_to_bytes(4, Gb1), 4 * (1 << 30));
    }
}
