//! Every crate ships `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE` as copies of the
//! files at the root of the repository (LEG-01). Copies instead of symlinks, so the
//! packages are right on every platform; this test keeps them identical.

use std::path::Path;

#[test]
fn license_files_are_identical_to_the_root_ones() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let root = crates.join("..");
    for name in ["LICENSE-MIT", "LICENSE-APACHE", "NOTICE"] {
        let expected = std::fs::read(root.join(name)).unwrap();
        for krate in ["transbank-pos", "transbank-pos-sim", "transbank-pos-cli"] {
            let path = crates.join(krate).join(name);
            let actual = std::fs::read(&path).unwrap();
            assert!(
                actual == expected,
                "{} differs from the root {name}",
                path.display()
            );
        }
    }
}
