//! Builds the embedded data pack (T1.7.3).
//!
//! The tree is **validated here**, and a broken data file fails the build with
//! the same report `gwsim data validate` would print. That is what makes it
//! impossible for a release binary to carry data that does not load, even
//! though what gets embedded is the RON text rather than a checked artefact
//! (T1.7.1).

use std::path::PathBuf;

fn main() {
    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");

    // Editing any data file rebuilds the binary.
    println!("cargo:rerun-if-changed={}", data_dir.display());

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let pack_path = out_dir.join("data.pack");

    if !data_dir.is_dir() {
        // A source checkout always has data/. If it is missing, something is
        // wrong with the build rather than with the data, so say so plainly
        // and write an empty pack so the error is the one that matters.
        println!(
            "cargo:warning=no data directory at {}; embedding an empty pack",
            data_dir.display()
        );
        let bytes = empty_pack();
        let hash = gwsim_data::pack::content_hash(&[]);
        println!("cargo:rustc-env=GWSIM_PACK_HASH={hash}");
        println!(
            "cargo:rustc-env=GWSIM_VERSION_STRING={} (data {})",
            env!("CARGO_PKG_VERSION"),
            &hash[..12]
        );
        std::fs::write(&pack_path, bytes).expect("could not write the data pack");
        return;
    }

    let source = gwsim_data::source::DirSource::new(&data_dir);

    // Validate before packing. The bytes we embed are the bytes that passed.
    if let Err(problems) = gwsim_data::dataset::DataSet::load(&source) {
        for line in problems.to_string().lines() {
            println!("cargo:warning={line}");
        }
        panic!(
            "the data tree in {} does not validate; see the warnings above",
            data_dir.display()
        );
    }

    let bytes = gwsim_data::pack::DataPack::bytes_from_source(&source)
        .expect("the tree validated, so packing it cannot fail");

    // The hash goes into the binary as an environment variable so that
    // `gwsim --version` can print it without loading the pack first.
    let pack = gwsim_data::pack::DataPack::from_source(&source)
        .expect("the tree validated, so loading it cannot fail");
    println!(
        "cargo:rustc-env=GWSIM_PACK_HASH={}",
        pack.version.content_hash
    );
    println!(
        "cargo:rustc-env=GWSIM_VERSION_STRING={} (data {})",
        env!("CARGO_PKG_VERSION"),
        pack.version.short_hash()
    );

    std::fs::write(&pack_path, bytes).expect("could not write the data pack");
}

/// A pack with no files, in the current format.
fn empty_pack() -> Vec<u8> {
    let source = gwsim_data::source::MemSource::default();
    gwsim_data::pack::DataPack::bytes_from_source(&source)
        .expect("an empty source cannot fail to pack")
}
