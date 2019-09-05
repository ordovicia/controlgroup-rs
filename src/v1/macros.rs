macro_rules! gen_doc {
    (reads; $subsystem: ident, $desc: literal $( : $detail: literal )?, $field: ident) => { concat!(
        "Reads ", $desc, " from `", subsystem_file!($subsystem, $field), "` file.",
        $( " ", $detail, )? "\n\n",
    ) };
    (reads; $file_prefix: literal, $desc: literal $( : $detail: literal )?, $field: ident) => {
        concat!(
            "Reads ", $desc, " from `", subsystem_file!($file_prefix, $field), "` file.",
            $( " ", $detail, )? "\n\n",
        )
    };

    (reads_see; $subsystem: ident, $field: ident, $method: ident) => { concat!(
        "Reads `", subsystem_file!($subsystem, $field), "` file.",
        gen_doc!(_see_method; $method)
    ) };
    (reads_see; $file_prefix: literal, $field: ident, $method: ident) => { concat!(
        "Reads `", subsystem_file!($file_prefix, $field), "` file.",
        gen_doc!(_see_method; $method)
    ) };

    (sets; $subsystem: ident, $desc: literal $( : $detail: literal )?, $field: ident) => { concat!(
        "Sets ", $desc, " by writing to `", subsystem_file!($subsystem, $field), "` file.",
        $( " ", $detail, )? "\n\n",
    ) };
    (sets; $file_prefix: literal, $desc: literal $( : $detail: literal )?, $field: ident) => {
        concat!(
            "Sets ", $desc, " by writing to `", subsystem_file!($file_prefix, $field), "` file.",
            $( " ", $detail, )? "\n\n",
        )
    };

    (sets_see; $file_prefix: literal, $field: ident, $method: ident) => { concat!(
        "Writes to `", subsystem_file!($file_prefix, $field), "` file.",
        gen_doc!(_see_method; $method)
    ) };
    (_see_method; $method: ident) => { concat!(
        " See [`", stringify!($method), "`](#method.", stringify!($method), ")",
        " method for more information."
    ) };

    (see $(; $field: ident )?)  => { concat!(
        "See"
        $(, " [`Resources.", stringify!($field), "`]",
            "(struct.Resources.html#structfield.", stringify!($field), ") and" )?,
        " the kernel's documentation for more information about this field.\n\n"
    ) };

    (err_read; $subsystem: ident, $field: ident) => { concat!(
        "# Errors\n\n",
        "Returns an error if failed to read and parse `",
        subsystem_file!($subsystem, $field), "` file of this cgroup.\n\n"
    ) };
    (err_read; $file_prefix: literal, $field: ident) => { concat!(
        "# Errors\n\n",
        "Returns an error if failed to read and parse `",
        subsystem_file!($file_prefix, $field), "` file of this cgroup.\n\n"
    ) };

    (err_write; $subsystem: ident, $field: ident) => { concat!(
        "# Errors\n\n",
        "Returns an error if failed to write to `",
        subsystem_file!($subsystem, $field), "` file of this cgroup.\n\n"
    ) };
    (err_write; $file_prefix: literal, $field: ident) => { concat!(
        "# Errors\n\n",
        "Returns an error if failed to write to `",
        subsystem_file!($file_prefix, $field), "` file of this cgroup.\n\n"
    ) };

    (eg_read; $subsystem: ident, $field: ident $(, $val: expr )*) => { concat!(
"# Examples

```no_run
# fn main() -> controlgroup::Result<()> {
use std::path::PathBuf;
use controlgroup::v1::{", stringify!($subsystem), ", Cgroup, CgroupPath, SubsystemKind};

let cgroup = ", stringify!($subsystem), "::Subsystem::new(
    CgroupPath::new(SubsystemKind::", _kind!($subsystem), ", PathBuf::from(\"students/charlie\")));

let ", stringify!($field), " = cgroup.", stringify!($field), "(", stringify!($( $val ),* ), ")?;
# Ok(())
# }
```") };

    (eg_write; $subsystem: ident, $setter: ident $(, $val: expr )*) => { concat!(
"# Examples

```no_run
# fn main() -> controlgroup::Result<()> {
use std::path::PathBuf;
use controlgroup::v1::{", stringify!($subsystem), ", Cgroup, CgroupPath, SubsystemKind};

let mut cgroup = ", stringify!($subsystem), "::Subsystem::new(
    CgroupPath::new(SubsystemKind::", _kind!($subsystem), ", PathBuf::from(\"students/charlie\")));

cgroup.", stringify!($setter), "(", stringify!($( $val ),* ), ")?;
# Ok(())
# }
```") };
}

macro_rules! gen_getter {
    (
        $subsystem: ident,
        $desc: literal $( : $detail: literal )?,
        $field: ident $( : $link : ident )?,
        $ty: ty,
        $parser: ident
    ) => { with_doc! { concat!(
        gen_doc!(reads; $subsystem, $desc $( : $detail )?, $field),
        _link!($field $( : $link )?),
        gen_doc!(err_read; $subsystem, $field),
        gen_doc!(eg_read; $subsystem, $field)),
        pub fn $field(&self) -> Result<$ty> {
            self.open_file_read(subsystem_file!($subsystem, $field)).and_then($parser)
        }
    } };
}

macro_rules! gen_setter {
    (
        $subsystem: ident,
        $desc: literal $( : $detail: literal )?,
        $field: ident $( : $link: ident )?,
        $setter: ident,
        $ty: ty,
        $( $val: expr ),*
    ) => { with_doc! { concat!(
        gen_doc!(sets; $subsystem, $desc $( : $detail )?, $field),
        _link!($field $( : $link )?),
        gen_doc!(err_write; $subsystem, $field),
        gen_doc!(eg_write; $subsystem, $setter, $( $val ),*)),
        pub fn $setter(&mut self, $field: $ty) -> Result<()> {
            self.write_file(subsystem_file!($subsystem, $field), $field)
        }
    } };

    (
        $subsystem: ident,
        $desc: literal $( : $detail: literal )?,
        $field: ident $( : $link : ident )?,
        $setter: ident,
        $arg: ident : $ty: ty $( as $as: ty )?,
        $( $val: expr ),*
    ) => { with_doc! { concat!(
        gen_doc!(sets; $subsystem, $desc $( : $detail )?, $field),
        _link!($field $( : $link )?),
        gen_doc!(err_write; $subsystem, $field),
        gen_doc!(eg_write; $subsystem, $setter, $( $val ),*)),
        pub fn $setter(&mut self, $arg: $ty) -> Result<()> {
            self.write_file(subsystem_file!($subsystem, $field), $arg $( as $as )?)
        }
    } };
}

#[cfg(test)]
macro_rules! gen_subsystem_test {
    // Test create, file_exists, and delete
    ($kind: ident, [ $( $file: literal ),* $(, )?]) => { {
        use crate::v1::{CgroupPath, SubsystemKind};

        let files = vec![$(
            format!("{}.{}", SubsystemKind::$kind.as_str(), $file)
        ),*];

        let mut cgroup = Subsystem::new(
            CgroupPath::new(SubsystemKind::$kind, gen_cgroup_name!()));
        cgroup.create()?;

        assert!(files.iter().all(|f| cgroup.file_exists(f)));
        assert!(!cgroup.file_exists("does_not_exist"));

        cgroup.delete()?;
        assert!(files.iter().all(|f| !cgroup.file_exists(f)));

        let ok: Result<()> = Ok(());
        ok
    } };

    // Test errors
    ($kind: ident, $field: ident, $( ($err_kind: ident, $($arg: expr),*) ),* $(, )?) => { {
        use crate::{ErrorKind, v1::{CgroupPath, SubsystemKind}};

        let mut cgroup = Subsystem::new(
            CgroupPath::new(SubsystemKind::$kind, gen_cgroup_name!()));
        cgroup.create()?;

        $(
            assert_eq!(cgroup.$field($( $arg ),*).unwrap_err().kind(), ErrorKind::$err_kind);
        )*

        cgroup.delete()
    } };

    // Test `apply()`
    ($kind: ident, $resources: expr, $( ($field: ident, $val: expr) ),* $(, )?) => { {
        let mut cgroup =
            Subsystem::new(CgroupPath::new(v1::SubsystemKind::$kind, gen_cgroup_name!()));
        cgroup.create()?;

        cgroup.apply(&$resources.into())?;

        $(
            assert_eq!(cgroup.$field()?, $val);
        )*

        cgroup.delete()
    } };

    // Test a read-only field
    ($kind: ident, $field: ident, $default: expr) => { {
        use crate::v1::{CgroupPath, SubsystemKind};

        let mut cgroup = Subsystem::new(
            CgroupPath::new(SubsystemKind::$kind, gen_cgroup_name!()));
        cgroup.create()?;
        assert_eq!(cgroup.$field()?, $default);

        cgroup.delete()
    } };

    // Test a read-write field
    ($kind: ident, $field: ident, $default: expr, $setter: ident, $( $val: expr ),* $(, )?) => { {
        use crate::v1::{CgroupPath, SubsystemKind};

        let mut cgroup = Subsystem::new(
            CgroupPath::new(SubsystemKind::$kind, gen_cgroup_name!()));
        cgroup.create()?;
        assert_eq!(cgroup.$field()?, $default);

        $(
            cgroup.$setter($val)?;
            assert_eq!(cgroup.$field()?, $val);
        )*

        cgroup.delete()
    } };
}

macro_rules! _kind {
    (cpu) => {
        "Cpu"
    };
    (cpuset) => {
        "Cpuset"
    };
    (cpuacct) => {
        "Cpuacct"
    };
    (memory) => {
        "Memory"
    };
    (hugetlb) => {
        "HugeTlb"
    };
    (devices) => {
        "Devices"
    };
    (blkio) => {
        "BlkIo"
    };
    (rdma) => {
        "Rdma"
    };
    (net_prio) => {
        "NetPrio"
    };
    (net_cls) => {
        "NetCls"
    };
    (pids) => {
        "Pids"
    };
    (freezer) => {
        "Freezer"
    }; // (perf_event) => { "PerfEvent" };
}

macro_rules! _link {
    ($field: ident : link) => {
        gen_doc!(see; $field);
    };
    ($field: ident) => {
        gen_doc!(see);
    }
}