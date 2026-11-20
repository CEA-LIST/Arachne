//! EC.M1h: `describe` reads a package referenced before its definition (`#//pkg/X` ahead of `pkg`)
//! also when the file names another .ecore file, in the root file and in every dependency file it
//! reads (spec: cea-cdrt-knowledge/spec/items/EC.M1h.md).
//!
//! Each case `cN` has a `forward/` folder and a `reordered/` twin whose files hold the same
//! classifiers with every package defined before it is referenced. EMF reads both as one
//! metamodel, and describe already reads the reordered one, so the reference is the twin: stdout,
//! stderr, and with `-o` the descriptor and every `dependencies/` file, byte for byte. The
//! fixtures keep the imported classes and their closure in their package's root (EC.M1h's notes
//! put the subpackage reads of the descriptor out of scope).
//!
//! EC.M1i (spec: cea-cdrt-knowledge/spec/items/EC.M1i.md): in each case a written class (in the root
//! or in a dependency describe writes) named a subpackage class, so the twins agreed on descriptors
//! that were not closed. They now refuse alike, and each twin claim holds on a closed variant
//! (`fixtures/ecm1i/closed/cN`) whose forward reference sits in class `early/H`, which describe does
//! not write.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ecm1h")
        .join(rel)
}

fn closed(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ecm1i/closed")
        .join(rel)
}

fn describe(input: &Path, out: Option<&Path>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_arachne"));
    cmd.arg("describe").arg(input);
    if let Some(out) = out {
        cmd.arg("-o").arg(out.join("metamodel.json"));
    }
    cmd.output().expect("run arachne describe")
}

fn scratch(label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "ecm1h-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Every file under `dir`, by relative path.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut all = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().to_string_lossy().into_owned();
                all.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    all
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Describe reads the closed `<case>/forward/root.ecore` exactly as `<case>/reordered/root.ecore`;
/// returns the forward run's `-o` folder.
fn reads_as_reordered(case: &str) -> PathBuf {
    let fwd = closed(&format!("{case}/forward/root.ecore"));
    let reo = closed(&format!("{case}/reordered/root.ecore"));
    let want = describe(&reo, None);
    assert!(
        want.status.success(),
        "{case}: the reordered twin must describe"
    );
    let got = describe(&fwd, None);
    assert!(
        got.status.success(),
        "{case}: describe should read the forward root, got: {}",
        text(&got.stderr)
    );
    assert_eq!(text(&got.stdout), text(&want.stdout), "{case}: stdout");
    assert_eq!(text(&got.stderr), text(&want.stderr), "{case}: stderr");

    let (dw, dg) = (
        scratch(&format!("{case}-reo")),
        scratch(&format!("{case}-fwd")),
    );
    let want = describe(&reo, Some(&dw));
    let got = describe(&fwd, Some(&dg));
    assert!(
        want.status.success() && got.status.success(),
        "{case}: -o runs"
    );
    assert_eq!(text(&got.stderr), text(&want.stderr), "{case}: -o stderr");
    let (fw, fg) = (files(&dw), files(&dg));
    assert_eq!(
        fg.keys().collect::<Vec<_>>(),
        fw.keys().collect::<Vec<_>>(),
        "{case}: -o file names"
    );
    for (name, bytes) in &fw {
        assert_eq!(text(&fg[name]), text(bytes), "{case}: -o {name}");
    }
    dg
}

fn describe_refuses(rel: &str) {
    let out = describe(&fixture(rel), None);
    assert!(!out.status.success(), "describe should refuse {rel}");
    assert!(out.stdout.is_empty(), "no descriptor for {rel}");
}

#[test]
fn c1_forward_root_with_a_plain_dependency() {
    reads_as_reordered("c1");
}

#[test]
fn c2_plain_root_with_a_forward_dependency_whose_digest_is_its_own_describe() {
    let dir = reads_as_reordered("c2");
    let alone = describe(&closed("c2/forward/dep.ecore"), None);
    assert!(alone.status.success(), "c2: the dependency describes alone");
    let id: serde_json::Value = serde_json::from_slice(&alone.stderr).expect("identity line");
    let digest = id["digest"].as_str().unwrap();
    let written = fs::read(dir.join(format!("dependencies/{digest}.metamodel.json")))
        .expect("c2: the dependency is written under the digest describe gives it alone");
    assert_eq!(text(&written), text(&alone.stdout), "c2: dependency bytes");
}

#[test]
fn c3_forward_root_with_a_forward_dependency() {
    reads_as_reordered("c3");
}

#[test]
fn c4_nested_forward_package_holding_the_cross_file_reference() {
    reads_as_reordered("c4");
}

#[test]
fn c5_transitive_root_mid_with_a_forward_package_leaf() {
    reads_as_reordered("c5");
}

#[test]
fn c6_cross_file_supertype_beside_a_forward_package() {
    reads_as_reordered("c6");
}

#[test]
fn the_open_twins_refuse_alike_as_not_closed() {
    for case in ["c1", "c2", "c3", "c4", "c5", "c6"] {
        let fwd = fixture(&format!("{case}/forward/root.ecore"));
        let reo = fixture(&format!("{case}/reordered/root.ecore"));
        let (got, want) = (describe(&fwd, None), describe(&reo, None));
        assert!(
            !got.status.success() && !want.status.success(),
            "{case}: both twins refuse"
        );
        assert!(
            got.stdout.is_empty() && want.stdout.is_empty(),
            "{case}: no descriptor"
        );
        // The parser's position lines (`at ..., line N, column M`) name each twin's own lines.
        let unplaced = |o: &Output| -> Vec<String> {
            text(&o.stderr)
                .lines()
                .filter(|l| !l.trim_start().starts_with("at `"))
                .map(str::to_owned)
                .collect()
        };
        assert_eq!(unplaced(&got), unplaced(&want), "{case}: stderr");
        assert!(
            text(&got.stderr).contains("descriptor not closed"),
            "{case}: {}",
            text(&got.stderr)
        );
        let dir = scratch(&format!("{case}-open"));
        assert!(!describe(&fwd, Some(&dir)).status.success());
        assert!(files(&dir).is_empty(), "{case}: -o wrote a file");
    }
}

#[test]
fn refusals_stay_explicit_through_the_resource_path() {
    for rel in [
        "refuse/twice.ecore",            // forward package defined twice
        "refuse/missing.ecore",          // #//sub/Missing, sub never defines it
        "refuse/ghost.ecore",            // #//ghost/X, ghost never defined
        "refuse/dep_missing_root.ecore", // the dependency's forward class never defined
        "refuse/pkg_x_forward.ecore",    // dep#//pkg/X never falls back to the dependency's root X
        "refuse/pkg_x_reordered.ecore",
        "refuse/a_b_x.ecore", // dep#//a/b/X
    ] {
        describe_refuses(rel);
    }
}
