//! MD.04 validator-owned oracle. See vault spec/items/MD.04.md.
#[path = "md04_common.rs"]
mod common;
use common::*;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};

fn survey() -> Source {
    Source::new(&fixture("survey-aliases.project.json"))
}
fn literal() -> Value {
    read(&fixture("survey.identified.expected.json"))
}
fn visit(v: &mut Value, f: &mut impl FnMut(&mut Value)) {
    f(v);
    match v {
        Value::Object(m) => {
            for v in m.values_mut() {
                visit(v, f)
            }
        }
        Value::Array(a) => {
            for v in a {
                visit(v, f)
            }
        }
        _ => {}
    }
}
fn plain(mut v: Value) -> Value {
    visit(&mut v, &mut |v| {
        if let Value::Object(m) = v {
            m.remove("__id");
        }
    });
    v
}

#[test]
fn md04_instrument_census_and_pins() {
    let s = survey();
    assert_eq!(
        s.census(),
        Census {
            objects: 33,
            elements: 32,
            local: 17,
            library: 1,
            dangling: 0,
            aliases: 1,
            alias_values: 3,
            sysml: 1,
            kerml: 0
        }
    );
    let b = Source::new(&batmobile());
    assert_eq!(
        b.census(),
        Census {
            objects: 742,
            elements: 741,
            local: 251,
            library: 27,
            dangling: 0,
            aliases: 9,
            alias_values: 9,
            sysml: 25,
            kerml: 2
        }
    );
    let pins = read(&fixture("pins.json"));
    assert_eq!(hash(&descriptor_path()), pins["sysml_sha256"]);
    assert_eq!(hash(&fixture("gauge.project.json")), pins["gauge_sha256"]);
    for good in [
        "sysml:DataType kermllibrary:///b2c6dd37-2084-3ce4-9ce2-580fdf30629c#b352ded6-38ba-4e7e-ae55-9299cb0cc6d0",
        "ScalarValues::Real",
        "SI::'watt hour'",
        "BaseFunctions::'#'",
        "A::'x::y'",
        "A::'can\\'t'",
    ] {
        assert!(library_value(good), "MD04 library grammar: {good}");
    }
    for bad in [
        "5ee1dead",
        "md04-alice@1.0",
        "ScalarValues",
        "A::",
        "A::::B",
        "A::''",
        "A::'unfinished",
        "A::B#suffix",
        " A::B",
        "A::B ",
        "sysml:DataType kermllibrary:///missing#missing",
        "04040404040404040404040404040404#alice@1.0",
    ] {
        assert!(!library_value(bad), "MD04 library grammar: {bad}");
    }
    let dangling = Source::new(&fixture("dangling-references.project.json"));
    assert_eq!(
        dangling.census(),
        Census {
            objects: 5,
            elements: 5,
            local: 1,
            library: 2,
            dangling: 1,
            sysml: 1,
            ..Census::default()
        }
    );
    assert_eq!(
        dangling.dangling(),
        vec![Dangling {
            holder: "md04-dangling-typing".into(),
            feature: "type".into(),
            value: "5ee1dead".into()
        }]
    );
    // Inspecting the pinned fixture is not execution of the GAUGE load step.
    assert_eq!(
        Source::new(&fixture("gauge.project.json")).dangling(),
        vec![Dangling {
            holder: "82087f5f-fdb2-4ff8-be00-51a29ace8cce".into(),
            feature: "type".into(),
            value: "5ee1dead".into()
        }]
    );
    // The relay parser must also carry deep model operation trees unchanged.
    let deep = format!("{}0{}", "[".repeat(220), "]".repeat(220));
    assert_eq!(parse(&deep).to_string(), deep);
}
#[test]
fn md04_instrument_literal_state_and_three_bad_cases() {
    let s = survey();
    let good = literal();
    let ids = Source::served(&good).unwrap();
    assert_eq!(
        Source::all_identities(&good).len(),
        33,
        "MD04 instrument: anonymous object is included"
    );
    s.check(&good, true, false, &ids);
    s.check(&plain(good.clone()), false, false, &ids);
    let map = s.il2(&ids);
    let mut bad = good.clone();
    let mut changes = 0;
    visit(&mut bad, &mut |v| {
        if let Value::Object(m) = v {
            if let Some(class) = m.get("eClass").and_then(Value::as_str).map(str::to_owned) {
                for (name, field) in s.table.features(&class) {
                    if field["merge"]["kind"] == "identityReference" {
                        if let Some(value) = m.get_mut(&name) {
                            visit(value, &mut |v| {
                                if let Value::String(text) = v {
                                    if let Some(next) = map.get(text) {
                                        if text != next {
                                            changes += 1;
                                            *text = next.clone();
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }
    });
    assert_eq!(
        Source::served(&bad).unwrap(),
        ids,
        "MD04 instrument: IL2 keeps served identities"
    );
    assert!(
        changes > 0,
        "MD04 instrument: IL2 must move class-compatible targets"
    );
    assert!(
        s.compare(&bad, true, false, &ids).is_err(),
        "MD04 instrument missed IL2"
    );
    let mut bad = good.clone();
    visit(&mut bad, &mut |v| {
        if v.as_str().is_some_and(|s| s.contains("sysmllibrary:///")) {
            *v = Value::Null
        }
    });
    assert!(
        s.compare(&bad, true, false, &ids).is_err(),
        "MD04 instrument missed dropped library"
    );
    let mut bad = good.clone();
    bad["aliasIds"] = json!([]);
    assert!(
        s.compare(&bad, true, false, &ids).is_err(),
        "MD04 instrument missed dropped aliases"
    );
    let mut bad = good.clone();
    bad["aliasIds"] = json!(["md04:alpha", "md04:middle", "md04:zeta"]);
    assert!(
        s.compare(&bad, true, false, &ids).is_err(),
        "MD04 instrument missed reordered aliases"
    );
    let mut bad = good.clone();
    bad["ownedRelationship"][0]["ownedRelatedElement"]
        .as_array_mut()
        .unwrap()
        .push(good.clone());
    assert!(
        s.compare(&bad, true, false, &ids).is_err(),
        "MD04 instrument missed duplicate structure"
    );
    let s = Source::new(&fixture("dangling-references.project.json"));
    let good = read(&fixture("dangling.identified.expected.json"));
    let ids = Source::served(&good).unwrap();
    s.check(&good, true, false, &ids);
    s.check(&plain(good.clone()), false, false, &ids);
    let mut bad = good.clone();
    bad["ownedRelationship"][0]["type"] = json!("5ee1dead");
    assert!(
        s.compare(&bad, true, false, &ids).is_err(),
        "MD04 instrument missed dangling read-back"
    );
    for index in 1..4 {
        let mut bad = good.clone();
        bad["ownedRelationship"][index]["type"] = Value::Null;
        assert!(
            s.compare(&bad, true, false, &ids).is_err(),
            "MD04 instrument lost retained reference {index}"
        );
    }
}
#[test]
fn md04_offline_legacy_batch_and_report_are_frozen() {
    let output = Command::new(env!("CARGO_BIN_EXE_arachne"))
        .arg("import-sirius")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ed12/survey.project.json"))
        .arg("--descriptor")
        .arg(root().join("demo/sysml-2df00538.json"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        fs::read(fixture("offline.stdout.json")).unwrap(),
        "MD04 G0: offline batch changed"
    );
    assert_eq!(
        output.stderr,
        fs::read(fixture("offline.stderr.txt")).unwrap(),
        "MD04 G0: offline reference report changed"
    );
}
#[test]
fn md04_offline_identity_rule_still_needs_a_live_node() {
    let output = Command::new(env!("CARGO_BIN_EXE_arachne"))
        .arg("import-sirius")
        .arg(fixture("survey-aliases.project.json"))
        .arg("--descriptor")
        .arg(descriptor_path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("store identity"),
        "MD04 G0: diagnostic must explain missing identities"
    );
}
#[test]
fn md04_live_arguments_are_paired() {
    for (flag, value, missing) in [
        ("--node", "http://127.0.0.1:1", "--model"),
        ("--model", "04040404040404040404040404040404", "--node"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_arachne"))
            .arg("import-sirius")
            .arg(fixture("survey-aliases.project.json"))
            .arg("--descriptor")
            .arg(descriptor_path())
            .args([flag, value])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(missing),
            "MD04 G4: {flag} must require {missing}, got {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
fn live(path: &Path, label: &str) {
    let s = Source::new(path);
    let c = Cluster::new(s, label);
    c.pause();
    let relay = Relay::new(&c, false);
    let mut cli = Importer::start(&c, path, relay.port, &descriptor_path());
    cli.success();
    relay.assert_trace();
    c.check();
    c.resume();
    c.convergence();
    let ids = c.check();
    c.source
        .check(&parse(&state(c.port(1), &c.model, true)), true, false, &ids);
}
#[test]
fn md04_live_survey() {
    live(&fixture("survey-aliases.project.json"), "survey")
}
#[test]
fn md04_batmobile_census() {
    live(&batmobile(), "batmobile")
}
#[test]
fn md04_dangling_references_are_reported_and_not_written() {
    let path = fixture("dangling-references.project.json");
    let c = Cluster::new(Source::new(&path), "dangling");
    c.pause();
    let relay = Relay::new(&c, false);
    let mut cli = Importer::start(&c, &path, relay.port, &descriptor_path());
    cli.success();
    relay.assert_trace();
    assert!(
        relay.trace.lock().unwrap().dangling_writes.is_empty(),
        "MD04 G5: dangling reference was written as Link"
    );
    c.source.check_dangling_report(&cli.stderr());
    c.check();
    c.resume();
    c.convergence();
    let ids = c.check();
    c.source
        .check(&parse(&state(c.port(1), &c.model, true)), true, false, &ids);
    c.source.check(
        &parse(&state(c.port(1), &c.model, false)),
        false,
        false,
        &ids,
    );
}
#[test]
fn md04_paused_link_phase() {
    let path = batmobile();
    let c = Cluster::new(Source::new(&path), "paused");
    let relay = Relay::new(&c, true);
    let mut cli = Importer::start(&c, &path, relay.port, &descriptor_path());
    relay.barrier(&mut cli);
    c.convergence();
    let before = state(c.port(1), &c.model, true);
    let doc = parse(&before);
    let ids = Source::served(&doc).unwrap();
    c.source.check(&doc, true, true, &ids);
    c.pause();
    relay.release();
    cli.success();
    relay.assert_trace();
    c.check();
    assert_eq!(
        state(c.port(1), &c.model, true),
        before,
        "MD04 G2: paused bob received links"
    );
    c.resume();
    c.convergence();
    let ids = c.check();
    c.source
        .check(&parse(&state(c.port(1), &c.model, true)), true, false, &ids);
}
#[test]
fn md04_killed_after_structure_resumes() {
    let path = batmobile();
    let c = Cluster::new(Source::new(&path), "killed");
    c.pause();
    let relay = Relay::new(&c, true);
    let mut cli = Importer::start(&c, &path, relay.port, &descriptor_path());
    relay.barrier(&mut cli);
    let before = parse(&state(c.port(0), &c.model, true));
    let ids = Source::served(&before).unwrap();
    let every_identity = Source::all_identities(&before);
    assert_eq!(every_identity.len(), c.source.census().objects);
    c.source.check(&before, true, true, &ids);
    cli.kill();
    drop(cli);
    drop(relay);
    c.source
        .check(&parse(&state(c.port(0), &c.model, true)), true, true, &ids);
    let relay = Relay::new(&c, false);
    let mut restarted = Importer::start(&c, &path, relay.port, &descriptor_path());
    restarted.success();
    relay.assert_trace();
    assert_eq!(
        c.check(),
        ids,
        "MD04 G3: resume recreated source identities"
    );
    let identified = state(c.port(0), &c.model, true);
    assert_eq!(
        Source::all_identities(&parse(&identified)),
        every_identity,
        "MD04 G3: resume recreated an anonymous or named object"
    );
    let ordinary = state(c.port(0), &c.model, false);
    let mut repeated = Importer::start(&c, &path, c.port(0), &descriptor_path());
    repeated.success();
    assert_eq!(
        state(c.port(0), &c.model, true),
        identified,
        "MD04 G3: completed repeat changed identified read"
    );
    assert_eq!(
        state(c.port(0), &c.model, false),
        ordinary,
        "MD04 G3: completed repeat changed ordinary read"
    );
    c.resume();
    c.convergence();
    assert_eq!(c.check(), ids);
}
#[test]
fn md04_wrong_descriptor_is_refused_before_writing() {
    let path = fixture("survey-aliases.project.json");
    let c = Cluster::new(Source::new(&path), "mismatch");
    c.pause();
    let before = state(c.port(0), &c.model, true);
    let mut cli = Importer::start(
        &c,
        &path,
        c.port(0),
        &root().join("demo/sysml-2df00538.json"),
    );
    let status = cli.finish();
    let error = cli.stderr().to_lowercase();
    assert!(!status.success());
    assert!(
        error.contains("descriptor") && error.contains("mismatch"),
        "MD04 G4: descriptor mismatch must be named: {error}"
    );
    assert_eq!(
        state(c.port(0), &c.model, true),
        before,
        "MD04 G4: mismatch wrote structure"
    );
}
#[test]
#[ignore = "MD.04 GAUGE load step; orchestrator schedules alone, release, six independent runs"]
fn md04_gauge_load() {
    let path = fixture("gauge.project.json");
    let mut rows = Vec::new();
    for block in 0..6 {
        let s = Source::new(&path);
        let census = s.census();
        let c = Cluster::new(s, "gauge");
        c.pause();
        let start = Instant::now();
        let mut cli = Importer::start(&c, &path, c.port(0), &descriptor_path());
        cli.success();
        let seconds = start.elapsed().as_secs_f64();
        c.source.check_dangling_report(&cli.stderr());
        c.check();
        println!(
            "MD04 GAUGE block={block} seconds={seconds:.6} census={census:?} node_sha={NODE_SHA}"
        );
        rows.push(json!({"block":block,"seconds":seconds,"objects":census.objects,"local":census.local,"library":census.library}));
        c.resume();
        c.convergence();
    }
    fs::write(
        work("gauge-report").join("gauge.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
}

/// A hand-written two-object setup validates the HTTP instrument against the
/// pinned node without relying on any importer behavior or generated batch.
#[test]
fn md04_instrument_real_node_relay_and_ordered_aliases() {
    use moirai_interp::{InstanceOp as I, LeafOp as L, ModelOp as M, Scalar};
    use moirai_semantics::{ClassSlot, FeatureSlot};
    let library = "sysml:Element sysmllibrary:///de9b8ac4-d3a5-4f9a-840d-fe7afd0b7f92#42c66539-5bc6-43c6-9806-00600d1d5a66";
    let project = json!({"content":[{"id":"source-root","eClass":"sysml:Namespace","data":{"elementId":"root-id","aliasIds":["z","a"],"ownedRelationship":[{"id":"source-rel","eClass":"sysml:Subclassification","data":{"elementId":"rel-id","source":["source-root"],"target":["source-root",library]}}]}}]});
    let source = Source::from_value(project);
    let c = Cluster::new(source, "instrument");
    c.pause();
    let table = moirai_semantics::from_descriptor(&c.source.table.descriptor).unwrap();
    let class = |name: &str| {
        table
            .classes
            .iter()
            .find(|e| &*e.name == name)
            .unwrap()
            .slot
    };
    let field = |class: ClassSlot, name: &str| {
        FeatureSlot(
            table.classes[class.index()]
                .visible
                .iter()
                .position(|(n, _, _)| &**n == name)
                .unwrap() as u16,
        )
    };
    let ns = class("Namespace");
    let rel = class("Subclassification");
    let root = |op| M::Instance(I::variant(ns, op));
    let related = |op| {
        root(I::field(
            field(ns, "ownedRelationship"),
            I::at(0, I::variant(rel, op)),
        ))
    };
    let ops = vec![
        root(I::New),
        root(I::field(
            field(ns, "elementId"),
            I::Leaf(L::Write(Scalar::text("root-id"))),
        )),
        root(I::field(
            field(ns, "aliasIds"),
            I::Leaf(L::Insert {
                pos: 0,
                value: Scalar::text("z"),
            }),
        )),
        root(I::field(
            field(ns, "aliasIds"),
            I::Leaf(L::Insert {
                pos: 1,
                value: Scalar::text("a"),
            }),
        )),
        root(I::field(
            field(ns, "ownedRelationship"),
            I::insert(0, I::variant(rel, I::New)),
        )),
        related(I::field(
            field(rel, "elementId"),
            I::Leaf(L::Write(Scalar::text("rel-id"))),
        )),
    ];
    let relay = Relay::new(&c, true);
    let result = ok(
        relay.port,
        "POST",
        &format!("/api/model/{}/ops", c.model),
        json!({"ops":ops}),
    );
    assert_eq!(
        result["applied"], 6,
        "MD04 instrument: literal structure accepted"
    );
    let response = http(
        relay.port,
        "GET",
        &format!("/api/model/{}/state/identified", c.model),
        "",
    );
    assert_eq!(response.status, 200);
    let doc = parse(&response.body);
    let ids = Source::served(&doc).unwrap();
    c.source.check(&doc, true, true, &ids);
    let ops = vec![
        related(I::field(
            field(rel, "source"),
            I::Leaf(L::Link(ids["root-id"].clone())),
        )),
        related(I::field(
            field(rel, "target"),
            I::Leaf(L::Link(ids["root-id"].clone())),
        )),
        related(I::field(
            field(rel, "target"),
            I::Leaf(L::Link(library.to_owned())),
        )),
    ];
    let relay_port = relay.port;
    let path = format!("/api/model/{}/ops", c.model);
    let poster = thread::spawn(move || ok(relay_port, "POST", &path, json!({"ops":ops})));
    let deadline = Instant::now() + Duration::from_secs(30);
    while !relay.trace.lock().unwrap().first_seen {
        assert!(
            Instant::now() < deadline,
            "MD04 instrument: literal Link never reached barrier"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        relay.trace.lock().unwrap().forwarded_links,
        0,
        "MD04 instrument: barrier forwarded a Link"
    );
    c.source
        .check(&parse(&state(c.port(0), &c.model, true)), true, true, &ids);
    relay.release();
    let result = poster.join().unwrap();
    assert_eq!(
        result["applied"], 3,
        "MD04 instrument: literal links accepted"
    );
    relay.assert_trace();
    c.check();
    c.resume();
    c.convergence();
}
