//! Structure-first imports. The node's identified tree is the restart record; no
//! structural operation is replayed over an existing tree.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, anyhow, ensure};
use reqwest::{Url, blocking::Client, redirect::Policy};
use serde_json::json;

use super::*;

pub fn import(descriptor: &Value, project: &Value, node: &str, model: &str) -> Result<()> {
    let node = Node::new(node, model)?;
    let hosted = node.get("metamodel")?;
    let digest = arachne_codegen::metamodel_digest(descriptor);
    ensure!(
        arachne_codegen::metamodel_digest(&hosted) == digest,
        "descriptor mismatch: model {model} is not registered under {digest}"
    );

    let table = Table::new(from_descriptor(descriptor)?, descriptor);
    let mut walk = Walk::new(&table, true);
    walk.document(project);
    ensure!(
        walk.failures.is_empty(),
        "project cannot be carried by the descriptor:\n  {}",
        walk.failures.join("\n  ")
    );
    let mut source_ids = HashSet::new();
    for (element_id, _) in walk.ids.values() {
        ensure!(
            source_ids.insert(element_id),
            "duplicate source id attribute {element_id}"
        );
    }

    let source = &project["content"][0];
    let mut state = node.get("state/identified")?;
    let mut applied = 0;
    if state.is_null() {
        node.submit("structure", &walk.ops)?;
        applied += walk.ops.len();
        state = node.get("state/identified")?;
    }
    // Also checks every anonymous object by containment path. An incomplete or
    // incompatible tree is refused rather than completed by guessing positions.
    let observed = Observed::read(&walk, source, &state)
        .context("incomplete or incompatible structure; refusing structural replay")?;
    for (element_id, _) in walk.ids.values() {
        ensure!(
            observed.by_attribute.contains_key(element_id),
            "no served store identity for source id attribute {element_id}"
        );
    }

    // A complete rerun is read-only. A killed or partial link phase may safely
    // repeat links, but never New, containment Insert, text inserts or counters.
    if check_links(&walk, &observed, source, &state, walk.root, "root").is_err() {
        let ops = walk
            .links
            .as_ref()
            .expect("live walk")
            .iter()
            .map(|link| {
                let target = match &link.target {
                    LinkTarget::Local(id) => observed.by_attribute[id].clone(),
                    LinkTarget::External(value) => value.clone(),
                };
                walk.operation(
                    &link.path,
                    InstanceOp::field(link.slot, InstanceOp::Leaf(LeafOp::Link(target))),
                )
            })
            .collect::<Vec<_>>();
        node.submit("links", &ops)?;
        applied += ops.len();
    }

    let state = node.get("state/identified")?;
    let final_observed =
        Observed::read(&walk, source, &state).context("final structure read-back failed")?;
    ensure!(
        observed.by_path == final_observed.by_path,
        "object identities changed during import"
    );
    check_links(&walk, &final_observed, source, &state, walk.root, "root")
        .context("final link read-back failed")?;
    eprintln!(
        "model: {model}\nmetamodel: {digest}\noperations: {applied} accepted\nread-back: {} objects verified\nreferences: {} resolved, {} unresolved",
        observed.by_path.len(),
        walk.resolved,
        walk.unresolved.len()
    );
    for missing in &walk.unresolved {
        eprintln!(
            "unresolved: {} {} {}",
            missing.holder, missing.feature, missing.value
        );
    }
    Ok(())
}

struct Node {
    client: Client,
    model: Url,
}

impl Node {
    fn new(node: &str, model: &str) -> Result<Self> {
        let mut url = Url::parse(node).context("invalid --node URL")?;
        ensure!(url.scheme() == "http", "--node must use http://");
        ensure!(!model.is_empty(), "--model must not be empty");
        url.path_segments_mut()
            .map_err(|_| anyhow!("--node URL cannot hold model routes"))?
            .pop_if_empty()
            .extend(["api", "model", model]);
        url.set_query(None);
        url.set_fragment(None);
        Ok(Self {
            client: Client::builder()
                .no_proxy()
                .redirect(Policy::none())
                .build()?,
            model: url,
        })
    }

    fn route(&self, leaf: &str) -> Url {
        let mut url = self.model.clone();
        url.path_segments_mut()
            .expect("validated model URL")
            .extend(leaf.split('/'));
        url
    }

    fn get(&self, leaf: &str) -> Result<Value> {
        let url = self.route(leaf);
        let response = self.client.get(url.clone()).send()?;
        let status = response.status();
        let body = response.text()?;
        ensure!(status.is_success(), "GET {url}: HTTP {status}: {body}");
        serde_json::from_str(&body).with_context(|| format!("GET {url}: invalid JSON response"))
    }

    fn submit(&self, phase: &str, ops: &[ModelOp]) -> Result<()> {
        if ops.is_empty() {
            return Ok(());
        }
        let response = self
            .client
            .post(self.route("ops"))
            .json(&json!({"ops": ops}))
            .send()
            .with_context(|| {
                format!("{phase} batch request failed; inspect the model before retrying")
            })?;
        let status = response.status();
        let body = response.text()?;
        ensure!(
            status.is_success(),
            "{phase} batch refused or partially applied: HTTP {status}: {body}"
        );
        let result: Value = serde_json::from_str(&body)
            .with_context(|| format!("{phase} batch returned an invalid report: {body}"))?;
        ensure!(
            result["outcome"] == "applied"
                && result["applied"].as_u64() == Some(ops.len() as u64)
                && result["of"].as_u64() == Some(ops.len() as u64),
            "{phase} batch refused or partially applied: requested {}, report {body}",
            ops.len()
        );
        Ok(())
    }
}

#[derive(Default)]
struct Observed {
    by_attribute: HashMap<String, String>,
    by_path: HashMap<String, String>,
    identities: HashSet<String>,
}

impl Observed {
    fn read(walk: &Walk<'_>, source: &Value, state: &Value) -> Result<Self> {
        let mut observed = Self::default();
        observed.object(walk, source, state, walk.root, "root")?;
        Ok(observed)
    }

    fn object(
        &mut self,
        walk: &Walk<'_>,
        source: &Value,
        got: &Value,
        class: ClassSlot,
        at: &str,
    ) -> Result<()> {
        let table = walk.table;
        ensure!(got["eClass"] == table.name(class), "{at}: class mismatch");
        let id = got["__id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| anyhow!("{at}: missing store identity"))?;
        ensure!(
            self.identities.insert(id.to_owned()),
            "{at}: duplicate store identity {id}"
        );
        self.by_path.insert(at.to_owned(), id.to_owned());
        if let Some(element_id) = got.as_object().and_then(|data| table.id_of(class, data)) {
            ensure!(
                self.by_attribute
                    .insert(element_id.clone(), id.to_owned())
                    .is_none(),
                "{at}: duplicate served id attribute {element_id}"
            );
        }
        let (data, _) = source_parts(source).expect("validated source object");
        for (name, _, _) in &table.sem.classes[class.index()].visible {
            let name = &**name;
            let value = data.get(name).unwrap_or(&Value::Null);
            let actual = &got[name];
            let here = format!("{at}.{name}");
            match table.feature(class, name).and_then(|(_, rule)| rule) {
                Some(MergeRule::Containment { target, .. }) => {
                    let wanted = values_of(value);
                    let children = values_of(actual);
                    ensure!(
                        wanted.len() == children.len(),
                        "{here}: expected {} children, read {}",
                        wanted.len(),
                        children.len()
                    );
                    for (pos, (child, served)) in wanted.into_iter().zip(children).enumerate() {
                        let child_class = child_class(table, child, target)?;
                        self.object(walk, child, served, child_class, &format!("{here}[{pos}]"))?;
                    }
                }
                Some(MergeRule::Attribute { leaf, .. }) if data.contains_key(name) => {
                    let expected = attribute_value(value, leaf)?;
                    ensure!(
                        expected == *actual,
                        "{here}: expected {expected}, read {actual}"
                    );
                }
                Some(MergeRule::Reference { many, .. }) if data.contains_key(name) => {
                    let expected = reference_values(walk, self, value, false)?;
                    compare_reference(&expected, actual, many, &here)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn child_class(table: &Table, child: &Value, target: ClassSlot) -> Result<ClassSlot> {
    match child.get("eClass").and_then(Value::as_str) {
        Some(name) => table
            .class_named(name)
            .ok_or_else(|| anyhow!("unknown class {name}")),
        None => Ok(target),
    }
}

fn attribute_value(value: &Value, leaf: LeafRule) -> Result<Value> {
    Ok(match leaf {
        LeafRule::Flag { .. } => json!(as_flag(value).context("invalid source flag")?),
        LeafRule::Counter {
            num: NumKind::F32 | NumKind::F64,
            ..
        } => {
            json!(as_float(value).context("invalid source counter")?)
        }
        LeafRule::Counter { .. } => json!(as_int(value).context("invalid source counter")?),
        _ => value.clone(),
    })
}

fn reference_values(
    walk: &Walk<'_>,
    observed: &Observed,
    value: &Value,
    identity: bool,
) -> Result<Vec<Value>> {
    values_of(value)
        .into_iter()
        .map(|value| {
            let text = value.as_str().context("non-string reference")?;
            let target = match walk.ids.get(text) {
                Some((element_id, _)) if identity => observed
                    .by_attribute
                    .get(element_id)
                    .ok_or_else(|| anyhow!("no served identity for {element_id}"))?,
                Some((element_id, _)) => element_id,
                None => text,
            };
            Ok(json!(target))
        })
        .collect()
}

fn compare_reference(expected: &[Value], actual: &Value, many: bool, at: &str) -> Result<()> {
    if many {
        let mut expected = expected.to_vec();
        expected.sort_by_key(Value::to_string);
        expected.dedup();
        let mut actual = values_of(actual).into_iter().cloned().collect::<Vec<_>>();
        actual.sort_by_key(Value::to_string);
        ensure!(
            expected == actual,
            "{at}: expected {expected:?}, read {actual:?}"
        );
    } else {
        let expected = expected.first().unwrap_or(&Value::Null);
        ensure!(
            expected == actual,
            "{at}: expected {expected}, read {actual}"
        );
    }
    Ok(())
}

fn check_links(
    walk: &Walk<'_>,
    observed: &Observed,
    source: &Value,
    got: &Value,
    class: ClassSlot,
    at: &str,
) -> Result<()> {
    let (data, _) = source_parts(source).expect("validated source object");
    for (name, value) in data {
        let here = format!("{at}.{name}");
        match walk.table.feature(class, name).and_then(|(_, rule)| rule) {
            Some(MergeRule::Containment { target, .. }) => {
                for (pos, (child, served)) in values_of(value)
                    .into_iter()
                    .zip(values_of(&got[name]))
                    .enumerate()
                {
                    let class = child_class(walk.table, child, target)?;
                    check_links(
                        walk,
                        observed,
                        child,
                        served,
                        class,
                        &format!("{here}[{pos}]"),
                    )?;
                }
            }
            Some(MergeRule::IdentityReference { many, .. }) => {
                let expected = reference_values(walk, observed, value, true)?;
                compare_reference(&expected, &got[name], many, &here)?;
            }
            _ => {}
        }
    }
    Ok(())
}
