//! The equivalence oracle: `ip13`, `ip14`'s shape and `ip15` of
//! [`02 Validation Plan`], step 5 of the implementation plan.
//!
//! # What is being claimed
//!
//! Criterion I-A1: for the same model edits applied in the same causal
//! order, a replica running the interpreted `ModelLog` over `bt.ecore`'s
//! table and a replica running this generated crate reach read-outs that are
//! equal under the canonical projection of the validation plan's section 2.
//! Criterion I-A2: the harness below fails when the two paths genuinely
//! differ, which `ip15` shows by binding `TreeNode.ID` to a multi-value
//! register on the interpreted side alone and demanding an inequality.
//!
//! # How the two paths are driven
//!
//! Four replicas: `a` and `b` of the interpreted `ModelLog`, `a` and `b` of
//! the generated `BehaviortreeLog`. One [`Edit`] is encoded twice, once into
//! a [`ModelOp`] and once into this crate's typed operation, and the two
//! encodings are handed to the two `a` replicas — or the two `b` replicas —
//! in the same order, with delivery under the test's control and identical
//! on both paths. The read-outs are compared **after every single
//! operation**, not only at the end, so a failure names the edit that broke
//! it rather than the script that contained it.
//!
//! Replica names are `a` and `b` on both paths and `EventId::cmp` orders by
//! replica name first (`event/id.rs:91-98`), so the two paths break their
//! concurrency ties the same way. The sequence numbers are lined up as well:
//! the interpreted log opens on a `ModelOp::Install` from `a`, so the
//! generated pair is opened with one `Root::New`, which writes nothing and
//! leaves the log default, purely so that edit *n* is event *n+1* on both
//! paths.
//!
//! # The typed encoder
//!
//! The generated operation is built as JSON and then deserialized into
//! `Behaviortree`, which is what the implementation plan asks for ("typed
//! `bt_crdt` operation JSON by the naming convention the generator uses").
//! Nothing about `bt.ecore` is hard-coded: the `<Super>Super` hops come from
//! the class table's `supers`, the `union!` variant chain from its subclass
//! relation, and the field names from `heck`, which is the crate the
//! generator itself names its fields with. A wrong shape is a
//! `serde_json::from_value` error naming the enum it could not build, not a
//! silent pass.
//!
//! # What the projection excludes, and why
//!
//! - **`DataFlowPort.entry`**, and every other non-containment reference.
//!   The interpreted path carries a reference as a string (design §8): a
//!   multi-value register for a single one, an add-wins set for a many.
//!   The generated path carries none of that in the record at all — a
//!   non-containment reference is a `typed_graph!` arc in a second log
//!   (`references.rs`), and `BehaviortreeValue::refs` is `serde(skip)`.
//!   There is no value on the generated side to compare against, so the
//!   projection drops the key from the interpreted side and the scripts
//!   never write one. This is the one feature of `bt.ecore` the oracle does
//!   not cover.
//! - **`Status`**, which no feature of `bt.ecore` reaches, so it appears on
//!   neither side. Stated by the validation plan §2 and true here.
//! - **Every value that is the default of its feature's rule**, dropped from
//!   both sides by the same function ([`without_defaults`]) before they are
//!   compared. This is the rule §2 was missing. `record!`'s `new` builds one
//!   field per feature eagerly, so a generated log that exists renders its
//!   whole shape — `Root.behaviortrees` as `[]`, a never-written
//!   `BehaviorTree.ID` as `""` — and a generated log renders that shape from
//!   the moment the log is constructed, before any operation at all. The
//!   interpreted path has no object anywhere until an operation mints one and
//!   reads `null` for the whole model until then. A never-written required
//!   attribute is semantically absent whichever path renders it, so the
//!   projection spells it absent on both, and a root every one of whose
//!   features is at its default is spelled `null` on both. Neither path is
//!   wrong and the difference is not observable in the canonical form.
//!
//!   What this costs is stated rather than hidden: the oracle cannot tell an
//!   object all of whose features are default from no object at all. It is
//!   not free to widen — an *optional* is exempt, because an optional's
//!   default is absence and absence already carries no key, so a value
//!   written and then emptied (`TreeNode.name` set and then unset, which both
//!   paths leave present and empty) stays present on both sides and is
//!   compared. [`an_emptied_optional_is_not_dropped`] is the test that keeps
//!   that honest.
//! - ~~**An object created into an ordered containment and never written
//!   into**~~, which the generated read-out could not distinguish from a
//!   removed one and so could not render. That was I-A1's one named
//!   exception, applied on top of the projection to both sides, and it is
//!   **gone**: `NestedListLog` asks its ordering for its children now instead
//!   of asking `UWMapLog` for the filtered map, so existence and value are
//!   separate questions again and the generated path renders the object. I-A1
//!   reads, with no exception: the two paths agree on every state reachable by
//!   a write. [`the_thirty_scripts_find_no_difference_at_all`] is what keeps
//!   that true.
//! - Nothing else. In particular the other structural difference — the
//!   generated path materialising a single-valued containment whose target
//!   has no subclasses (`Root.main`, `BehaviorTree.blackboard`,
//!   `SubTree.tree`) while the interpreted path mints an object only when
//!   something is written into it — is handled where it belongs, in the
//!   script: creating an object also mints every such mandatory child of it,
//!   in the same operation, so both paths hold the same objects.


mod support;

use std::sync::Arc;

use moirai_interp::{InstanceOp, LeafOp, ModelOp};
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::IsReplica;
use moirai_semantics::{LeafRule, MergeRule, Shape, from_descriptor};
use serde_json::{Value, json};
use support::*;

// ---------------------------------------------------------------------------
// 7. The tests
// ---------------------------------------------------------------------------

/// The descriptor the interpreted path runs is byte-for-byte the one this
/// crate was generated from. If it ever is not, everything below compares two
/// metamodels rather than two paths.
#[test]
fn the_two_arms_hold_the_same_metamodel() {
    let checked_in: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/metamodel.json"))
            .expect("the generated crate ships its descriptor"),
    )
    .expect("the descriptor is JSON");
    assert_eq!(
        checked_in,
        bt_descriptor(),
        "`moirai-interp`'s fixture and this crate's `metamodel.json` have drifted"
    );
}

/// `ip13`: thirty seeded edit scripts over `bt.ecore`, ten sequential and
/// twenty with two writers diverging and merging, every one equal under the
/// canonical projection after every operation.
///
/// **This test fails, and the failure is the finding.** Every one of the
/// thirty scripts differs, and every difference is the one inequality
/// [`a_sequence_child_with_nothing_written_is_invisible_on_the_generated_path`]
/// pins and [`the_thirty_scripts_find_exactly_one_kind_of_difference`] shows
/// to be the only one: an object created into an ordered containment and not
/// yet written into is on the interpreted read-out and absent from the
/// generated one, because `UWMapLog`'s read drops a child whose value equals
/// the default and `moirai-interp`'s sequence read does not. I-A1 does not
/// hold at this tip and it is one rule in one function away from holding.
/// The fix is in Moirai, not here, so this test says so rather than being
/// widened until it passes.
#[test]
fn ip13_thirty_seeded_scripts_over_bt_ecore_agree() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let mut failures = Vec::new();
    let mut edits = 0;
    let mut ops = 0;
    let mut refused = 0;
    for seed in 0..30u64 {
        let concurrent = seed >= 10;
        let script = seeded_script(&meta, seed, concurrent);
        edits += script
            .steps
            .iter()
            .filter(|step| matches!(step, Step::Edit(_)))
            .count();
        let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
        if let Err(reason) = harness.run(&script) {
            failures.push(reason);
        }
        ops += harness.ops;
        refused += harness.refused;
    }
    assert!(
        edits > 30 * 10,
        "thirty scripts came to only {edits} edits; the generator stopped early"
    );
    assert!(
        failures.is_empty(),
        "{} of thirty scripts differ. Every one of them is the empty sequence \
         child: an object created into an ordered containment and not yet \
         written into, which `moirai-interp`'s `eval::read_node` renders and \
         `UWMapLog`'s read drops. See \
         `a_sequence_child_with_nothing_written_is_invisible_on_the_generated_path` \
         for the reproducer and \
         `the_thirty_scripts_find_exactly_one_kind_of_difference` for the proof \
         that nothing else is behind it.\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
    println!(
        "ip13: 30 scripts, {edits} edits, {ops} operations, {refused} refused by both intakes"
    );
}

/// The story the validation plan tells under I-A1, written out rather than
/// drawn: two people concurrently rename the same `Sequence` and each add a
/// child to its `children`.
#[test]
fn ip13_the_validation_plans_own_story() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let script = rename_and_add_script(&meta);
    let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
    harness.run(&script).expect("the two paths agree");

    // And what they agree on is the merge, not an empty document.
    let doc = harness.interp_doc('a');
    let sequence = &doc["main"]["child"];
    assert_eq!(sequence[ECLASS], json!("Sequence"));
    assert_eq!(
        sequence["name"].as_str().expect("a name").chars().count(),
        2,
        "both renames survived, character by character: {}",
        sequence["name"]
    );
    assert_eq!(
        sequence["children"]
            .as_array()
            .expect("children read as an array")
            .len(),
        2,
        "both children survived"
    );
}

fn rename_and_add_script(meta: &Meta) -> EditScript {
    let mut steps: Vec<Step> = open_the_model(meta).into_iter().map(Step::Edit).collect();
    let main = Path::default().child(Hop {
        feature: "main".to_string(),
        at: None,
        class: "BehaviorTree".to_string(),
    });
    let sequence = main.child(Hop {
        feature: "child".to_string(),
        at: None,
        class: "Sequence".to_string(),
    });
    steps.push(Step::Edit(Edit {
        id: 2,
        writer: 'a',
        path: main.clone(),
        action: Action::Create {
            feature: "child".to_string(),
            pos: None,
            class: "Sequence".to_string(),
        },
    }));
    steps.push(Step::Deliver);
    // Both writers rename the same `Sequence` and add a child to it, each
    // seeing only its own edit.
    for (writer, ch, class) in [('a', 'x', "Fallback"), ('b', 'y', "Inverter")] {
        steps.push(Step::Edit(Edit {
            id: 2,
            writer,
            path: sequence.clone(),
            action: Action::Text {
                feature: "name".to_string(),
                op: TextOp::Insert {
                    pos: 0,
                    ch,
                    after: ch.to_string(),
                },
            },
        }));
        steps.push(Step::Edit(Edit {
            id: 3,
            writer,
            path: sequence.clone(),
            action: Action::Create {
                feature: "children".to_string(),
                pos: Some(0),
                class: class.to_string(),
            },
        }));
    }
    steps.push(Step::Deliver);
    EditScript {
        label: "the I-A1 story".to_string(),
        steps,
    }
}

/// `ip15`, the control: the same script, run twice. Once with both paths
/// holding `bt.ecore`'s own table, where it must be equal; once with
/// `TreeNode.ID` bound to a multi-value register on the interpreted side
/// alone, where the harness **must** report inequality.
///
/// Without the second half, I-A1 rests on an oracle that has never been seen
/// to fail.
#[test]
fn ip15_the_oracle_reports_a_difference_when_there_is_one() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let script = concurrent_id_script(&meta);

    let mut faithful = Harness::new(&bt_descriptor(), &bt_descriptor());
    faithful
        .run(&script)
        .expect("with the same table on both sides the two paths agree");

    let mut mutated = Harness::new(&bt_descriptor(), &register_id_descriptor());
    let verdict = mutated.run(&script);
    let reason = verdict.expect_err(
        "`TreeNode.ID` bound to a multi-value register on the interpreted side alone \
         is a genuine difference; an oracle that reports equality here is worthless",
    );
    assert!(
        reason.contains("ID"),
        "the failure should name the feature that was rebound: {reason}"
    );
    println!("ip15: the oracle reported the mutation —\n{reason}");
}

/// Two writers appending to the same `TreeNode.ID` while divergent: a text
/// leaf merges them character by character, a multi-value register keeps
/// both and reads out a conflict.
fn concurrent_id_script(meta: &Meta) -> EditScript {
    let mut steps: Vec<Step> = open_the_model(meta).into_iter().map(Step::Edit).collect();
    let main = Path::default().child(Hop {
        feature: "main".to_string(),
        at: None,
        class: "BehaviorTree".to_string(),
    });
    let sequence = main.child(Hop {
        feature: "child".to_string(),
        at: None,
        class: "Sequence".to_string(),
    });
    steps.push(Step::Edit(Edit {
        id: 2,
        writer: 'a',
        path: main,
        action: Action::Create {
            feature: "child".to_string(),
            pos: None,
            class: "Sequence".to_string(),
        },
    }));
    steps.push(Step::Edit(Edit {
        id: 2,
        writer: 'a',
        path: sequence.clone(),
        action: Action::Text {
            feature: "ID".to_string(),
            op: TextOp::Insert {
                pos: 0,
                ch: 's',
                after: "s".to_string(),
            },
        },
    }));
    steps.push(Step::Deliver);
    for (writer, ch) in [('a', 'a'), ('b', 'b')] {
        steps.push(Step::Edit(Edit {
            id: 2,
            writer,
            path: sequence.clone(),
            action: Action::Text {
                feature: "ID".to_string(),
                op: TextOp::Insert {
                    pos: 1,
                    ch,
                    after: format!("s{ch}"),
                },
            },
        }));
    }
    steps.push(Step::Deliver);
    EditScript {
        label: "two writers on one `TreeNode.ID`".to_string(),
        steps,
    }
}

/// The encoders, checked against the shapes the generator actually emitted,
/// so that a wrong hop is a failure here and not a puzzling inequality later.
#[test]
fn the_typed_encoder_walks_the_super_hops_and_the_union_variants() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let edit = Edit {
        id: 1,
        writer: 'a',
        path: Path::default()
            .child(Hop {
                feature: "main".to_string(),
                at: None,
                class: "BehaviorTree".to_string(),
            })
            .child(Hop {
                feature: "child".to_string(),
                at: None,
                class: "Sequence".to_string(),
            }),
        action: Action::Text {
            feature: "ID".to_string(),
            op: TextOp::Insert {
                pos: 0,
                ch: 'q',
                after: "q".to_string(),
            },
        },
    };
    assert_eq!(
        typed_json(&meta, &edit),
        json!({
            "Root": { "Main": { "Child": { "ControlNode": { "Sequence": {
                "ControlNodeSuper": { "TreeNodeSuper": {
                    "Id": { "Insert": { "content": "q", "pos": 0 } }
                } }
            } } } } }
        }),
        "`Sequence` sees `ID` through `control_node_super` and then `tree_node_super`"
    );
    // And the interpreted encoding of the same edit has no hop at all.
    assert_eq!(
        interp_op(&meta, &edit),
        ModelOp::Instance(InstanceOp::variant(
            meta.slot("Root"),
            InstanceOp::field(
                meta.visible_slot(meta.slot("Root"), "main"),
                InstanceOp::variant(
                    meta.slot("BehaviorTree"),
                    InstanceOp::field(
                        meta.visible_slot(meta.slot("BehaviorTree"), "child"),
                        InstanceOp::variant(
                            meta.slot("Sequence"),
                            InstanceOp::field(
                                meta.visible_slot(meta.slot("Sequence"), "ID"),
                                InstanceOp::Leaf(LeafOp::InsertChar { pos: 0, ch: 'q' })
                            )
                        )
                    )
                )
            )
        ))
    );
    // The mint of a `SubTree` carries the two objects the generated path
    // materialises with it.
    assert_eq!(
        typed_mint(&meta, meta.slot("SubTree")),
        json!({"Tree": {"Blackboard": "New"}}),
        "a `SubTree` brings its `tree`, and that `BehaviorTree` its `blackboard`"
    );
}

/// The exclusion, stated as a test: `DataFlowPort.entry` is on the
/// interpreted read-out and nowhere on the generated one, and the projection
/// is what removes it.
///
/// The claim is about the projection, so the script is carried and not
/// compared. It used not to be comparable at all: an `OutFlowPort` has no
/// feature but `entry`, so once the projection has taken `entry` out there is
/// nothing left in it, and an object with nothing left in it was one the
/// generated path could not render — which was
/// [`a_sequence_child_with_nothing_written_is_invisible_on_the_generated_path`],
/// the one inequality this oracle ever found. That inequality is gone,
/// `NestedListLog` reading its children by the ordering rather than by their
/// values, so the two paths are compared here as well: an `OutFlowPort` is
/// `{"eClass": "OutFlowPort"}` on both. That test owns the finding; this one
/// owns the exclusion, and the assertions at the end pin the two together
/// rather than letting either hide the other.
#[test]
fn the_only_exclusion_is_the_non_containment_reference() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let references: Vec<String> = meta
        .sem
        .classes
        .iter()
        .flat_map(|class| {
            class.declared.iter().filter_map(move |feature| {
                matches!(feature.merge, MergeRule::Reference { .. })
                    .then(|| format!("{}.{}", class.name, feature.name))
            })
        })
        .collect();
    assert_eq!(
        references,
        vec!["DataFlowPort.entry".to_string()],
        "`bt.ecore` has exactly one non-containment reference and this is it"
    );

    let mut steps: Vec<Step> = open_the_model(&meta).into_iter().map(Step::Edit).collect();
    let main = Path::default().child(Hop {
        feature: "main".to_string(),
        at: None,
        class: "BehaviorTree".to_string(),
    });
    steps.push(Step::Edit(Edit {
        id: 2,
        writer: 'a',
        path: main.clone(),
        action: Action::Create {
            feature: "child".to_string(),
            pos: None,
            class: "OpenDoor".to_string(),
        },
    }));
    let door = main.child(Hop {
        feature: "child".to_string(),
        at: None,
        class: "OpenDoor".to_string(),
    });
    steps.push(Step::Edit(Edit {
        id: 3,
        writer: 'a',
        path: door,
        action: Action::Create {
            feature: "outflowports".to_string(),
            pos: Some(0),
            class: "OutFlowPort".to_string(),
        },
    }));
    let script = EditScript {
        label: "a flow port, whose only feature is a reference".to_string(),
        steps,
    };
    let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
    for step in &script.steps {
        match step {
            Step::Edit(edit) => {
                assert_eq!(
                    harness.carry(edit),
                    Ok(true),
                    "both intakes take {}",
                    edit.show()
                );
            }
            Step::Deliver => harness.cross(),
        }
    }

    let raw: Value = harness.ia.query(&Read::<Value>::new());
    let port = &raw["main"]["child"]["outflowports"][0];
    assert_eq!(port[ECLASS], json!("OutFlowPort"));
    assert!(
        port.get("entry").is_some(),
        "the interpreted read-out carries `entry`: {port}"
    );
    let stripped = harness.interp_doc('a');
    assert_eq!(
        stripped["main"]["child"]["outflowports"][0],
        json!({"eClass": "OutFlowPort"}),
        "and the projection is what takes it out"
    );
    assert_eq!(
        harness.gen_doc('a'),
        stripped,
        "and with `entry` gone an `OutFlowPort` holds nothing, which the \
         generated path renders as the object it is: its existence is the \
         ordering's and no longer a function of whether it has a value"
    );
    assert_eq!(
        harness.gen_doc('a')["main"]["child"]["outflowports"][0],
        json!({"eClass": "OutFlowPort"}),
        "a class with no writable feature anywhere in the classifier tree is \
         showable on the generated path, which it never was"
    );
}

/// The guard on the projection's one exemption: a value written and then
/// emptied is **not** dropped, and compares equal on both paths.
///
/// [`without_defaults`] drops a key whose value is the default of its
/// feature's rule, and the whole rule turns on optionals being exempt. An
/// optional's default is absence, and an absent optional carries no key at
/// all, so a key that *is* there under an optional was put there by an
/// operation and has to survive the projection even when what it holds is
/// the empty string. Without this test the exemption is a comment; with it,
/// widening the rule to cover optionals fails here.
#[test]
fn an_emptied_optional_is_not_dropped() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    assert!(
        matches!(
            meta.rule(meta.slot("Sequence"), "name"),
            MergeRule::Attribute { shape, leaf: LeafRule::Text } if shape == Shape::Optional
        ),
        "`TreeNode.name` is the optional text this test is about"
    );

    let main = Path::default().child(Hop {
        feature: "main".to_string(),
        at: None,
        class: "BehaviorTree".to_string(),
    });
    let node = main.child(Hop {
        feature: "child".to_string(),
        at: None,
        class: "Sequence".to_string(),
    });
    let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
    for edit in open_the_model(&meta) {
        harness.apply(&edit).expect("the model opens");
    }
    harness
        .apply(&Edit {
            id: 2,
            writer: 'a',
            path: main,
            action: Action::Create {
                feature: "child".to_string(),
                pos: None,
                class: "Sequence".to_string(),
            },
        })
        .expect("a `Sequence` goes under `child`");

    // Nothing has been written into `name`, so neither path carries the key.
    assert!(
        harness.interp_doc('a')["main"]["child"].get("name").is_none(),
        "an untouched optional carries no key: {}",
        harness.interp_doc('a')
    );

    harness
        .apply(&Edit {
            id: 2,
            writer: 'a',
            path: node.clone(),
            action: Action::Text {
                feature: "name".to_string(),
                op: TextOp::Insert { pos: 0, ch: 'z', after: "z".to_string() },
            },
        })
        .expect("both paths take the character");
    assert_eq!(
        harness.interp_doc('a')["main"]["child"]["name"],
        json!("z"),
        "written, it is there on both paths"
    );

    harness
        .apply(&Edit {
            id: 2,
            writer: 'a',
            path: node,
            action: Action::Text {
                feature: "name".to_string(),
                op: TextOp::Delete { pos: 0, after: String::new() },
            },
        })
        .expect("both paths take the deletion");

    let interpreted = harness.interp_doc('a');
    let generated = harness.gen_doc('a');
    assert_eq!(interpreted, generated, "and emptied, the two paths agree");
    assert_eq!(
        interpreted["main"]["child"]["name"],
        json!(""),
        "emptied is present-and-empty and not absent: {interpreted}"
    );
    assert_eq!(
        interpreted["main"]["child"][ECLASS],
        json!("Sequence"),
        "and the object it hangs off is not collapsed either"
    );
}

/// **The one inequality this oracle used to find, and it is gone.**
///
/// An object created into an ordered containment and not yet written into was
/// on the interpreted read-out and was *not* on the generated one. It was
/// never a projection artefact: the projection is applied to both sides by the
/// same function and drops keys, never elements, and the two logs held the
/// same object at the same position — only the read-outs differed. They do not
/// differ now, and this test asserts the agreement where it used to assert the
/// gap. The name is kept because it is what the vault, the validation plan and
/// the manuscript call this finding.
///
/// # Where it came from
///
/// The generated path's ordered containment is `NestedListLog<L>`
/// (`moirai-crdt/src/list/nested_list.rs`), an ordering half over
/// `EventGraph<List<EventId>>` and a mapping half that is a `UWMapLog`.
/// `UWMapLog`'s read keeps a child only when its value differs from
/// `Value::default()`, and it has to: `UWMap::Remove` is not a tombstone, it
/// calls `redundant_by_parent` on the child and leaves it in the map, so
/// *reading as the default is how a map spells removed*. A child created and
/// never written into read as the default too, and the generated path could
/// not tell the two apart.
///
/// # What changed, and why it is not `UWMapLog`
///
/// `NestedListLog`'s read asks the **ordering** for its children now, one at a
/// time, instead of asking `UWMapLog` for the filtered map. A list never
/// needed the value-based reading: `NestedList::Delete` takes the position out
/// of the ordering as well as resetting the child, and a concurrent update
/// keeps the position alive through the eg-walker's own life dots, so the
/// ordering already recorded removal and the map's filter was doing that work
/// a second time — with the side effect this test was written for. Existence
/// is the ordering; value is the child's log; they are separate questions
/// again. `UWMapLog` is untouched, so a keyed map keeps the value-based rule
/// it does need.
///
/// `moirai-interp`'s `eval::read_node` always did it this way — its sequence
/// arm drops a *hole*, an id in the ordering with no child behind it, and
/// nothing else — so it is the generated path that moved, onto the answer the
/// interpreted path already gave. Nothing was weakened to meet it.
///
/// # What it buys, beyond this one object
///
/// An `OutFlowPort`, whose only feature is a non-containment reference and
/// which therefore has no writable feature anywhere in the classifier tree,
/// could never be rendered on the generated path at all — it was in the log,
/// took part in convergence, was reachable by reference operations and was
/// invisible in every document. It renders now, as `{"eClass":
/// "OutFlowPort"}`, and
/// [`the_only_exclusion_is_the_non_containment_reference`] is where that is
/// asserted.
///
/// # It used to heal, and now there is nothing to heal
///
/// The second half of this test still writes one character into the new
/// object and still checks that the two paths agree with the object at the
/// same index. It was the important half while the divergence lasted from an
/// object's creation to its first write; it is a plain regression check now.
#[test]
fn a_sequence_child_with_nothing_written_is_invisible_on_the_generated_path() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let blackboard = Path::default()
        .child(Hop { feature: "main".to_string(), at: None, class: "BehaviorTree".to_string() })
        .child(Hop { feature: "blackboard".to_string(), at: None, class: "Blackboard".to_string() });
    let entry = blackboard.child(Hop {
        feature: "entries".to_string(),
        at: Some(0),
        class: "BlackboardEntry".to_string(),
    });

    let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
    for edit in open_the_model(&meta) {
        harness.apply(&edit).expect("the model opens");
    }
    let create = Edit {
        id: 2,
        writer: 'a',
        path: blackboard,
        action: Action::Create {
            feature: "entries".to_string(),
            pos: Some(0),
            class: "BlackboardEntry".to_string(),
        },
    };
    assert_eq!(
        harness.carry(&create),
        Ok(true),
        "both intakes take the create; this is not a refusal"
    );

    let interpreted = harness.interp_doc('a');
    let generated = harness.gen_doc('a');
    assert_eq!(
        interpreted,
        json!({
            "eClass": "Root",
            "main": {
                "eClass": "BehaviorTree",
                "blackboard": {"eClass": "Blackboard", "entries": [{"eClass": "BlackboardEntry"}]}
            }
        }),
        "the interpreted path holds the object it was told to make"
    );
    assert_eq!(
        generated, interpreted,
        "the generated path holds it too and renders it now: the ordering says \
         the element is there and the map is no longer asked whether its value \
         is worth showing"
    );

    // One character, and they agree again.
    harness
        .apply(&Edit {
            id: 2,
            writer: 'a',
            path: entry,
            action: Action::Text {
                feature: "key".to_string(),
                op: TextOp::Insert { pos: 0, ch: 'k', after: "k".to_string() },
            },
        })
        .expect("one write into the new object and the two paths agree again");
    assert_eq!(
        harness.interp_doc('a')["main"]["blackboard"]["entries"],
        json!([{"eClass": "BlackboardEntry", "key": "k"}]),
        "at the same index, on both paths"
    );
}

/// And now there is none at all. Thirty scripts, every read-out after every
/// operation and after every delivery, both replicas of both paths, and the
/// two paths agree everywhere.
///
/// This test was `the_thirty_scripts_find_exactly_one_kind_of_difference`, and
/// it is where the manuscript's **one named exception** was kept honest. It
/// re-pruned both canonical documents by `except_unwritten_sequence_children`
/// — which dropped a sequence element carrying nothing but its class and
/// re-ran [`without_defaults`] to a fixed point, since dropping the element
/// could empty the array that held it and so empty the object that held
/// *that* — and asserted that the residual was empty, so that the whole of
/// I-A1 rested on one rule in one function and nothing else could hide behind
/// it.
///
/// The exception is gone, so the pruning is gone with it and what is asserted
/// is the stronger thing: not that every difference is explained by the
/// exception, but that there is no difference to explain. `differing` is
/// counted and asserted to be zero, so a re-appearance shows up as a count and
/// not as a silence.
///
/// What closed it was `NestedListLog` asking its ordering for its children
/// instead of asking `UWMapLog` for the filtered map; see
/// [`a_sequence_child_with_nothing_written_is_invisible_on_the_generated_path`]
/// for the whole of it. Neither side was weakened to meet the other: the
/// generated path moved onto the answer the interpreted path already gave.
#[test]
fn the_thirty_scripts_find_no_difference_at_all() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let mut sequential = 0;
    let mut concurrent = 0;
    let mut comparisons = 0;
    let mut differing = 0;
    let mut unexplained = Vec::new();
    for seed in 0..30u64 {
        let script = seeded_script(&meta, seed, seed >= 10);
        let here = script
            .steps
            .iter()
            .filter(|step| matches!(step, Step::Edit(_)))
            .count();
        if seed >= 10 {
            concurrent += here;
        } else {
            sequential += here;
        }
        let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
        let check = |harness: &Harness,
                     unexplained: &mut Vec<String>,
                     comparisons: &mut usize,
                     differing: &mut usize,
                     at: String| {
            for writer in ['a', 'b'] {
                *comparisons += 1;
                let interpreted = harness.interp_doc(writer);
                let generated = harness.gen_doc(writer);
                if interpreted == generated {
                    continue;
                }
                *differing += 1;
                unexplained.push(format!(
                    "{} seed {seed} {at} replica {writer}: {}",
                    script.label,
                    difference(&interpreted, &generated, "").unwrap_or_default()
                ));
            }
        };
        for (index, step) in script.steps.iter().enumerate() {
            match step {
                Step::Edit(edit) => {
                    harness
                        .carry(edit)
                        .unwrap_or_else(|reason| panic!("{}: {reason}", script.label));
                    check(&harness, &mut unexplained, &mut comparisons, &mut differing, format!("after edit {index}"));
                }
                Step::Deliver => {
                    harness.cross();
                    check(&harness, &mut unexplained, &mut comparisons, &mut differing, format!("after delivery {index}"));
                }
            }
        }
        harness.cross();
        check(&harness, &mut unexplained, &mut comparisons, &mut differing, "at the end".to_string());
    }
    let edits = sequential + concurrent;
    assert!(
        edits > 30 * 10,
        "thirty scripts came to only {edits} edits; the generator stopped early"
    );
    assert!(
        unexplained.is_empty(),
        "{} of {comparisons} comparisons differ, and the two paths are supposed \
         to agree everywhere now:\n\n{}",
        unexplained.len(),
        unexplained.join("\n")
    );
    assert_eq!(differing, 0, "counted separately from the report above");
    println!(
        "30 scripts, {edits} edits ({sequential} over the ten sequential, \
         {concurrent} over the twenty concurrent), {comparisons} comparisons, \
         {differing} of them differing"
    );
}

/// **A generated log with a non-empty ordered containment cannot be
/// serialized at all, so this metamodel cannot be state-transferred.**
///
/// Found while building the state-transfer oracle `ip32`
/// (`generated/classdiagram_crdt/tests/equivalence.rs`), which needed a
/// metamodel whose generated log goes on the wire. This one does not.
///
/// `Root.behaviortrees` compiles to `NestedListLog<BehaviorTreeLog>`, whose
/// children are a `UWMapLog<EventId, L>`
/// (`moirai-crdt/src/list/nested_list.rs:61-66`), which is a `HashMap` keyed
/// by `EventId` — a struct. A JSON object's keys are strings, so `serde_json`
/// answers `key must be a string` the moment that map holds anything. An
/// empty log serializes fine, which is why nothing noticed until a log with
/// content had to travel.
///
/// # What it costs
///
/// `moirai-network`'s state transfer is JSON end to end:
/// `TransferableLog::export_log` is `serde_json::to_value`
/// (`moirai-network/src/state_transfer.rs:49-60`) and returns
/// `Value::Null` on failure, and `GenericNode` measures the serialized length
/// before serving. So a donor hosting a `bt.ecore` model with one behaviour
/// tree in it cannot serve a state transfer for it, and a joiner falls back
/// to a delta sync — which reaches only the unstable suffix, so the compacted
/// prefix stays out of reach. The three other checked-in metamodels divide
/// the same way: `bt.ecore`, `json.ecore` and `kitchen_sink.ecore` all
/// produce a `NestedListLog`, and `class_diagram.ecore` is the one that does
/// not, which is why `ip32` runs there.
///
/// The interpreted path does not have this problem for the same model:
/// `SeqNode`'s children are a `BTreeMap<EventId, Node>` serialized as a list
/// of pairs precisely because the key is not a string
/// (`moirai-interp/src/node.rs:565-587`).
///
/// `moirai-crdt` is out of scope for this branch, so this is a pin and not a
/// fix: a `UWMapLog` that serialized as pair lists would make this test fail,
/// which is the point.
#[test]
fn a_generated_log_with_a_non_empty_ordered_containment_cannot_be_serialized() {
    use moirai_crdt::utils::membership::twins_log;
    use moirai_protocol::broadcast::tcsb::Tcsb;
    use moirai_protocol::replica::Replica;

    type Gen = Replica<bt_crdt::package::BehaviortreeLog, Tcsb<bt_crdt::package::Behaviortree>>;
    let (mut a, mut b): (Gen, Gen) = twins_log::<bt_crdt::package::BehaviortreeLog>();

    // Empty, it serializes: an empty `HashMap` is an empty JSON object.
    let empty = serde_json::to_string(a.log());
    assert!(
        empty.is_ok(),
        "an empty generated log has to serialize, or this test is measuring \
         something else: {:?}",
        empty.err()
    );

    // One behaviour tree in the ordered containment.
    let insert: bt_crdt::package::Behaviortree = serde_json::from_value(
        json!({"Root": {"Behaviortrees": {"Insert": {"pos": 0, "op": "New"}}}}),
    )
    .expect("the insert shape is the one `record!` writes");
    let event = a.send(insert).expect("the log takes it");
    b.receive(event);

    let error = serde_json::to_string(a.log())
        .expect_err("a `HashMap<EventId, _>` cannot become a JSON object");
    assert_eq!(
        error.to_string(),
        "key must be a string",
        "the failure has to be the key and not something else"
    );
    // And `to_value`, which is the call `export_log` actually makes.
    assert!(
        serde_json::to_value(a.log()).is_err(),
        "`TransferableLog::export_log` would hand `Value::Null` to the wire"
    );
    eprintln!(
        "bt: an empty generated log is {} B of JSON; one with a single \
         behaviour tree in `Root.behaviortrees` cannot be serialized at all \
         ({error})",
        empty.expect("checked above").len()
    );
}

// ---------------------------------------------------------------------------
// The conflict matrix over `bt.ecore`
// ---------------------------------------------------------------------------
//
// `moirai_interp::matrix` holds the whole matrix and assigns this crate the
// three structural rows no other metamodel with a generated crate reaches: the
// optional attribute (`TreeNode.name`), the single containment onto a union
// with eight concrete subtypes (`BehaviorTree.child`) and the ordered
// containment onto that union (`ControlNode.children`). The cells run on
// `support`'s encoders and on exactly the comparison `Harness::compare` makes,
// the canonical projection and nothing else. They used to take I-A1's one
// named exception off both sides first; there is no exception to take off.
//
// The interpreted arm here is `moirai_interp::testing::Harness` rooted at
// `Root` rather than a `ModelLog`: with no `Install` in front of it, the
// opening `Create main` is event one on both paths and every later event
// lines up by construction, which the runner asserts on every send. Every
// cell acknowledges by writing one character into `Root.main.ID`, which no
// cell contends.

use moirai_interp::matrix::{self, Arm, Cell, Construction, pattern as p};

type MatrixInterp = moirai_protocol::replica::Replica<
    moirai_interp::testing::Harness,
    moirai_protocol::broadcast::tcsb::Tcsb<InstanceOp>,
>;
type MatrixGen = moirai_protocol::replica::Replica<
    bt_crdt::package::BehaviortreeLog,
    moirai_protocol::broadcast::tcsb::Tcsb<bt_crdt::package::Behaviortree>,
>;

fn hop(feature: &str, at: Option<usize>, class: &str) -> Hop {
    Hop {
        feature: feature.to_string(),
        at,
        class: class.to_string(),
    }
}

fn main_path() -> Path {
    Path::default().child(hop("main", None, "BehaviorTree"))
}

/// `/main/child`, holding a `class`.
fn child_path(class: &str) -> Path {
    main_path().child(hop("child", None, class))
}

/// `/main/child:Sequence/children[at]`, holding a `class`.
fn kid_path(at: usize, class: &str) -> Path {
    child_path("Sequence").child(hop("children", Some(at), class))
}

fn at(path: Path, action: Action) -> Edit {
    Edit {
        id: 0,
        writer: 'a',
        path,
        action,
    }
}

fn create(path: Path, feature: &str, pos: Option<usize>, class: &str) -> Edit {
    at(
        path,
        Action::Create {
            feature: feature.to_string(),
            pos,
            class: class.to_string(),
        },
    )
}

fn type_char(path: Path, feature: &str, pos: usize, ch: char) -> Edit {
    at(
        path,
        Action::Text {
            feature: feature.to_string(),
            op: TextOp::Insert {
                pos,
                ch,
                after: ch.to_string(),
            },
        },
    )
}

fn beat() -> Edit {
    type_char(main_path(), "ID", 0, 'h')
}

fn bt_cells(meta: &Meta) -> Vec<Cell<Edit>> {
    let opened = |extra: Vec<Edit>| {
        let mut out = open_the_model(meta);
        out.extend(extra);
        out
    };
    let mut cells = Vec::new();

    // The single containment onto `TreeNode`.
    let row = Construction::SingleContainment;
    let put = |class: &str, ch: char| {
        vec![
            create(main_path(), "child", None, class),
            type_char(child_path(class), "ID", 0, ch),
        ]
    };
    cells.push(
        Cell::new(row, p::DIFFERENT_SUBTYPES, opened(vec![]), vec![put("Sequence", 's'), put("Fallback", 'f')], beat())
            .expect(
                "/main/child",
                json!({CONFLICT: [{"eClass": "Fallback", "ID": "f"}, {"eClass": "Sequence", "ID": "s"}]}),
            ),
    );
    cells.push(
        Cell::new(row, p::SAME_SUBTYPE, opened(vec![]), vec![put("Sequence", 'a'), put("Sequence", 'b')], beat())
            .expect("/main/child/eClass", json!("Sequence"))
            .watch("/main/child"),
    );
    cells.push(
        Cell::new(
            row,
            p::UPDATE_UPDATE_CHILD,
            opened(vec![create(main_path(), "child", None, "Sequence"), type_char(child_path("Sequence"), "ID", 0, 'x')]),
            vec![
                vec![type_char(child_path("Sequence"), "ID", 1, 'a')],
                vec![type_char(child_path("Sequence"), "ID", 1, 'b')],
            ],
            beat(),
        )
        .expect("/main/child/eClass", json!("Sequence"))
        .watch("/main/child"),
    );

    // The optional attribute `TreeNode.name`, on the `Sequence` at `/main/child`.
    let row = Construction::OptionalAttribute;
    let sequence = || vec![create(main_path(), "child", None, "Sequence")];
    let named = || {
        let mut out = sequence();
        out.push(type_char(child_path("Sequence"), "name", 0, 'x'));
        opened(out)
    };
    let unset = || vec![at(child_path("Sequence"), Action::Unset { feature: "name".to_string() })];
    // Update-wins: the character concurrent with the unset survives it, the
    // one causally below it does not.
    cells.push(
        Cell::new(
            row,
            p::SET_UNSET,
            named(),
            vec![vec![type_char(child_path("Sequence"), "name", 1, 'y')], unset()],
            beat(),
        )
        .expect("/main/child/name", json!("y")),
    );
    cells.push(
        Cell::new(
            row,
            p::SET_SET,
            opened(sequence()),
            vec![
                vec![type_char(child_path("Sequence"), "name", 0, 'a')],
                vec![type_char(child_path("Sequence"), "name", 0, 'b')],
            ],
            beat(),
        )
        .watch("/main/child/name"),
    );
    // Present and empty on both paths, which is `an_emptied_optional_is_not_dropped`'s rule.
    cells.push(
        Cell::new(row, p::UNSET_UNSET, named(), vec![unset(), unset()], beat())
            .expect("/main/child/name", json!("")),
    );

    // The ordered containment `ControlNode.children` onto `TreeNode`, on the
    // `Sequence` at `/main/child`, seeded with two named `Fallback`s.
    let row = Construction::SequenceContainment;
    let seeded = || {
        let mut out = sequence();
        out.push(create(child_path("Sequence"), "children", Some(0), "Fallback"));
        out.push(type_char(kid_path(0, "Fallback"), "ID", 0, 'p'));
        out.push(create(child_path("Sequence"), "children", Some(1), "Fallback"));
        out.push(type_char(kid_path(1, "Fallback"), "ID", 0, 'q'));
        opened(out)
    };
    let insert = |pos: usize, class: &str, ch: char| {
        vec![
            create(child_path("Sequence"), "children", Some(pos), class),
            type_char(kid_path(pos, class), "ID", 0, ch),
        ]
    };
    let delete = |pos: usize| {
        vec![at(child_path("Sequence"), Action::Delete { feature: "children".to_string(), pos })]
    };
    let rename = |pos: usize, ch: char| vec![type_char(kid_path(pos, "Fallback"), "ID", 1, ch)];
    let ids = |doc: &[&str]| -> Value {
        Value::Array(doc.iter().map(|id| json!({"eClass": "Fallback", "ID": id})).collect())
    };
    cells.push(
        Cell::new(
            row,
            p::INSERT_INSERT_SAME_POS,
            seeded(),
            vec![insert(1, "Fallback", 'a'), insert(1, "Sequence", 'b')],
            beat(),
        )
        .watch("/main/child/children"),
    );
    cells.push(
        Cell::new(row, p::INSERT_DELETE, seeded(), vec![insert(1, "Fallback", 'a'), delete(0)], beat())
            .expect("/main/child/children", ids(&["a", "q"])),
    );
    // Update-wins: the removed child comes back holding only what was written
    // into it concurrently with the removal.
    cells.push(
        Cell::new(row, p::DELETE_UPDATE_SAME, seeded(), vec![delete(0), rename(0, 'z')], beat())
            .expect("/main/child/children", ids(&["z", "q"])),
    );
    cells.push(
        Cell::new(row, p::DELETE_DELETE_SAME, seeded(), vec![delete(0), delete(0)], beat())
            .expect("/main/child/children", ids(&["q"])),
    );
    cells.push(
        Cell::new(row, p::UPDATE_UPDATE_SAME, seeded(), vec![rename(0, 'a'), rename(0, 'b')], beat())
            .watch("/main/child/children"),
    );
    cells.push(
        Cell::new(
            row,
            p::THREE_INSERTS_SAME_POS,
            seeded(),
            vec![insert(1, "Fallback", 'a'), insert(1, "Sequence", 'b'), insert(1, "Fallback", 'c')],
            beat(),
        )
        .watch("/main/child/children"),
    );
    cells
}

/// **The conflict matrix** over `bt.ecore`: every cell the registry assigns to
/// this crate, each under every schedule.
#[test]
fn conflict_matrix_over_bt_ecore() {
    let sem = Arc::new(from_descriptor(&bt_descriptor()).expect("the descriptor parses"));
    let meta = Meta::new(Arc::clone(&sem));
    moirai_interp::testing::install_fixture(&sem, "Root");
    let interp_encode = |edit: &Edit, _: &Value| match interp_op(&meta, edit) {
        ModelOp::Instance(op) => op,
        other => panic!("an edit encodes as an instance operation, not {other:?}"),
    };
    let gen_encode = |edit: &Edit, _: &Value| typed_op(&meta, edit);
    let interp_read =
        |replica: &MatrixInterp| canon(&meta, replica.query(&Read::<Value>::new()));
    let gen_read = |replica: &MatrixGen| {
        project(&meta, &replica.query(&Read::<bt_crdt::package::BehaviortreeValue>::new()))
    };
    let interp = Arm {
        name: "interpreted",
        encode: &interp_encode,
        read: &interp_read,
    };
    let generated = Arm {
        name: "generated",
        encode: &gen_encode,
        read: &gen_read,
    };
    let cells = bt_cells(&meta);
    matrix::run_matrix(matrix::BT, &sem, &cells, &interp, &generated)
        .unwrap_or_else(|reason| panic!("{reason}"));
}
