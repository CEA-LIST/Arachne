//! Local Ecore resource resolution for `describe`.
//!
//! The parser deliberately resolves only names within a resource and Ecore built-ins.
//! Rewrite relative resource references to qualified local forward references, then
//! define those references through the parser's public context API before finalizing.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use anyhow::{Context, bail};
use ecore_rs::{
    ctx::Ctx,
    parser,
    repr::{self, ecore, idx},
};
use regex::Regex;
use serde_json::{Value, json};

use crate::{ArachneError, EcoreParser, codegen::descriptor, find_user_package};

pub(crate) struct Resources {
    pub parser: EcoreParser,
    pub descriptor: Value,
    dependencies: BTreeMap<String, Value>,
    origins: BTreeMap<String, String>,
}

/// What `arachne describe` writes: the descriptor, and the descriptor of each model it depends on, by digest.
pub struct Described {
    pub descriptor: Value,
    dependencies: BTreeMap<String, Value>,
}

impl Described {
    pub fn write_dependencies(&self, output: &Path) -> anyhow::Result<()> {
        if !self.dependencies.is_empty() {
            let directory = output.join("dependencies");
            fs::create_dir_all(&directory)?;
            for (digest, descriptor) in &self.dependencies {
                fs::write(
                    directory.join(format!("{digest}.metamodel.json")),
                    format!("{descriptor:#}\n"),
                )?;
            }
        }
        Ok(())
    }
}

/// Describes `path`. The file is read alone first, so every file that was described before keeps
/// its descriptor and, when it is refused, the parser's own error. Only a file the parser refuses
/// is resolved against the resources it names; when that fails too, the parser's error stands.
pub fn describe(path: &Path) -> crate::Result<Described> {
    // Describe reads a package referenced before its definition; `generate` refuses it.
    let first = fs::read_to_string(path)
        .map_err(ArachneError::from)
        .and_then(|text| {
            Ok(EcoreParser {
                ctx: Ctx::parse_filling_forward_packs(text)?,
            })
        });
    match first {
        Ok(parser) => Ok(Described {
            descriptor: descriptor::descriptor_json(&parser.ctx, find_user_package(&parser.ctx)?)?,
            dependencies: BTreeMap::new(),
        }),
        Err(refused) => {
            // The parser already logged this file's errors; the second read must stay silent.
            let level = log::max_level();
            log::set_max_level(log::LevelFilter::Off);
            let resolved = load(path);
            log::set_max_level(level);
            resolved
                .map(|resources| Described {
                    descriptor: resources.descriptor.clone(),
                    dependencies: resources.dependencies.clone(),
                })
                .map_err(|_| refused)
        }
    }
}

impl Resources {
    fn qualified_name(&self, class: idx::Class) -> anyhow::Result<String> {
        let name = self.parser.ctx[class].name();
        Ok(if self.origins.contains_key(name) {
            name.to_owned()
        } else {
            format!("{}::{name}", find_user_package(&self.parser.ctx)?.name())
        })
    }
}

pub(crate) fn load(path: &Path) -> anyhow::Result<Rc<Resources>> {
    Loader::default().load(path)
}

#[derive(Default)]
struct Loader {
    loaded: BTreeMap<PathBuf, Rc<Resources>>,
    active: BTreeSet<PathBuf>,
}

impl Loader {
    fn load(&mut self, path: &Path) -> anyhow::Result<Rc<Resources>> {
        let path = fs::canonicalize(path)
            .with_context(|| format!("Cannot resolve Ecore resource {}", path.display()))?;
        if let Some(resource) = self.loaded.get(&path) {
            return Ok(Rc::clone(resource));
        }
        if !self.active.insert(path.clone()) {
            bail!("Cyclic Ecore resource dependencies at {}", path.display());
        }
        let content = fs::read_to_string(&path)?;
        let mut requested = BTreeMap::new();
        let mut dependencies = BTreeMap::new();
        let rewritten = rewrite_references(&content, |reference| {
            // Ecore's existing built-in path must remain with the legacy parser.
            if ecore::classifier_path(reference).is_some() {
                return Ok(None);
            }
            let Some((file, classifier)) = reference.split_once("#//") else {
                return Ok(None);
            };
            if file.is_empty() {
                return Ok(None);
            }
            if file.contains(':') || Path::new(file).is_absolute() {
                bail!("Only relative local Ecore resources are supported: {reference}");
            }
            let dependency = self.load(&path.parent().unwrap().join(file))?;
            let pack = find_user_package(&dependency.parser.ctx)?;
            let class = pack
                .classes()
                .iter()
                .copied()
                .find(|idx| dependency.parser.ctx[*idx].name() == classifier)
                .with_context(|| format!("Missing classifier in resource reference {reference}"))?;
            let qualified = dependency.qualified_name(class)?;
            if let Some((previous, _)) = requested.get(&qualified)
                && !Rc::ptr_eq(previous, &dependency)
            {
                bail!("Conflicting dependency classifier {qualified}");
            }
            requested.insert(qualified.clone(), (Rc::clone(&dependency), class));
            dependencies.extend(dependency.dependencies.clone());
            dependencies.insert(
                descriptor::metamodel_digest(&dependency.descriptor),
                dependency.descriptor.clone(),
            );
            Ok(Some(format!("#//{qualified}")))
        })?;

        let mut ctx = Ctx::with_capacity_filling_forward_packs(7, 7);
        parser::parse(&rewritten, &mut ctx).map_err(ArachneError::from)?;
        let pack = find_user_package(&ctx)?;
        let pack_path = pack.path(&ctx);
        let own_classes: Vec<_> = pack
            .classes()
            .iter()
            .copied()
            .filter(|class| !requested.contains_key(ctx[*class].name()))
            .collect();
        let mut origins = BTreeMap::new();
        let mut imported = BTreeMap::new();
        for (dependency, class) in requested.values() {
            import_class(
                dependency,
                *class,
                &mut ctx,
                &pack_path,
                &mut imported,
                &mut origins,
            )?;
        }
        ctx.finalize().map_err(ArachneError::from)?;
        let pack = find_user_package(&ctx)?;
        let mut descriptor = descriptor::descriptor_with_roots(&ctx, pack, &own_classes)?;
        for (name, origin) in &origins {
            if let Some(class) = descriptor["classes"].get_mut(name) {
                class["origin"] = json!(origin);
            }
        }
        if !dependencies.is_empty() {
            let mut identities: Vec<_> = dependencies
                .iter()
                .map(|(digest, d)| json!({"nsURI": d["nsURI"], "digest": digest}))
                .collect();
            identities.sort_by_key(Value::to_string);
            descriptor["dependsOn"] = json!(identities);
        }
        let resource = Rc::new(Resources {
            parser: EcoreParser { ctx },
            descriptor,
            dependencies,
            origins,
        });
        self.active.remove(&path);
        self.loaded.insert(path, Rc::clone(&resource));
        Ok(resource)
    }
}

/// Copy exactly the upward/feature-target closure, never a target's subclasses.
/// Allocate before following edges so recursive feature types terminate.
fn import_class(
    resource: &Resources,
    source: idx::Class,
    destination: &mut Ctx,
    path: &repr::Path,
    imported: &mut BTreeMap<String, idx::Class>,
    origins: &mut BTreeMap<String, String>,
) -> anyhow::Result<idx::Class> {
    let ctx = &resource.parser.ctx;
    if let Some((typ, _)) = ctx.builtins().iter().find(|(_, idx)| **idx == source) {
        return Ok(destination
            .get_builtin_idx(typ)
            .map_err(ArachneError::from)?);
    }
    if let Some((typ, _)) = ctx.ecore_classes().iter().find(|(_, idx)| **idx == source) {
        return Ok(destination
            .ecore_class_idx(*typ)
            .map_err(ArachneError::from)?);
    }
    let class = &ctx[source];
    let name = resource.qualified_name(source)?;
    let origin = resource
        .origins
        .get(class.name())
        .cloned()
        .or_else(|| find_user_package(ctx).ok()?.ns_uri().map(str::to_owned))
        .context("Dependency package must declare an nsURI")?;
    if let Some(idx) = imported.get(&name) {
        if origins.get(&name) != Some(&origin) {
            bail!("Conflicting dependency classifier {name}");
        }
        return Ok(*idx);
    }
    let index = destination
        .add_class(
            path.clone(),
            class.typ().clone(),
            &name,
            class.inst_name(),
            Some(class.is_abstract()),
            Some(class.is_interface()),
        )
        .map_err(ArachneError::from)?;
    imported.insert(name.clone(), index);
    origins.insert(name, origin);
    if let Some(name) = class.instance_class_name() {
        destination[index].set_instance_class_name(name);
    }
    for annotation in class.annotations() {
        destination[index].add_annotation(annotation.clone());
    }
    for literal in class.literals() {
        destination[index].add_literal(literal.clone());
    }
    for parent in class.sup() {
        let parent = import_class(resource, *parent, destination, path, imported, origins)?;
        destination.add_sup_class(parent, index);
    }
    for feature in class.structural() {
        let mut feature = feature.clone();
        feature.typ = feature
            .typ
            .map(|target| import_class(resource, target, destination, path, imported, origins))
            .transpose()?;
        destination[index].add_structural(feature);
    }
    Ok(index)
}

/// Locate quoted type attributes in XML tags, excluding comments, processing
/// instructions and CDATA. The existing Ecore parser still validates the XML.
/// Only attribute values change; annotations and all rule declarations stay intact.
fn rewrite_references(
    content: &str,
    mut resolve: impl FnMut(&str) -> anyhow::Result<Option<String>>,
) -> anyhow::Result<String> {
    let tags = Regex::new(
        r#"(?s)<!--.*?-->|<\?.*?\?>|<!\[CDATA\[.*?\]\]>|<(?:"[^"]*"|'[^']*'|[^'">])*>"#,
    )?;
    let attributes = Regex::new(r#"\s([\w:.-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')"#)?;
    let mut replacements = Vec::new();
    for tag in tags.find_iter(content) {
        if tag.as_str().starts_with("<!") || tag.as_str().starts_with("<?") {
            continue;
        }
        for attribute in attributes.captures_iter(tag.as_str()) {
            if !matches!(&attribute[1], "eType" | "eSuperTypes") {
                continue;
            }
            let value = attribute.get(2).or_else(|| attribute.get(3)).unwrap();
            let mut changed = false;
            let mut words = Vec::new();
            for word in value.as_str().split_whitespace() {
                if let Some(replacement) = resolve(word)? {
                    changed = true;
                    words.push(replacement);
                } else {
                    words.push(word.to_owned());
                }
            }
            if changed {
                // A qualified eType may carry an optional `ecore:EClass` prefix;
                // the parser's local-name form has no such prefix.
                if &attribute[1] == "eType" && words.len() == 2 && words[0].starts_with("ecore:") {
                    words.remove(0);
                }
                replacements.push((
                    tag.start() + value.start()..tag.start() + value.end(),
                    words.join(" "),
                ));
            }
        }
    }
    let mut rewritten = content.to_owned();
    for (range, value) in replacements.into_iter().rev() {
        rewritten.replace_range(range, &value);
    }
    Ok(rewritten)
}
