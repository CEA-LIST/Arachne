//! SI.05, claims R3 and R4 over `SimpleUML.ecore`: the interpreted path,
//! running SimpleUML's descriptor with the new rule
//! (`fixtures/si05/simpleuml.identity.metamodel.json`, `arachne describe`'s
//! output at d81cae8 with the four in-scope references moved to
//! `identityReference`), reads every reference as this generated crate does,
//! projected to creation identity, on every schedule below and on thirty
//! seeded scripts. The harness and the projection are `si05_twin/mod.rs`.
//!
//! The four in-scope references: `Generalization.general` (`[0, 1]`),
//! `Property.ttype`, `Association.source`, `Association.target` (`[1, 1]`).
//! Left out, on both sides, and only these: `Packageable.owner` and
//! `Property.owner`, which declare an `eOpposite` (Cam's decision 3).
//!
//! ```bash
//! cd generated/simpleuml_crdt && nice -n 10 cargo test -j 2 --test si05_reference_twin -- --nocapture
//! ```

mod si05_twin;

use std::collections::BTreeSet;

use moirai_crdt::utils::membership::twins_log;
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::Tcsb;
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::{IsReplica, Replica};
use serde_json::{Value, json};
use simpleuml_crdt::package::{Simpleuml, SimpleumlLog, SimpleumlValue};

use si05_twin::*;

type GenReplica = Replica<SimpleumlLog, Tcsb<Simpleuml>>;

struct SimpleUml {
    a: GenReplica,
    b: GenReplica,
}

impl GenSide for SimpleUml {
    type Msg = EventMessage<Simpleuml>;
    const ROOT_CLASS: &'static str = "Model";

    fn fresh() -> Self {
        let (a, b) = twins_log::<SimpleumlLog>();
        SimpleUml { a, b }
    }

    /// A `PrimitiveType` in the `t_type` root arm: an event of `a`'s that no
    /// script reads, answering the interpreted `Install`.
    fn opener() -> Value {
        json!({"TTypeKind": {"PrimitiveType": "New"}})
    }

    fn mint_root() -> Value {
        json!({"ModelElementKind": {"Classifier": {"Package": {"Model": "New"}}}})
    }

    fn on_root(_meta: &Meta, op: Value) -> Value {
        json!({"ModelElementKind": {"Classifier": {"Package": {"Model": op}}}})
    }

    fn arc(source: &str, feature: &str, target: &str) -> Option<&'static str> {
        match (source, feature, target) {
            ("Generalization", "general", "Class") => Some("GeneralizationToClass"),
            ("Property", "ttype", "Class") => Some("PropertyToClass2"),
            ("Property", "ttype", "DataType") => Some("PropertyToDataType2"),
            ("Property", "ttype", "PrimitiveType") => Some("PropertyToPrimitiveType"),
            ("Property", "ttype", "Enumeration") => Some("PropertyToEnumeration"),
            ("Association", "source", "Class") => Some("AssociationToClass"),
            ("Association", "target", "Class") => Some("AssociationToClass2"),
            _ => None,
        }
    }

    fn feature_of(arc: &str) -> Option<&'static str> {
        match arc {
            "GeneralizationToClass" => Some("general"),
            "PropertyToClass2"
            | "PropertyToDataType2"
            | "PropertyToPrimitiveType"
            | "PropertyToEnumeration" => Some("ttype"),
            "AssociationToClass" => Some("source"),
            "AssociationToClass2" => Some("target"),
            _ => None,
        }
    }

    fn send(&mut self, writer: char, op: Value) -> Result<Self::Msg, String> {
        let op: Simpleuml = serde_json::from_value(op.clone()).unwrap_or_else(|error| {
            panic!("the typed encoder built no `Simpleuml`: {error}\n{op}")
        });
        let replica = if writer == 'a' {
            &mut self.a
        } else {
            &mut self.b
        };
        replica.send(op).map_err(|rejection| rejection.to_string())
    }

    fn receive(&mut self, at: char, message: Self::Msg) {
        if at == 'a' { &mut self.a } else { &mut self.b }.receive(message);
    }

    fn tag(message: &Self::Msg) -> String {
        let event = message.event();
        format!(
            "{}@{} L{}",
            event.id().origin_id(),
            event.id().seq(),
            event.lamport().val()
        )
    }

    fn graph(&self, at: char) -> (Vec<Value>, Vec<(usize, usize, Value)>) {
        let replica = if at == 'a' { &self.a } else { &self.b };
        let value: SimpleumlValue = replica.query(&Read::<SimpleumlValue>::new());
        let graph = &value.refs;
        let vertices = graph
            .node_indices()
            .map(|i| serde_json::to_value(&graph[i]).expect("a vertex serializes"))
            .collect();
        let arcs = graph
            .edge_indices()
            .map(|e| {
                let (from, to) = graph.edge_endpoints(e).expect("an edge has endpoints");
                (
                    from.index(),
                    to.index(),
                    serde_json::to_value(&graph[e]).expect("an edge serializes"),
                )
            })
            .collect();
        (vertices, arcs)
    }
}

fn read(relative: &str) -> Value {
    let path = format!("{}/{relative}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}")),
    )
    .unwrap_or_else(|e| panic!("{path} is not JSON: {e}"))
}

/// Today's descriptor, as `arachne describe` writes it at d81cae8
/// (`examples/uml.metamodel.json`, the same bytes).
fn today() -> Value {
    read("../../examples/uml.metamodel.json")
}

fn identity() -> Value {
    read("tests/fixtures/si05/simpleuml.identity.metamodel.json")
}

// Labels of the opening model.
const C: Label = 1;
const D: Label = 2;
const E: Label = 3;
const G: Label = 4;
const A: Label = 5;
const P: Label = 6;
const T: Label = 7;
const F: Label = 8;

/// `Model{ownedElements: [C, D{generalizations: [G], attributes: [P]}, E, A, T, F]}`,
/// four classes, an association, a primitive type, written by `a` and
/// delivered; `C` and `D` named.
fn opening() -> Vec<Step> {
    vec![
        edit('a', Action::Root),
        create('a', 0, "ownedElements", 0, "Class", C),
        create('a', 0, "ownedElements", 1, "Class", D),
        create('a', 0, "ownedElements", 2, "Class", E),
        create('a', D, "generalizations", 0, "Generalization", G),
        create('a', 0, "ownedElements", 3, "Association", A),
        create('a', D, "attributes", 0, "Property", P),
        create('a', 0, "ownedElements", 4, "PrimitiveType", T),
        create('a', 0, "ownedElements", 5, "Class", F),
        rename('a', C, 'c'),
        rename('a', D, 'd'),
        Step::Deliver,
    ]
}

/// Every schedule the spec's method section 5.3 lists for SimpleUML, by name.
fn schedules() -> Vec<(&'static str, Vec<Step>)> {
    let linked = || {
        vec![
            link('a', G, "general", C),
            link('a', A, "source", C),
            link('a', A, "target", D),
            link('a', P, "ttype", T),
            Step::Deliver,
        ]
    };
    let with = |mut head: Vec<Step>, tail: Vec<Step>| {
        head.extend(tail);
        head
    };
    vec![
        (
            "link, then a rename of every target on the other replica",
            with(
                linked(),
                vec![
                    rename('b', C, 'x'),
                    rename('b', D, 'y'),
                    rename('b', T, 'z'),
                    Step::Deliver,
                ],
            ),
        ),
        (
            "unlink, then retarget",
            with(
                linked(),
                vec![
                    unlink('a', G, "general", C),
                    Step::Deliver,
                    link('b', G, "general", E),
                    Step::Deliver,
                ],
            ),
        ),
        (
            "concurrent retarget of a [0, 1] reference, one Lamport",
            with(
                linked(),
                vec![
                    unlink('a', G, "general", C),
                    link('a', G, "general", E),
                    unlink('b', G, "general", C),
                    link('b', G, "general", F),
                    Step::Deliver,
                ],
            ),
        ),
        (
            "concurrent retarget of a [0, 1] reference, one Lamport, the parity shifted by one delivered edit",
            with(
                linked(),
                vec![
                    rename('a', E, 'p'),
                    Step::Deliver,
                    unlink('a', G, "general", C),
                    link('a', G, "general", E),
                    unlink('b', G, "general", C),
                    link('b', G, "general", F),
                    Step::Deliver,
                ],
            ),
        ),
        (
            "concurrent retarget of a [0, 1] reference, a ahead by one",
            with(
                linked(),
                vec![
                    rename('a', E, 'q'),
                    unlink('a', G, "general", C),
                    link('a', G, "general", E),
                    unlink('b', G, "general", C),
                    link('b', G, "general", F),
                    Step::Deliver,
                ],
            ),
        ),
        (
            "concurrent retarget of a [0, 1] reference, b ahead by one",
            with(
                linked(),
                vec![
                    unlink('a', G, "general", C),
                    link('a', G, "general", E),
                    rename('b', F, 'q'),
                    unlink('b', G, "general", C),
                    link('b', G, "general", F),
                    Step::Deliver,
                ],
            ),
        ),
        (
            "concurrent retarget to E and F, then E deleted",
            with(
                linked(),
                vec![
                    unlink('a', G, "general", C),
                    link('a', G, "general", E),
                    unlink('b', G, "general", C),
                    link('b', G, "general", F),
                    Step::Deliver,
                    delete('a', E),
                    Step::Deliver,
                ],
            ),
        ),
        (
            "concurrent retarget to E and F, then F deleted",
            with(
                linked(),
                vec![
                    unlink('a', G, "general", C),
                    link('a', G, "general", E),
                    unlink('b', G, "general", C),
                    link('b', G, "general", F),
                    Step::Deliver,
                    delete('b', F),
                    Step::Deliver,
                ],
            ),
        ),
        (
            "two rounds of concurrent retargets, then the second round's targets deleted in turn",
            with(
                linked(),
                vec![
                    unlink('a', G, "general", C),
                    link('a', G, "general", E),
                    rename('b', C, 'w'),
                    Step::Deliver,
                    unlink('b', G, "general", E),
                    link('b', G, "general", C),
                    unlink('a', G, "general", E),
                    link('a', G, "general", F),
                    Step::Deliver,
                    delete('b', C),
                    Step::Deliver,
                    delete('a', F),
                    Step::Deliver,
                ],
            ),
        ),
        (
            "deletion causally after the links",
            with(
                vec![
                    link('a', G, "general", C),
                    link('a', A, "source", C),
                    link('a', A, "target", C),
                    link('a', P, "ttype", C),
                    Step::Deliver,
                ],
                vec![delete('b', C), Step::Deliver],
            ),
        ),
        (
            "deletion concurrent with the links",
            vec![
                delete('a', C),
                link('b', G, "general", C),
                link('b', A, "source", C),
                link('b', P, "ttype", C),
                Step::Deliver,
            ],
        ),
        (
            "Q1: a link the deletion saw, the target renamed concurrently",
            with(
                linked(),
                vec![delete('a', C), rename('b', C, 'r'), Step::Deliver],
            ),
        ),
        (
            "Q2: a link and a rename concurrent with the deletion",
            vec![
                delete('a', C),
                rename('b', C, 'r'),
                link('b', G, "general", C),
                link('b', A, "source", C),
                Step::Deliver,
            ],
        ),
        (
            "delete, then re-create with the same name",
            with(
                linked(),
                vec![
                    delete('a', C),
                    Step::Deliver,
                    create('a', 0, "ownedElements", 0, "Class", 9),
                    rename('a', 9, 'c'),
                    Step::Deliver,
                ],
            ),
        ),
        (
            "the source's container deleted concurrently with a link from it",
            vec![delete('a', D), link('b', G, "general", C), Step::Deliver],
        ),
        (
            "the source's container deleted and renamed concurrently, its link made before",
            with(
                linked(),
                vec![delete('a', D), rename('b', D, 'r'), Step::Deliver],
            ),
        ),
        (
            "one target linked on both replicas at once",
            vec![
                link('a', G, "general", C),
                link('b', G, "general", C),
                Step::Deliver,
                unlink('b', G, "general", C),
                Step::Deliver,
            ],
        ),
        (
            "a full [1, 1] reference refuses a second link and its unlink",
            with(
                linked(),
                vec![
                    link('a', A, "source", D),
                    unlink('b', A, "source", C),
                    Step::Deliver,
                ],
            ),
        ),
        (
            "a link into a package deleted with its contents",
            vec![
                create('a', 0, "ownedElements", 6, "Package", 10),
                create('a', 10, "ownedElements", 0, "Class", 11),
                link('a', G, "general", 11),
                Step::Deliver,
                delete('b', 10),
                Step::Deliver,
            ],
        ),
        (
            "a link into a package, the package deleted while the class is renamed",
            vec![
                create('a', 0, "ownedElements", 6, "Package", 10),
                create('a', 10, "ownedElements", 0, "Class", 11),
                link('a', G, "general", 11),
                link('a', P, "ttype", 11),
                Step::Deliver,
                delete('b', 10),
                rename('a', 11, 'k'),
                Step::Deliver,
            ],
        ),
    ]
}

/// R3 on every hand-written schedule, in both delivery orders: after every
/// edit and every delivery the four replicas' references, projected to
/// creation identity, are two equal pairs, and the peers of each path agree
/// at every delivery.
#[test]
fn si05_r3_simpleuml_every_reference_schedule_reads_as_the_generated_graph() {
    let mut failures = Vec::new();
    let (mut runs, mut edits, mut refused, mut comparisons, mut links) = (0, 0, 0, 0, 0);
    for (name, steps) in schedules() {
        for order in [Order::AFirst, Order::BFirst] {
            let mut twin = Twin::<SimpleUml>::new(&today(), &identity(), order);
            let mut all = opening();
            all.extend(steps.clone());
            runs += 1;
            match twin.run(&all) {
                Ok(()) => {}
                Err(why) => failures.push(format!("{name} ({order:?}): {why}")),
            }
            edits += twin.edits;
            refused += twin.refused;
            comparisons += twin.comparisons;
            links += twin.links_read;
        }
    }
    println!(
        "SI05-R3 SimpleUML hand schedules: {} schedules x 2 orders = {runs} runs, {edits} edits, {refused} \
         refused by the generated intake, {comparisons} comparisons, {links} links read at the peaks",
        schedules().len()
    );
    assert!(
        failures.is_empty(),
        "{} of {runs} runs differ:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
    assert!(
        refused >= 4,
        "the full [1, 1] schedule must meet the generated intake's refusals"
    );
}

/// R3 and R4 on thirty seeded scripts, ten sequential and twenty concurrent,
/// each edit a create, a delete, a rename, a link or an unlink proposed
/// against the writer's own view.
const CREATABLE: [(&str, &str, &str); 5] = [
    ("Model", "ownedElements", "Class"),
    ("Model", "ownedElements", "Association"),
    ("Model", "ownedElements", "PrimitiveType"),
    ("Class", "generalizations", "Generalization"),
    ("Class", "attributes", "Property"),
];

#[test]
fn si05_r4_simpleuml_thirty_seeded_scripts_compare_every_in_scope_reference() {
    let mut failures = Vec::new();
    let (mut edits, mut refused, mut comparisons, mut links) = (0, 0, 0, 0);
    for seed in 0..30u64 {
        match seeded::<SimpleUml>(
            &today(),
            &identity(),
            seed,
            seed >= 10,
            &opening(),
            &CREATABLE,
            false,
        ) {
            Ok(twin) => {
                edits += twin.edits;
                refused += twin.refused;
                comparisons += twin.comparisons;
                links += twin.links_read;
            }
            Err(why) => failures.push(why),
        }
    }
    println!(
        "SI05-R4 SimpleUML seeded: 30 scripts, {edits} edits, {refused} refused by the generated intake, \
         {comparisons} comparisons, {links} links read at the peaks"
    );
    assert!(
        failures.is_empty(),
        "{} of 30 scripts differ:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
    assert!(edits > 30 * 15, "thirty scripts came to only {edits} edits");
    assert!(
        links > 30,
        "the scripts read only {links} links; they do not exercise references"
    );
}

/// R4: the in-scope references are exactly the four, every one of them is
/// driven by the hand schedules, and the only references left out are the two
/// that declare an `eOpposite`.
#[test]
fn si05_r4_simpleuml_the_only_references_left_out_are_the_eopposite_ones() {
    let descriptor = identity();
    let mut left_out = BTreeSet::new();
    let mut in_scope = BTreeSet::new();
    for (class, body) in descriptor["classes"].as_object().expect("classes") {
        for entry in body["references"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            let name = format!("{class}.{}", entry["name"].as_str().expect("a name"));
            if entry["merge"]["kind"] == json!("identityReference") {
                in_scope.insert(name);
            } else {
                assert!(
                    entry.get("eOpposite").is_some(),
                    "{name} is left out and declares no eOpposite"
                );
                left_out.insert(name);
            }
        }
    }
    let expect = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    assert_eq!(
        in_scope,
        expect(&[
            "Association.source",
            "Association.target",
            "Generalization.general",
            "Property.ttype"
        ])
    );
    assert_eq!(left_out, expect(&["Packageable.owner", "Property.owner"]));
    let driven: BTreeSet<String> = schedules()
        .iter()
        .flat_map(|(_, steps)| steps.clone())
        .filter_map(|step| match step {
            Step::Edit(Edit {
                action: Action::Link {
                    source, feature, ..
                },
                ..
            }) => Some(match source {
                G => format!("Generalization.{feature}"),
                A => format!("Association.{feature}"),
                P => format!("Property.{feature}"),
                _ => String::new(),
            }),
            _ => None,
        })
        .collect();
    for name in &in_scope {
        assert!(
            driven.contains(name),
            "{name} is in scope and no hand schedule links it"
        );
    }
}

// ---------------------------------------------------------------------------
// The harness's own rehearsal, against the generated runtime alone
// ---------------------------------------------------------------------------

fn expected(rows: &[(Label, &str, &[Label])]) -> Projection {
    rows.iter()
        .filter(|(_, _, targets)| !targets.is_empty())
        .map(|(source, feature, targets)| {
            (
                (*source, feature.to_string()),
                targets.iter().copied().collect(),
            )
        })
        .collect()
}

/// What the generated graph reads at the end of each hand schedule, computed
/// by hand from `typed_graph!`'s rules (an arc goes with a causally later
/// `RemoveArc` of itself or `DeleteSubtree` over either end; an arc is read
/// while both its vertices are; a `[0, 1]` edge keeps the greatest arc by
/// `FairPolicy`) and from decision 2's finding (an update concurrent with a
/// deletion keeps the object and its vertex). `None` for the two schedules
/// whose answer is the tie itself, held to "one of the two" instead.
fn hand_computed(name: &str) -> Option<Projection> {
    let linked = [
        (G, "general", &[C][..]),
        (A, "source", &[C][..]),
        (A, "target", &[D][..]),
        (P, "ttype", &[T][..]),
    ];
    let with = |general: &[Label]| {
        let mut rows = linked.to_vec();
        rows[0] = (G, "general", general);
        expected(&rows)
    };
    Some(match name {
        "link, then a rename of every target on the other replica" => expected(&linked),
        "unlink, then retarget" => with(&[E]),
        "concurrent retarget of a [0, 1] reference, one Lamport" => return None,
        "concurrent retarget of a [0, 1] reference, one Lamport, the parity shifted by one delivered edit" =>
        {
            return None;
        }
        "concurrent retarget of a [0, 1] reference, a ahead by one" => with(&[E]),
        "concurrent retarget of a [0, 1] reference, b ahead by one" => with(&[F]),
        "concurrent retarget to E and F, then E deleted" => with(&[F]),
        "concurrent retarget to E and F, then F deleted" => with(&[E]),
        "two rounds of concurrent retargets, then the second round's targets deleted in turn" => {
            expected(&[(A, "target", &[D]), (P, "ttype", &[T])])
        }
        "deletion causally after the links" => expected(&[]),
        "deletion concurrent with the links" => expected(&[]),
        "Q1: a link the deletion saw, the target renamed concurrently" => {
            expected(&[(A, "target", &[D]), (P, "ttype", &[T])])
        }
        "Q2: a link and a rename concurrent with the deletion" => {
            expected(&[(G, "general", &[C]), (A, "source", &[C])])
        }
        "delete, then re-create with the same name" => {
            expected(&[(A, "target", &[D]), (P, "ttype", &[T])])
        }
        "the source's container deleted concurrently with a link from it" => expected(&[]),
        "the source's container deleted and renamed concurrently, its link made before" => {
            expected(&[(A, "source", &[C])])
        }
        "one target linked on both replicas at once" => expected(&[]),
        "a full [1, 1] reference refuses a second link and its unlink" => expected(&linked),
        "a link into a package deleted with its contents" => expected(&[]),
        "a link into a package, the package deleted while the class is renamed" => expected(&[]),
        other => panic!("no hand-computed answer for `{other}`"),
    })
}

/// The validator's own control on the harness (VALIDATION.md rule 2): every
/// hand schedule run against the generated runtime alone, the interpreted
/// pair standing in, must end at the hand-computed answer in both orders.
/// Green before SI.05 is implemented, and it stays green: it reads no
/// interpreted reference.
#[test]
fn si05_harness_rehearsal_simpleuml_the_generated_half_reads_the_hand_computed_answers() {
    let mut wrong = Vec::new();
    for (name, steps) in schedules() {
        for order in [Order::AFirst, Order::BFirst] {
            let mut twin = Twin::<SimpleUml>::rehearsal(&today(), &identity(), order);
            let mut all = opening();
            all.extend(steps.clone());
            if let Err(why) = twin.run(&all) {
                wrong.push(format!(
                    "{name} ({order:?}): the rehearsal did not run: {why}"
                ));
                continue;
            }
            let got = twin.gen_projection('a').expect("a projection");
            match hand_computed(name) {
                Some(want) if got != want => wrong.push(format!(
                    "{name} ({order:?}): generated {got:?}, by hand {want:?}"
                )),
                Some(_) => {}
                None => {
                    let general = got
                        .get(&(G, "general".to_string()))
                        .cloned()
                        .unwrap_or_default();
                    if general.len() != 1 || !(general.contains(&E) || general.contains(&F)) {
                        wrong.push(format!(
                            "{name} ({order:?}): G.general reads {general:?}, not one of E and F"
                        ));
                    }
                }
            }
            println!("SI05-REHEARSAL {name} ({order:?}): {got:?}");
        }
    }
    assert!(
        wrong.is_empty(),
        "the harness or a hand answer is wrong:\n  {}",
        wrong.join("\n  ")
    );
}

/// The same thirty seeded scripts, rehearsed against the generated runtime
/// alone: they run without a harness error and exercise references (links
/// read, links refused by the generated intake). Green before SI.05.
#[test]
fn si05_harness_rehearsal_simpleuml_the_seeded_scripts_run_against_the_generated_half() {
    let (mut edits, mut refused, mut links) = (0, 0, 0);
    let mut failures = Vec::new();
    for seed in 0..30u64 {
        match seeded::<SimpleUml>(
            &today(),
            &identity(),
            seed,
            seed >= 10,
            &opening(),
            &CREATABLE,
            true,
        ) {
            Ok(twin) => {
                edits += twin.edits;
                refused += twin.refused;
                links += twin
                    .gen_projection('a')
                    .expect("a projection")
                    .values()
                    .map(BTreeSet::len)
                    .sum::<usize>();
            }
            Err(why) => failures.push(why),
        }
    }
    println!(
        "SI05-REHEARSAL SimpleUML seeded: 30 scripts, {edits} edits, {refused} refused, {links} links read at the end"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    assert!(edits > 30 * 15, "only {edits} edits");
    assert!(
        links > 30,
        "only {links} links at the end of thirty scripts"
    );
}
