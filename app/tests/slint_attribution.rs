//! Slint's royalty-free licence requires its "Made with Slint" badge on a
//! public page where Couchfin can be downloaded
//! (docs/decisions/003-slint-licence.md). The README carries it, and
//! release.yml appends `.github/release-footer.md` to every GitHub Release's
//! notes, so removing either fails this test.

use std::fs;
use std::path::Path;

/// The badge image the licence links to, in Slint's own repository.
const BADGE: &str =
    "https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png";

fn read(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(file);
    fs::read_to_string(&path).expect("file is readable")
}

#[test]
fn readme_shows_the_slint_badge() {
    assert!(
        read("README.md").contains(BADGE),
        "README.md must show the Made with Slint badge"
    );
}

#[test]
fn release_notes_show_the_slint_badge() {
    assert!(
        read(".github/release-footer.md").contains(BADGE),
        ".github/release-footer.md must show the Made with Slint badge"
    );
    assert!(
        read(".github/workflows/release.yml").contains("cat .github/release-footer.md >> notes.md"),
        "release.yml must append .github/release-footer.md to the release notes"
    );
}
