//! Immutable model facts shared by every generator in one generation run.

use std::{
    collections::{BTreeSet, HashSet},
    ops::Deref,
};

use anyhow::{Result, ensure};
use ecore_rs::{
    ctx::Ctx,
    repr::{Class, Pack, idx, structural},
};
use syn::Ident;

use super::{
    annotation::{transparent_field, uw_map_spec},
    classifier::is_instantiable_class,
    cycles::{CycleAnalysis, analyze_cycles},
    ident::classifier_type_names,
    reference::analysis::ReferenceAnalysis,
};
use crate::{ArachneError, utils::topo::topological_sort};

/// Stores inheritance and instantiation facts for one Ecore class.
pub struct ClassInfo {
    /// Inheritance closure, including self and excluding uninhabited classes.
    pub family: Vec<idx::Class>,
    /// Instantiable members of the inheritance closure, including self when concrete.
    pub concrete_family: Vec<idx::Class>,
    /// Direct union variants in generated order: self, then inhabited subclasses.
    pub variants: Vec<idx::Class>,
    pub uninhabited: bool,
    pub polymorphic: bool,
    /// A helper referenced exclusively by UW-map containment features.
    pub map_entry: bool,
    pub wrapper: bool,
    pub wrapper_descendant: bool,
    pub transparent_field: Option<String>,
}

/// Stores classifier names and inheritance facts once, indexed by Ecore class ID.
pub struct ModelIndex<'a> {
    ctx: &'a Ctx,
    names: Vec<Ident>,
    classes: Vec<ClassInfo>,
}

impl Deref for ModelIndex<'_> {
    type Target = Ctx;
    fn deref(&self) -> &Ctx {
        self.ctx
    }
}

impl<'a> ModelIndex<'a> {
    pub fn new(ctx: &'a Ctx) -> Result<Self> {
        let count = ctx.classes().len();
        let mut families = vec![None; count];
        let mut visiting = vec![false; count];
        for class in ctx.classes() {
            inheritance_family(ctx, class.idx, &mut families, &mut visiting)?;
        }
        let families = families.into_iter().map(Option::unwrap).collect::<Vec<_>>();
        let concretes = families
            .iter()
            .map(|family| {
                family
                    .iter()
                    .copied()
                    .filter(|idx| is_instantiable_class(ctx.class(*idx)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let uninhabited = ctx
            .classes()
            .iter()
            .map(|class| {
                (class.is_abstract() || class.is_interface()) && concretes[*class.idx].is_empty()
            })
            .collect::<Vec<_>>();
        // One pass over incoming features replaces a complete model scan per classifier.
        let mut incoming = vec![false; count];
        let mut map_only = vec![true; count];
        for feature in ctx.classes().iter().flat_map(Class::structural) {
            if let Some(target) = feature.typ {
                incoming[*target] = true;
                map_only[*target] &= feature.kind == structural::Typ::EReference
                    && feature.containment
                    && uw_map_spec(feature).is_some();
            }
        }
        let transparent = ctx
            .classes()
            .iter()
            .map(transparent_field)
            .collect::<Vec<_>>();
        let wrappers = ctx
            .classes()
            .iter()
            .map(|class| {
                is_instantiable_class(class)
                    && transparent[*class.idx].is_none()
                    && !(incoming[*class.idx] && map_only[*class.idx])
            })
            .collect::<Vec<_>>();
        let classes = ctx
            .classes()
            .iter()
            .map(|class| {
                let index = *class.idx;
                let children = class
                    .sub()
                    .iter()
                    .copied()
                    .filter(|idx| !uninhabited[**idx]);
                let variants = std::iter::once(class.idx)
                    .filter(|_| is_instantiable_class(class))
                    .chain(children.clone())
                    .collect();
                ClassInfo {
                    family: families[index]
                        .iter()
                        .copied()
                        .filter(|idx| !uninhabited[**idx])
                        .collect(),
                    concrete_family: concretes[index].clone(),
                    variants,
                    uninhabited: uninhabited[index],
                    polymorphic: if class.is_abstract() || class.is_interface() {
                        !uninhabited[index]
                    } else {
                        children.count() > 0
                    },
                    map_entry: is_instantiable_class(class) && incoming[index] && map_only[index],
                    wrapper: wrappers[index],
                    wrapper_descendant: families[index]
                        .iter()
                        .any(|idx| *idx != class.idx && wrappers[**idx]),
                    transparent_field: transparent[index].clone(),
                }
            })
            .collect();
        Ok(Self {
            ctx,
            names: classifier_type_names(ctx),
            classes,
        })
    }

    pub fn info(&self, class: idx::Class) -> &ClassInfo {
        &self.classes[*class]
    }
    pub fn classifier_name(&self, class: idx::Class) -> &Ident {
        &self.names[*class]
    }
}

fn inheritance_family(
    ctx: &Ctx,
    class: idx::Class,
    families: &mut [Option<Vec<idx::Class>>],
    visiting: &mut [bool],
) -> Result<()> {
    if families[*class].is_some() {
        return Ok(());
    }
    ensure!(
        !visiting[*class],
        "Inheritance cycle involving class '{}'",
        ctx.class(class).name()
    );
    visiting[*class] = true;
    let mut family = BTreeSet::from([class]);
    for child in ctx.class(class).sub() {
        inheritance_family(ctx, *child, families, visiting)?;
        family.extend(families[**child].as_ref().unwrap().iter().copied());
    }
    visiting[*class] = false;
    families[*class] = Some(family.into_iter().collect());
    Ok(())
}

/// Owns package selection, roots, reachable classes and graph analyses for one run.
pub struct GenerationPlan<'a> {
    /// Cached classifier names and inheritance facts for the complete parsed model.
    pub model: ModelIndex<'a>,
    /// The EPackage selected for package, reference and XMI generation.
    pub pack_idx: idx::Pack,
    /// Top-level package roots used to discover reachable classes and generate root logs.
    pub roots: Vec<idx::Class>,
    /// Reachable classifiers in the selected package, shared by references and XMI.
    /// Iteration follows class-ID order; membership lookup is O(log n).
    pub reachable_classes: BTreeSet<idx::Class>,
    /// Classifier generation order: parents before subclasses, including model-wide enums.
    /// A vector preserves this topological order, which can differ from class-ID order.
    pub generation_order: Vec<idx::Class>,
    /// Concrete inheritance members in the selected package, indexed by class ID.
    /// Classes outside that package have empty entries.
    pub concrete_families: Vec<Vec<idx::Class>>,
    /// Shared containment boxing decisions for classifier and XMI generation.
    pub cycles: CycleAnalysis,
    /// Concrete non-containment references and their shared generated names.
    pub references: ReferenceAnalysis,
}

impl<'a> GenerationPlan<'a> {
    pub fn new(ctx: &'a Ctx, pack: &Pack) -> Result<Self> {
        let model = ModelIndex::new(ctx)?;
        let package_classes = pack.classes().iter().copied().collect::<Vec<_>>();
        let package_set = package_classes.iter().copied().collect::<HashSet<_>>();
        let concrete_families = ctx
            .classes()
            .iter()
            .map(|class| {
                if package_set.contains(&class.idx) {
                    model
                        .info(class.idx)
                        .concrete_family
                        .iter()
                        .copied()
                        .filter(|idx| package_set.contains(idx))
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                }
            })
            .collect::<Vec<_>>();
        let incoming = concrete_containment_incoming(ctx, &package_classes, &concrete_families);
        let mut roots = package_classes
            .iter()
            .copied()
            .filter(|idx| is_instantiable_class(ctx.class(*idx)) && !incoming.contains(idx))
            .collect::<Vec<_>>();
        if roots.is_empty() {
            roots = package_classes
                .iter()
                .copied()
                .filter(|idx| {
                    let class = ctx.class(*idx);
                    (class.is_abstract() || class.is_interface())
                        && !class.is_enum()
                        && !concrete_families[**idx].is_empty()
                        && family_has_no_external_container(
                            ctx,
                            *idx,
                            &package_classes,
                            &package_set,
                            &concrete_families,
                        )
                })
                .collect();
        }
        if roots.is_empty() {
            return Err(ArachneError::RootClassNotFound(pack.name().to_string()).into());
        }
        let reachable_classes = roots
            .iter()
            .flat_map(|root| reachable_classes(ctx, *root, &package_set))
            .collect::<BTreeSet<_>>();
        let generated = ctx
            .classes()
            .iter()
            .filter(|class| reachable_classes.contains(&class.idx) || class.is_enum())
            .collect::<Vec<_>>();
        let generation_order = topological_sort(ctx, &generated)
            .iter()
            .map(|class| class.idx)
            .collect();
        let cycles = analyze_cycles(&model)?;
        let references = ReferenceAnalysis::analyze(&model, &reachable_classes, &concrete_families);
        Ok(Self {
            model,
            pack_idx: pack.idx,
            roots,
            reachable_classes,
            generation_order,
            concrete_families,
            cycles,
            references,
        })
    }
}

fn reachable_classes(
    ctx: &Ctx,
    root: idx::Class,
    package: &HashSet<idx::Class>,
) -> HashSet<idx::Class> {
    let mut reachable = HashSet::new();
    let mut stack = vec![root];
    while let Some(idx) = stack.pop() {
        if !package.contains(&idx) || !reachable.insert(idx) {
            continue;
        }
        let class = ctx.class(idx);
        stack.extend(class.sup().iter().copied());
        stack.extend(class.sub().iter().copied());
        stack.extend(
            class
                .structural()
                .iter()
                .filter(|feature| {
                    feature.kind == structural::Typ::EReference && feature.containment
                })
                .filter_map(|feature| feature.typ),
        );
    }
    reachable
}

fn concrete_containment_incoming(
    ctx: &Ctx,
    classes: &[idx::Class],
    families: &[Vec<idx::Class>],
) -> HashSet<idx::Class> {
    let mut incoming = HashSet::new();
    for source in classes {
        for target in ctx
            .class(*source)
            .structural()
            .iter()
            .filter(|feature| feature.kind == structural::Typ::EReference && feature.containment)
            .filter_map(|feature| feature.typ)
        {
            for concrete in &families[*target] {
                if families[**source].iter().any(|source| source != concrete) {
                    incoming.insert(*concrete);
                }
            }
        }
    }
    incoming
}

fn family_has_no_external_container(
    ctx: &Ctx,
    class: idx::Class,
    classes: &[idx::Class],
    package: &HashSet<idx::Class>,
    families: &[Vec<idx::Class>],
) -> bool {
    let family = &families[*class];
    let context = reachable_classes(ctx, class, package);
    !classes.iter().any(|source| {
        families[**source]
            .iter()
            .any(|source| !context.contains(source))
            && ctx
                .class(*source)
                .structural()
                .iter()
                .filter(|feature| {
                    feature.kind == structural::Typ::EReference && feature.containment
                })
                .filter_map(|feature| feature.typ)
                .any(|target| {
                    families[*target]
                        .iter()
                        .any(|target| family.contains(target))
                })
    })
}
