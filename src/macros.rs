macro_rules! with_doc {
    ($doc: expr, $( $tt: tt )*) => {
        #[doc = $doc]
        $( $tt )*
    };
}

macro_rules! subsystem_file {
    ($subsystem: ident, $field: ident) => {
        concat!(stringify!($subsystem), ".", stringify!($field))
    };
    ($subsystem: literal, $field: ident) => {
        concat!($subsystem, ".", stringify!($field))
    };
}

#[cfg(test)]
macro_rules! gen_cgroup_name {
    () => {
        std::path::PathBuf::from(format!(
            "cgroups_rs-{}-{}",
            std::path::Path::new(file!())
                .file_stem()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap(),
            line!()
        ))
    };
}

#[cfg(test)]
macro_rules! hashmap {
    ( $( ( $k: expr, $v: expr $(, )? ) ),* $(, )? ) => { {
        #[allow(unused_mut)]
        let mut hashmap = std::collections::HashMap::new();
        $( hashmap.insert($k, $v); )*
        hashmap
    } };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_subsystem_file() {
        assert_eq!(subsystem_file!(cgroup, procs), "cgroup.procs");
    }

    #[test]
    fn test_gen_cgroup_name() {
        assert_eq!(
            gen_cgroup_name!(),
            std::path::PathBuf::from("cgroups_rs-macros-51")
        );
    }

    #[test]
    fn test_hashmap() {
        use std::collections::HashMap;

        assert_eq!(
            hashmap! { (0, "zero"), (1, "one") },
            [(0, "zero"), (1, "one")]
                .iter()
                .copied()
                .collect::<HashMap<_, _>>()
        );
    }
}
