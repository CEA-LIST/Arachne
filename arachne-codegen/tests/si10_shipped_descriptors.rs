//! SI.10 (`cea-cdrt-knowledge/spec/items/SI.10.md`), claim C1: the descriptors Arachne ships are
//! what `arachne describe` writes for their `.ecore`, byte for byte, so every in-scope reference
//! (no `eOpposite`) carries SI.05's `identityReference` and a reference with an `eOpposite` keeps
//! today's `reference`; nothing else of them moves; the digest fixture names them; the
//! workspace's `demo/sysml.json` is the shipped SysML descriptor and the one it replaces is kept
//! beside it; and the generated crates' descriptors (RQ1's baseline, not shipped) keep their bytes.
//!
//! Ground truth, none of it computed by the emitter under test:
//! - the descriptors as shipped before SI.10, frozen under `tests/fixtures/si10/legacy/` (file
//!   sha256 registered below, the bytes `examples/` held at Arachne `25ede18`), moved by SI.05's
//!   rule as `si05_support::identity_rule` states it (one key read, `eOpposite`, no class or
//!   feature name);
//! - the counts of moved and kept references, read by hand from the legacy files (spec section 4);
//! - the digests and file sha256 below, computed at spec time with Python's `json` and `hashlib`
//!   from the moved legacy files (compact, keys sorted, UTF-8; and `json.dumps(indent=2)` is not
//!   used: the file bytes are the emitter's `{:#}` and a newline, checked against D2's render).
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-codegen --test si10_shipped_descriptors -- --nocapture
//! ```

#[path = "si05_support/mod.rs"]
mod si05_support;

use std::{
    fs,
    path::{Path, PathBuf},
};

use arachne_codegen::{EcoreParser, descriptor_json, find_user_package, metamodel_digest};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use si05_support::{render, with_identity_rule};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The cea-crdt workspace root, where `demo/` sits outside every repository.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn describe(ecore: &Path) -> Value {
    let parser = EcoreParser::from_file(ecore)
        .unwrap_or_else(|e| panic!("set-up: {} should parse: {e}", ecore.display()));
    let pack = find_user_package(&parser.ctx)
        .unwrap_or_else(|e| panic!("set-up: {} has no user package: {e}", ecore.display()));
    descriptor_json(&parser.ctx, pack)
        .unwrap_or_else(|e| panic!("set-up: describing {}: {e}", ecore.display()))
}

fn read_bytes(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|e| panic!("set-up: reading {}: {e}", path.display()))
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&read_bytes(path))
        .unwrap_or_else(|e| panic!("set-up: {} is not JSON: {e}", path.display()))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// One shipped descriptor.
struct Shipped {
    /// Under `examples/`.
    file: &'static str,
    /// The `.ecore` it is described from, under `examples/`.
    ecore: &'static str,
    /// The frozen legacy copy, under `arachne-codegen/tests/fixtures/si10/legacy/`, or `None`
    /// where the descriptor has no reference the rule moves (json).
    legacy: Option<&'static str>,
    /// sha256 of the legacy file (the bytes `examples/` held at `25ede18`).
    legacy_sha: &'static str,
    /// Its metamodel digest.
    legacy_digest: &'static str,
    /// sha256 of the regenerated file.
    new_sha: &'static str,
    /// Its metamodel digest (SI.05's registered digests, `si05_identity_reference_emitter.rs`).
    new_digest: &'static str,
    /// Reference entries on `identityReference` after SI.10, read by hand.
    identity: usize,
    /// Reference entries left on `reference`, every one declaring an `eOpposite`, read by hand.
    kept: usize,
}

const SHIPPED: [Shipped; 6] = [
    Shipped {
        file: "bt.metamodel.json",
        ecore: "behavior_tree.ecore",
        legacy: Some("bt.b899ec33.metamodel.json"),
        legacy_sha: "ff4525108d4ecb65d297002c7bc02f22978966d347af3678712de07a5c6e1263",
        legacy_digest: "b899ec335e7dc46303d5a7871a1370a12db8858718bea57dc6f58f01c474feb5",
        new_sha: "d3b20675a5df3f62e824b25467f00840ed506171e300c4b1aa0e787df995bd4e",
        new_digest: "f19f730e4aaa7320d23e6b211a7856e7aa9042b56cef67cb3c0cb36091ad6822",
        identity: 1,
        kept: 0,
    },
    Shipped {
        file: "uml.metamodel.json",
        ecore: "SimpleUML.ecore",
        legacy: Some("uml.21c50dea.metamodel.json"),
        legacy_sha: "b000aa0d2b562a41763d3e77829fd9363224fdcade914d42846b9cb0c54690a5",
        legacy_digest: "21c50dea71938b861233b72116f5ead094fe1bbb3d67bf623864f52c3ead1c3f",
        new_sha: "8c50a7a89aedad54fd19c6939798ddf7ef1c3b7c4a7444a2ddf467748dc4c727",
        new_digest: "5d34af887ace294f008c96403a4e6dd442f374d4e23639ec1620c04a4cf301f3",
        identity: 4,
        kept: 2,
    },
    Shipped {
        file: "notation.metamodel.json",
        ecore: "notation.ecore",
        legacy: Some("notation.12e7d2be.metamodel.json"),
        legacy_sha: "04ebb5c15dad375284d4e5e681a5e634130525c614517d373763e6040eead03f",
        legacy_digest: "12e7d2beab5a6d0a44090ed0512228c170a19879a7ac25093d817d58fe5c4de1",
        new_sha: "dcf9b2f02cf7066a5a51a7062c0cd019d585bec901cafc82149760ad93a1b040",
        new_digest: "14377158866c6dd249915cdadbc5579efb64a3e742b5bd7aff1ced4396f57b86",
        identity: 2,
        kept: 0,
    },
    Shipped {
        file: "umlclass.metamodel.json",
        ecore: "umlclass.ecore",
        legacy: Some("umlclass.a44920ad.metamodel.json"),
        legacy_sha: "a770d938ae503aff62f395b6f3cf241a4776459da5b2a04d5ba5e3cddc892edd",
        legacy_digest: "a44920adc661d308b7bbb9a8e4ccf1549a093546a241c0e33b28596de8000f3b",
        new_sha: "9600cc395935773caa15518c3a3e1d337aaccfa31f1d5073deae53dc2064ec79",
        new_digest: "999ed54f0657c2c8e27162645b0676e94be56a16d9058429707a104d1e620dc9",
        identity: 8,
        kept: 2,
    },
    Shipped {
        file: "json.metamodel.json",
        ecore: "json.ecore",
        legacy: None,
        legacy_sha: "efdbde163d711b39e102ae7dc98585c3cb981ad9be82914ba4f4270e10113ec1",
        legacy_digest: "a9d83505175d8d6708b86c7eb64d356d64197f066f752c1a9ea842c76a45bc30",
        new_sha: "efdbde163d711b39e102ae7dc98585c3cb981ad9be82914ba4f4270e10113ec1",
        new_digest: "a9d83505175d8d6708b86c7eb64d356d64197f066f752c1a9ea842c76a45bc30",
        identity: 0,
        kept: 0,
    },
    Shipped {
        file: "sysml/sysml.metamodel.json",
        ecore: "sysml/sysml.ecore",
        legacy: Some("sysml.2df00538.metamodel.json"),
        legacy_sha: "896ba4d758c6d8adfe9764bb7779f3400212c02afd97959012936d4e113d4e47",
        legacy_digest: "2df00538ec6fc36639611b6e57d0926e8422bce4aa7a17d4083bdbcd1f761457",
        new_sha: "8f41862402d936e54b9d6de9cd0ccd6e290ea0adaa8d0331ba21c3c13f7a1f83",
        new_digest: "a75d8af47bf99f51b908f909a468c75c629a505de80b5379f19085943d6ae066",
        identity: 34,
        kept: 2,
    },
];

fn legacy_path(name: &str) -> PathBuf {
    repo()
        .join("arachne-codegen/tests/fixtures/si10/legacy")
        .join(name)
}

/// `(identityReference entries, reference entries without eOpposite, reference entries with one)`.
fn census(descriptor: &Value) -> (usize, usize, usize) {
    let (mut identity, mut bare, mut opposite) = (0, 0, 0);
    for body in descriptor["classes"].as_object().expect("classes").values() {
        for entry in body["references"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            match entry["merge"]["kind"].as_str() {
                Some("identityReference") => identity += 1,
                Some("reference") if entry.get("eOpposite").is_some() => opposite += 1,
                Some("reference") => bare += 1,
                _ => {}
            }
        }
    }
    (identity, bare, opposite)
}

/// D0, the premise, green before and after: the frozen legacy copies are the registered bytes,
/// and their digests the registered ones.
#[test]
fn si10_d0_premise_the_legacy_copies_are_the_registered_bytes() {
    for s in SHIPPED.iter() {
        let Some(name) = s.legacy else { continue };
        let path = legacy_path(name);
        assert_eq!(
            sha256(&read_bytes(&path)),
            s.legacy_sha,
            "set-up: {name} is not the registered legacy file"
        );
        assert_eq!(
            metamodel_digest(&read_json(&path)),
            s.legacy_digest,
            "set-up: {name} does not carry the registered legacy digest"
        );
    }
}

/// D1. Each shipped descriptor is, byte for byte, what `arachne describe` writes for its `.ecore`
/// (`{descriptor:#}` and a newline), with the registered sha256, digest and reference census.
#[test]
fn si10_d1_each_shipped_descriptor_is_what_describe_writes_byte_for_byte() {
    let mut wrong = Vec::new();
    for s in SHIPPED.iter() {
        let path = repo().join("examples").join(s.file);
        let bytes = read_bytes(&path);
        let described = render(&describe(&repo().join("examples").join(s.ecore)));
        let checked_in: Value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|e| panic!("set-up: examples/{} is not JSON: {e}", s.file));
        let (identity, bare, opposite) = census(&checked_in);
        println!(
            "SI10-D1 examples/{}: sha256 {}, digest {}, identityReference {identity}, reference \
             without eOpposite {bare}, with one {opposite}; equal to describe: {}",
            s.file,
            sha256(&bytes),
            metamodel_digest(&checked_in),
            bytes == described.as_bytes()
        );
        if bytes != described.as_bytes() {
            wrong.push(format!(
                "examples/{} is not what `arachne describe examples/{}` writes",
                s.file, s.ecore
            ));
        }
        if sha256(&bytes) != s.new_sha {
            wrong.push(format!(
                "examples/{}: sha256 {}, registered {}",
                s.file,
                sha256(&bytes),
                s.new_sha
            ));
        }
        if metamodel_digest(&checked_in) != s.new_digest {
            wrong.push(format!(
                "examples/{}: digest {}, registered {}",
                s.file,
                metamodel_digest(&checked_in),
                s.new_digest
            ));
        }
        if (identity, bare, opposite) != (s.identity, 0, s.kept) {
            wrong.push(format!(
                "examples/{}: {identity} identityReference, {bare} reference without eOpposite, \
                 {opposite} with one; registered {}, 0, {}",
                s.file, s.identity, s.kept
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "SI10 claim C1, the shipped descriptors:\n  {}",
        wrong.join("\n  ")
    );
}

/// D2. Each regenerated descriptor is the legacy one moved by SI.05's rule and by nothing else,
/// rendered as the emitter renders; the rule moves exactly the registered number of entries.
#[test]
fn si10_d2_each_shipped_descriptor_is_the_legacy_one_moved_by_the_rule_alone() {
    let mut wrong = Vec::new();
    for s in SHIPPED.iter() {
        let Some(name) = s.legacy else { continue };
        let legacy = read_json(&legacy_path(name));
        let mut moved = legacy.clone();
        let count = si05_support::identity_rule(&mut moved);
        assert_eq!(
            count, s.identity,
            "set-up: the rule moves {count} entries of {name}, read by hand {}",
            s.identity
        );
        assert_eq!(
            moved,
            with_identity_rule(&legacy),
            "set-up: the rule is a function"
        );
        let expected = render(&moved);
        assert_eq!(
            sha256(expected.as_bytes()),
            s.new_sha,
            "set-up: {name} moved by the rule does not have the registered sha256"
        );
        let shipped = read_bytes(&repo().join("examples").join(s.file));
        if shipped != expected.as_bytes() {
            wrong.push(format!(
                "examples/{} is not {name} moved by the identity rule alone",
                s.file
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "SI10 claim C1, nothing but the rule moves:\n  {}",
        wrong.join("\n  ")
    );
}

/// The digest fixture after SI.10: today's text with the three moved digests replaced (json's
/// does not move), as `sed` wrote it at spec time.
const DIGEST_FIXTURE_SHA: &str = "9e4a4f24bc3f5485a1b54726cd2645a8ddf18c0f0bd55a539b4fb7dd2c277efe";

/// D3. `examples/fixtures/metamodel-digests.json` (read by the editor's mp6 and the generated
/// crate's mp13) names the shipped descriptors by their new digests, and is the registered bytes.
#[test]
fn si10_d3_the_digest_fixture_names_the_shipped_descriptors() {
    let path = repo().join("examples/fixtures/metamodel-digests.json");
    let fixture = read_json(&path);
    let mut wrong = Vec::new();
    for (file, digest) in [
        ("bt.metamodel.json", SHIPPED[0].new_digest),
        ("uml.metamodel.json", SHIPPED[1].new_digest),
        ("json.metamodel.json", SHIPPED[4].new_digest),
        ("notation.metamodel.json", SHIPPED[2].new_digest),
    ] {
        let shipped = read_json(&repo().join("examples").join(file));
        let want = json!({"nsURI": shipped["nsURI"], "digest": digest});
        if fixture[file] != want {
            wrong.push(format!(
                "the fixture entry for {file} is {}, not {want}",
                fixture[file]
            ));
        }
    }
    let sha = sha256(&read_bytes(&path));
    if sha != DIGEST_FIXTURE_SHA {
        wrong.push(format!(
            "examples/fixtures/metamodel-digests.json: sha256 {sha}, registered {DIGEST_FIXTURE_SHA}"
        ));
    }
    assert!(
        wrong.is_empty(),
        "SI10 claim C1, the digest fixture:\n  {}",
        wrong.join("\n  ")
    );
}

/// D4. The workspace's `demo/sysml.json`, which the editor's SysML tests, ED.12 and the bench
/// read by default, is the shipped SysML descriptor byte for byte; the descriptor it replaces is
/// kept as `demo/sysml-2df00538.json` for the models created under it, and the two earlier ones
/// keep their bytes.
#[test]
fn si10_d4_the_workspace_sysml_descriptor_is_the_shipped_one_and_the_previous_is_kept() {
    let demo = workspace().join("demo");
    let shipped = read_bytes(&repo().join("examples/sysml/sysml.metamodel.json"));
    let mut wrong = Vec::new();
    match fs::read(demo.join("sysml.json")) {
        Ok(bytes) if bytes == shipped => {}
        Ok(bytes) => wrong.push(format!(
            "demo/sysml.json has sha256 {}, not the shipped examples/sysml/sysml.metamodel.json's {}",
            sha256(&bytes),
            sha256(&shipped)
        )),
        Err(e) => wrong.push(format!("demo/sysml.json: {e}")),
    }
    for (name, sha) in [
        (
            "sysml-2df00538.json",
            "896ba4d758c6d8adfe9764bb7779f3400212c02afd97959012936d4e113d4e47",
        ),
        (
            "sysml-ebd58069.json",
            "ae01acb7a2d201cc5262ca3bbd5e4501eb63217e915d0534b2f9ce15d24ddaea",
        ),
        (
            "sysml-5168d5f6.json",
            "1e6a71e5960a4f7f9d216300e748cf9b7e5f592d08a11becaab958a48b10b57a",
        ),
    ] {
        match fs::read(demo.join(name)) {
            Ok(bytes) if sha256(&bytes) == sha => {}
            Ok(bytes) => wrong.push(format!(
                "demo/{name}: sha256 {}, registered {sha}",
                sha256(&bytes)
            )),
            Err(e) => wrong.push(format!("demo/{name}: {e}")),
        }
    }
    assert!(
        wrong.is_empty(),
        "SI10 claim C1, the workspace SysML descriptor:\n  {}",
        wrong.join("\n  ")
    );
}

/// D5. The boundary (spec, scoping decision B1): the generated crates' descriptors are not
/// shipped; they are the descriptors RQ1's twin suites run their interpreted arm on, and this
/// item does not rerun RQ1. They keep their bytes.
#[test]
fn si10_d5_the_generated_crates_keep_their_descriptors() {
    let mut wrong = Vec::new();
    for (krate, sha) in [
        (
            "bt_crdt",
            "ff4525108d4ecb65d297002c7bc02f22978966d347af3678712de07a5c6e1263",
        ),
        (
            "classdiagram_crdt",
            "f3e8b8270afc23f8e3ea570c8f6b64abfe41b4bb1bbaeb27141d801e23b1990a",
        ),
        (
            "json_crdt",
            "efdbde163d711b39e102ae7dc98585c3cb981ad9be82914ba4f4270e10113ec1",
        ),
        (
            "kitchen_crdt",
            "384d4bf4b4891fe5af9780d5434947cec3a818fef02da63bdb11b7866ac41774",
        ),
        (
            "simpleuml_crdt",
            "4e52b38504c9db56def0753483967a86059104b3e245490a87f672bb2fb8467a",
        ),
    ] {
        let got = sha256(&read_bytes(
            &repo().join("generated").join(krate).join("metamodel.json"),
        ));
        if got != sha {
            wrong.push(format!(
                "generated/{krate}/metamodel.json: sha256 {got}, registered {sha}"
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "SI10 boundary B1, the generated crates:\n  {}",
        wrong.join("\n  ")
    );
}

/// Every file under `examples/` after SI.10, with its sha256: the five moved descriptors and the
/// digest fixture at their new bytes, every other file at the bytes it held at `25ede18`
/// (`sha256sum`, spec time).
const EXAMPLES_AFTER: [(&str, &str); 25] = [
    (
        "GENERIC_NETWORK_GUIDE.md",
        "01ab74a6ad9ea4e0f7cf0465b10661f8953d095eafef3bb87d684ccc9c15adac",
    ),
    (
        "README.md",
        "3463d7c5a2c6374e6bad364137eb89c0d838999b7da85307bbf50a9077efe3f5",
    ),
    (
        "SimpleUML.ecore",
        "31296010f3cde494863c4d55b4fd0d1adaf56023dc07ccc6d694c6bec0be411c",
    ),
    (
        "UML.ecore",
        "421e95a6b1946e9705a136011a0fc02aa7bf9b2871cba3e83bf11f11234176b6",
    ),
    (
        "behavior_tree.ecore",
        "d658b5c5b7a71dbdc49dda87d20a151c25336aabc16cd2c38b5b05f41dd663d5",
    ),
    (
        "bt.metamodel.json",
        "d3b20675a5df3f62e824b25467f00840ed506171e300c4b1aa0e787df995bd4e",
    ),
    (
        "class_diagram.ecore",
        "b8645c0fb97cbe9debed4af2937b2e63ac71ab0b18bd23542aca611267a6a05b",
    ),
    (
        "class_hierarchy.ecore",
        "bc1d7f1a40522d9905e6576caf72422a87f42c307a936084f6fdeb5e3cdbf7d3",
    ),
    (
        "conference.ecore",
        "4b30cbcf9411caa86ca4a6e74ef835b9944747d319fa3a6440b5d505741a0346",
    ),
    ("fixtures/metamodel-digests.json", DIGEST_FIXTURE_SHA),
    (
        "json.ecore",
        "fd88ee6b27546e51d9948fccb709ba95e082a0ed343b171aae1e71a94adc4b9c",
    ),
    (
        "json.metamodel.json",
        "efdbde163d711b39e102ae7dc98585c3cb981ad9be82914ba4f4270e10113ec1",
    ),
    (
        "notation.ecore",
        "bad29f0e27d54e8ec5b12934eda20b169a559af5ba511dfa372eb6aa05f0d9b0",
    ),
    (
        "notation.metamodel.json",
        "dcf9b2f02cf7066a5a51a7062c0cd019d585bec901cafc82149760ad93a1b040",
    ),
    (
        "pet_metamodels/abstract_inherits_concrete.ecore",
        "d11e29a90768c6d9c3226a3d8289b0158733972b99bfd8c8daa41a1bfeb1b6e6",
    ),
    (
        "pet_metamodels/concrete_inherits_concrete.ecore",
        "b2277e3bfa6951675e0575fddf6548175ed17c83ff275044ec944ceb2198be57",
    ),
    (
        "pet_metamodels/concrete_polymorphic_targets.ecore",
        "16dba6831820fee59f1072c029696e674de623c04b781d0fc98982d548aa89d8",
    ),
    (
        "pet_metamodels/ecore_builtins.ecore",
        "108465fab2d43a002c81a49fcf2f6dc1b4aa0da3fa6a7f64059e9704d1c33149",
    ),
    (
        "pet_metamodels/kitchen_sink.ecore",
        "dd765685c7072f6f0250c70bba8d1214d37250c5cb5af114d9e93e2c9c7d4a1b",
    ),
    (
        "pet_metamodels/multiple_inheritance.ecore",
        "ce39e5d5a6566e814588e44d2f81028bdc1226f5ae3b7937e8292d14cda7b7bc",
    ),
    (
        "sysml/sysml.ecore",
        "f4265e5a3fb2d8440e705e847744ebb8ec99c17af6403a2baf3aa16033f645e2",
    ),
    (
        "sysml/sysml.metamodel.json",
        "8f41862402d936e54b9d6de9cd0ccd6e290ea0adaa8d0331ba21c3c13f7a1f83",
    ),
    (
        "umlclass.ecore",
        "d2baec8b4f99d36d6a5b53420f31ade7422160c64cba4c87d9e7cebed9618437",
    ),
    (
        "umlclass.metamodel.json",
        "9600cc395935773caa15518c3a3e1d337aaccfa31f1d5073deae53dc2064ec79",
    ),
    (
        "uml.metamodel.json",
        "8c50a7a89aedad54fd19c6939798ddf7ef1c3b7c4a7444a2ddf467748dc4c727",
    ),
];

/// Every file under `dir`, relative to it with `/` separators.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap_or_else(|e| panic!("listing {}: {e}", d.display())) {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(
                    path.strip_prefix(dir)
                        .expect("under the directory")
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/"),
                );
            }
        }
    }
    out.sort();
    out
}

/// D6. `examples/` after SI.10 is exactly the registered list: the five moved descriptors and the
/// digest fixture at their new bytes, json's descriptor and every other file unchanged, and no
/// file added or removed.
#[test]
fn si10_d6_examples_holds_exactly_the_registered_files() {
    let dir = repo().join("examples");
    let listed = files_under(&dir);
    let mut registered: Vec<String> = EXAMPLES_AFTER.iter().map(|(f, _)| f.to_string()).collect();
    registered.sort();
    let mut wrong = Vec::new();
    if listed != registered {
        wrong.push(format!(
            "examples/ lists {listed:?}, registered {registered:?}"
        ));
    }
    for (file, sha) in EXAMPLES_AFTER {
        let path = dir.join(file);
        if !path.exists() {
            continue;
        }
        let got = sha256(&read_bytes(&path));
        if got != sha {
            wrong.push(format!("examples/{file}: sha256 {got}, registered {sha}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "SI10 claim C1, examples/ after the item:\n  {}",
        wrong.join("\n  ")
    );
}
