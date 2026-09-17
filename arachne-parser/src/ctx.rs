//! Parsing context.
//!
//! The main context is [`Ctx`], which can turn itself into a [`PathCtx`] which is really a [`Ctx`]
//! along with a package [`Path`]. [`PathCtx`]s allow to act on the current package directly during
//! parsing to add annotations, go down a sub-package (yielding a new [`PathCtx`]), or go up the
//! parent package.
//!
//! To add a class to a [`PathCtx`], one first *"enters the class"* by calling
//! [`PathCtx::enter_class`] which yields a [`ClassCtx`]. A class context in turn allows adding
//! annotations, operations, structural features... to the current class before finalizing it
//! and going back to the [`PathCtx`] that spawned it.

prelude! {
    repr::{Class, Pack, Path},
}

/// A plain context, see also [`PathCtx`] and [`ClassCtx`].
///
/// Note that this type needs to be [finalized](Self::finalize).
///
/// # Forward Reference
///
/// Ecore allows classes to be forward referenced, in particular in structural features. To handle
/// this, we have maintain `forward_refs`, a set of pre-declared classes that must be defined before
/// parsing [finalization](Self::finalize).
///
/// When registering a class, the context will automatically update `forward_refs`.
pub struct Ctx {
    packs: idx::PackMap<Pack>,
    top_pack: idx::Pack,
    builtin_pack: idx::Pack,
    name_to_pack: PathMap<String, idx::Pack>,
    classes: idx::ClassMap<Class>,
    name_to_class: PathMap<String, idx::Class>,
    builtin_map: HashMap<builtin::Typ, idx::Class>,
    /// Package of Ecore's own classes, added when the metamodel first refers to one of them.
    ecore_pack: Option<idx::Pack>,
    ecore_map: HashMap<ecore::Typ, idx::Class>,
    forward_ref_classes: BTreeSet<idx::Class>,
    forward_ref_packs: BTreeSet<idx::Pack>,
}

impl std::ops::Index<idx::Class> for Ctx {
    type Output = Class;
    fn index(&self, idx: idx::Class) -> &Self::Output {
        &self.classes[idx]
    }
}
impl std::ops::IndexMut<idx::Class> for Ctx {
    fn index_mut(&mut self, idx: idx::Class) -> &mut Self::Output {
        &mut self.classes[idx]
    }
}

impl std::ops::Index<idx::Pack> for Ctx {
    type Output = Pack;
    fn index(&self, idx: idx::Pack) -> &Self::Output {
        &self.packs[idx]
    }
}
impl std::ops::IndexMut<idx::Pack> for Ctx {
    fn index_mut(&mut self, idx: idx::Pack) -> &mut Self::Output {
        &mut self.packs[idx]
    }
}

impl Ctx {
    /// Name of the root package.
    ///
    /// This package's name is not legal, on purpose, so that it does not clash with user-defined
    /// packages.
    pub const ROOT_PACKAGE_NAME: &str = "[root]";

    /// Name of the package containing builtin classes.
    ///
    /// This package's name is not legal, on purpose, so that it does not clash with user-defined
    /// packages.
    pub const BUILTIN_PACKAGE_NAME: &str = "[builtin]";

    /// Augments the context with builtin types.
    ///
    /// Called by [`Self::with_capacity`], the bottom-most constructor, no need to call this
    /// anywhere else.
    fn populate_builtin(&mut self) -> Res<()> {
        let path = Path::new(self.builtin_pack);
        let builtins = builtin::Typ::builders();
        for (typ, class_builder) in builtins {
            let c_idx = self.raw_add_class(|idx| class_builder(path.clone(), idx))?;
            let _prev = self.builtin_map.insert(typ, c_idx);
            if _prev.is_some() {
                panic!(
                    "[fatal] failed to populate builtin classes, trying to redefine `{:?}`",
                    typ,
                )
            }
        }
        Ok(())
    }

    /// Constructor.
    pub fn with_capacity(pack_capa: usize, class_capa: usize) -> Self {
        let mut packs = idx::PackMap::with_capacity(pack_capa);
        let top_pack = packs.push_idx(|idx| Pack::new(idx, "[root]", None));
        let builtin_pack =
            packs.push_idx(|idx| Pack::new(idx, Self::BUILTIN_PACKAGE_NAME, Some(top_pack)));
        packs[top_pack].add_sub(builtin_pack);
        let mut slf = Self {
            packs,
            top_pack,
            builtin_pack,
            name_to_pack: PathMap::new(),
            classes: idx::ClassMap::with_capacity(class_capa),
            name_to_class: PathMap::new(),
            builtin_map: HashMap::with_capacity(13),
            ecore_pack: None,
            ecore_map: HashMap::new(),
            forward_ref_classes: BTreeSet::new(),
            forward_ref_packs: BTreeSet::new(),
        };
        slf.populate_builtin()
            .expect("[fatal] failed to populate builtin classes");
        slf
    }

    pub fn parse(txt: impl AsRef<str>) -> Res<Self> {
        let mut slf = Self::with_capacity(7, 7);
        parser::raw::Parser::parse(txt.as_ref(), &mut slf)?;
        slf.finalize()?;
        Ok(slf)
    }

    pub fn to_pretty_string(&self) -> String {
        let mut stack = vec![];
        let mut res = String::with_capacity(113);
        let mut current = self.top_pack;

        macro_rules! post {
            ($pref:expr, line $($interp_str:tt)*) => {{
                if !res.is_empty() {
                    res.push('\n');
                }
                res.push_str($pref);
                res.push_str(&format!($($interp_str)*));
            }};
        }

        'go_down: loop {
            let pref = &format!("{0:>1$}", "", stack.len() * 2);
            post!(pref, line "- {} #{}", self[current].name(), self[current].idx);

            // show classes
            let mut classes: Vec<idx::Class> = self[current].classes().iter().cloned().collect();
            classes.sort();
            for c_idx in classes {
                let class = &self[c_idx];
                post!(pref, line "  class {} [{}] #{}", class.name(), class.typ(), class.idx);

                if !class.sup().is_empty() {
                    let sups = class
                        .sup()
                        .iter()
                        .cloned()
                        .show_iter_cs(|idx| self[idx].name());
                    post!(pref, line "    supers: {}", sups);
                } else {
                    // post!(pref, line "    no super classes");
                }

                if !class.sub().is_empty() {
                    let subs = class
                        .sub()
                        .iter()
                        .cloned()
                        .show_iter_cs(|idx| self[idx].name());
                    post!(pref, line "    subers: {}", subs);
                } else {
                    // post!(pref, line "    no suber classes");
                }

                if !class.structural().is_empty() {
                    post!(pref, line "    structural features:");
                    for tural in class.structural() {
                        let typ_name = if let Some(typ_idx) = tural.typ {
                            self[typ_idx].name().to_string()
                        } else if let Some(typ_path) = &tural.typ_path {
                            format!("(external: {})", typ_path)
                        } else {
                            "(unknown)".to_string()
                        };
                        post!(pref, line "    - `{}`: {} of {} `{}`", tural.name, tural.kind, tural.bounds, typ_name);
                    }
                }
            }

            // go down first sub if any
            let mut subs = {
                let mut subs: Vec<idx::Pack> = self[current].sub().iter().cloned().collect();
                subs.sort();
                subs.into_iter()
            };
            if let Some(next) = subs.next() {
                stack.push(subs);
                current = next;
                continue 'go_down;
            }

            // only reachable if there are no subpackages
            'go_up: loop {
                if let Some(mut subs) = stack.pop() {
                    if let Some(next) = subs.next() {
                        // new package to do gown into
                        stack.push(subs);
                        current = next;
                        continue 'go_down;
                    } else {
                        // no subs here, but stack might contain more
                        continue 'go_up;
                    }
                } else {
                    // stack is empty, we done
                    break 'go_down;
                }
            }
        }

        res.shrink_to_fit();
        res
    }

    pub fn top_pack(&self) -> idx::Pack {
        self.top_pack
    }
    pub fn builtin_pack(&self) -> idx::Pack {
        self.builtin_pack
    }
    /// Package of Ecore's own classes, see [`ecore`].
    ///
    /// `None` unless the metamodel refers to one of them.
    pub fn ecore_pack(&self) -> Option<idx::Pack> {
        self.ecore_pack
    }

    pub fn pack_idx<K>(&self, path: &Path, name: &K) -> Res<idx::Pack>
    where
        String: Borrow<K>,
        K: std::hash::Hash + Eq + Display + ?Sized,
    {
        self.name_to_pack
            .unwrap_at(path, |p| p.display(&self.packs))
            .and_then(|map| {
                map.get(name)
                    .ok_or_else(|| error!(@unknown("package") name.to_string()))
                    .cloned()
            })
    }

    pub fn pack_idx_or_forward_ref(
        &mut self,
        path: &Path,
        name: impl AsRef<str>,
    ) -> Res<idx::Pack> {
        let name = name.as_ref();
        if let Ok(idx) = self.pack_idx(path, name) {
            Ok(idx)
        } else {
            let idx = self.add_pack(path.clone(), name)?;
            let _is_new = self.forward_ref_packs.insert(idx);
            debug_assert!(_is_new);
            Ok(idx)
        }
    }

    /// Map from [`builtin::Typ`] to [`idx::Class`].
    pub fn builtins(&self) -> &HashMap<builtin::Typ, idx::Class> {
        &self.builtin_map
    }
    /// Retrieves the [`idx::Class`] of a builtin class.
    pub fn get_builtin_idx(&self, typ: impl AsRef<builtin::Typ>) -> Res<idx::Class> {
        let typ = typ.as_ref();
        self.builtin_map.get(typ).cloned().ok_or_else(|| {
            error!(
                "[fatal] builtin type `{}` has not been properly registered",
                typ,
            )
        })
    }

    /// Map from [`ecore::Typ`] to [`idx::Class`], empty unless the metamodel refers to one of
    /// Ecore's own classes.
    pub fn ecore_classes(&self) -> &HashMap<ecore::Typ, idx::Class> {
        &self.ecore_map
    }
    /// True if `idx` is one of Ecore's own classes, see [`ecore`].
    pub fn is_ecore_class(&self, idx: idx::Class) -> bool {
        self.ecore_pack == Some(self[idx].path.last())
    }
    /// Retrieves the [`idx::Class`] of one of Ecore's own classes, adding the Ecore package to the
    /// context the first time.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # ecore_rs::prelude! {}
    /// let mut ctx = Ctx::with_capacity(0, 0);
    /// assert_eq!(ctx.ecore_pack(), None);
    /// let idx = ctx.ecore_class_idx(ecore::Typ::EModelElement).expect("adding Ecore failed");
    /// assert_eq!(ctx[idx].name(), "EModelElement");
    /// assert_eq!(Some(ctx[idx].path.last()), ctx.ecore_pack());
    /// assert_eq!(ctx.ecore_class_idx(ecore::Typ::EModelElement).unwrap(), idx);
    /// ```
    pub fn ecore_class_idx(&mut self, typ: ecore::Typ) -> Res<idx::Class> {
        if self.ecore_pack.is_none() {
            let (pack, classes) = ecore::populate(self)?;
            self.ecore_pack = Some(pack);
            self.ecore_map = ecore::Typ::ALL.into_iter().zip(classes).collect();
        }
        self.ecore_map.get(&typ).cloned().ok_or_else(|| {
            error!(
                "[fatal] Ecore class `{}` has not been properly registered",
                typ
            )
        })
    }

    /// Parses a builtin type URL appearing after a `ecore:EDataType` in an `eType` attribute.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # ecore_rs::prelude! {}
    /// let ctx = Ctx::with_capacity(0, 0);
    /// let idx =
    ///     ctx.parse_builtin_etype_url("http://www.eclipse.org/emf/2002/Ecore#//EString")
    ///     .expect("url parsing failed");
    /// assert_eq!(ctx[idx].name(), "EString");
    /// assert_eq!(ctx[idx].path.last(), ctx.builtin_pack());
    /// ```
    pub fn parse_builtin_etype_url(&self, url: impl AsRef<str>) -> Res<idx::Class> {
        let typ = builtin::Typ::parse_etype_url(url)?;
        self.get_builtin_idx(typ)
    }
    /// Recognizes a builtin `ecore` datatype from its `eType` description.
    ///
    /// Returns `None` if `s` does not start with `ecore:EDataType`, otherwise returns the type if it is
    /// recognized from the URL part and an error if not.
    ///
    /// If you know you're parsing a builtin type, like if you already parsed the `ecore:EDataType`
    /// header of the `eType`, use [`Self::parse_builtin_etype_url`] instead.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # ecore_rs::prelude! {}
    /// let ctx = Ctx::with_capacity(0, 0);
    /// let idx =
    ///     ctx.try_parse_builtin_etype("ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString")
    ///     .expect("url parsing failed")
    ///     .expect("failed to parse `EString` builtin type");
    /// assert_eq!(ctx[idx].name(), "EString");
    /// assert_eq!(ctx[idx].path.last(), ctx.builtin_pack());
    ///
    /// let idx =
    ///     ctx.try_parse_builtin_etype("#//NotBuiltin")
    ///     .expect("non-builtin class parsing failed");
    /// assert_eq!(idx, None);
    ///
    /// let res =
    ///     ctx.try_parse_builtin_etype("ecore:EDataType http://www.some.non/sense");
    /// assert!(res.is_err());
    /// ```
    pub fn try_parse_builtin_etype(&self, etype: impl AsRef<str>) -> Res<Option<idx::Class>> {
        let typ_opt = builtin::Typ::try_parse_etype(etype)?;
        if let Some(typ) = typ_opt {
            self.get_builtin_idx(typ).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn class_names_at(&self, path: &Path) -> Res<&HashMap<String, idx::Class>> {
        self.name_to_class
            .unwrap_at(path, |p| p.display(&self.packs))
    }
    pub fn class_names_at_mut(&mut self, path: &Path) -> Res<&mut HashMap<String, idx::Class>> {
        self.name_to_class
            .unwrap_at_mut(path, |p| p.display(&self.packs))
    }
    pub fn class_names_at_mut_or_new(&mut self, path: Path) -> &mut HashMap<String, idx::Class> {
        self.name_to_class.at_mut_or_new(path)
    }

    pub fn pack_names_at(&self, path: &Path) -> Res<&HashMap<String, idx::Pack>> {
        self.name_to_pack
            .unwrap_at(path, |p| p.display(&self.packs))
    }
    pub fn pack_names_at_mut(&mut self, path: &Path) -> Res<&mut HashMap<String, idx::Pack>> {
        self.name_to_pack
            .unwrap_at_mut(path, |p| p.display(&self.packs))
    }
    pub fn pack_names_at_mut_or_new(&mut self, path: Path) -> &mut HashMap<String, idx::Pack> {
        self.name_to_pack.at_mut_or_new(path)
    }

    pub fn get_class_idx_in<K>(&self, path: &Path, name: &K) -> Res<idx::Class>
    where
        String: Borrow<K>,
        K: std::hash::Hash + Eq + Display + ?Sized,
    {
        self.class_names_at(path)
            .context(|| {
                format!(
                    "failed to resolve class `{}{}`",
                    path.display_sep(&self.packs),
                    name
                )
            })?
            .get(name)
            .cloned()
            .ok_or_else(|| error!(@unknown("class") name.to_string()))
    }

    pub fn get_class_idx_or_forward_ref_in(
        &mut self,
        path: &Path,
        name: impl AsRef<str>,
    ) -> Res<idx::Class> {
        let name = name.as_ref();
        // do we already know this class at this path?
        if let Ok(idx) = self.get_class_idx_in(path, name) {
            return Ok(idx);
        }

        self.add_forward_ref_class(path.clone(), name)
    }

    pub fn add_pack(&mut self, path: Path, name: impl Into<String>) -> Res<idx::Pack> {
        let name = name.into();

        // do we know this package?
        if let Ok(p_idx) = self.pack_idx(&path, &name) {
            // either a definition for a previously forward-ref-ed package or a redef
            if self.forward_ref_packs.contains(&p_idx) {
                self.forward_ref_packs.remove(&p_idx);
                // the package should be empty at this point, since we're currently defining it
                if !self[p_idx].is_empty() {
                    bail!("failed to define previously forward-referenced package: not empty")
                }
                return Ok(p_idx);
            } else {
                bail!(@redef("package") name)
            }
        }

        let p_idx = self.packs.next_index();

        // retrieve local context, insert new if needed
        let local_pack_names = self.pack_names_at_mut_or_new(path.clone());
        // register pack index
        let _prev = local_pack_names.insert(name.clone(), p_idx);

        if let Some(p_idx) = _prev {
            let name = self[p_idx].name();
            let note = format!("existing package `{name}` has index `#{p_idx}`");
            return Err(error!(@redef("package") name).with_context(note));
        }

        let sup_idx = path.last();
        // register as sub-package in super
        let _is_new = self[sup_idx].add_sub(p_idx);
        // actually create sub-package
        let real_p_idx = self
            .packs
            .push_idx(|idx| Pack::new(idx, &name, Some(sup_idx)));
        debug_assert_eq!(p_idx, real_p_idx); // #defense
        Ok(real_p_idx)
    }

    /// Adds a package under `sup` without registering its name, so that it cannot clash with the
    /// metamodel's packages.
    pub(crate) fn raw_add_pack(&mut self, name: impl Into<String>, sup: idx::Pack) -> idx::Pack {
        let name = name.into();
        let p_idx = self.packs.push_idx(|idx| Pack::new(idx, name, Some(sup)));
        self[sup].add_sub(p_idx);
        p_idx
    }

    pub(crate) fn raw_add_class(
        &mut self,
        build_class: impl FnOnce(idx::Class) -> Class,
    ) -> Res<idx::Class> {
        // register class
        let c_idx = self.classes.push_idx(build_class);
        let parent = self[c_idx].path.last();
        // update relevant package
        let _is_new = self[parent].classes_insert(c_idx);
        if !_is_new {
            bail!(
                "class `{}` is already in package `{}`",
                self[c_idx].name(),
                self[parent].name()
            );
        }
        Ok(c_idx)
    }

    /// Adds a class to the context.
    ///
    /// If `path`/`name` is already registered as forward-referenced, the dummy class is replaced by
    /// the actual definition and removed from `self.forward_refs`.
    pub fn add_class(
        &mut self,
        path: Path,
        typ: impl Into<String>,
        name: impl Into<String>,
        inst_name: Option<impl Into<String>>,
        is_abstract: Option<bool>,
        is_interface: Option<bool>,
    ) -> Res<idx::Class> {
        let name = name.into();

        macro_rules! build_class {
            ($idx:expr) => {
                Class::new($idx, path, typ, name, inst_name, is_abstract, is_interface)
            };
        }

        // do we know have a `name` at this `path`?
        if let Some(idx) = self
            .name_to_class
            .get(&path)
            .and_then(|name_map| name_map.get(&name))
            .cloned()
        {
            // either a class redef', or it's a forward ref and we need to handle it
            if self.forward_ref_classes.contains(&idx) {
                self.forward_ref_classes.remove(&idx);
                let class = build_class!(idx);
                let dummy = std::mem::replace(&mut self[idx], class);

                for sup in dummy.sup().iter().copied() {
                    let _ = self[idx].add_sup(sup);
                }
                for sub in dummy.sub().iter().copied() {
                    let _ = self[idx].add_sub(sub);
                }
                // don't need to update the class' parent, the forward ref already did that
                return Ok(idx);
            } else {
                return Err(error!(@redef("class") name)
                    .with_context(format!("in package `{}`", path.display(&self.packs))));
            }
        }

        // actually new `path`/`name` class
        let c_idx = self.classes.next_index();

        // retrieve local context, insert new if needed
        let local_class_names = self.class_names_at_mut_or_new(path.clone());
        // register class index
        let _prev = local_class_names.insert(name.clone(), c_idx);

        // check for name clashes
        if let Some(c_idx) = _prev {
            let name = self[c_idx].name();
            let note = format!("existing class `{name}` has index `#{c_idx}`");
            return Err(error!(@redef("class") name).with_context(note));
        }

        let real_c_idx = self.raw_add_class(|c_idx| {
            Class::new(
                c_idx,
                path.clone(),
                typ,
                name,
                inst_name,
                is_abstract,
                is_interface,
            )
        })?;
        debug_assert_eq!(c_idx, real_c_idx); // #defense

        Ok(real_c_idx)
    }

    /// Adds a dummy class and registers it as a forward reference.
    ///
    /// Also registers the class as a member of its parent package.
    pub fn add_forward_ref_class(
        &mut self,
        path: Path,
        name: impl Into<String>,
    ) -> Res<idx::Class> {
        let msg = "[forward referenced class placeholder]";
        let c_idx = self.add_class(path, msg, name, None as Option<String>, None, None)?;
        let _is_new = self.forward_ref_classes.insert(c_idx);
        debug_assert!(_is_new);
        Ok(c_idx)
    }

    /// Registers `sup` as a super-class of `sub`.
    pub fn add_sup_class(&mut self, sup: idx::Class, sub: idx::Class) {
        let _is_new = self[sup].add_sub(sub);
        let _is_new = self[sub].add_sup(sup);
    }

    /// Packs appear in the order they were added in.
    pub fn packs(&self) -> &idx::PackMap<Pack> {
        &self.packs
    }
    pub fn pack_indices<'me>(&'me self) -> impl Iterator<Item = idx::Pack> + 'me {
        self.packs.indices()
    }

    /// Classes appear in the order they were added in.
    pub fn classes(&self) -> &[Class] {
        &self.classes
    }
    pub fn class(&self, idx: idx::Class) -> &Class {
        &self.classes[idx]
    }
    pub fn class_indices<'me>(&'me self) -> impl Iterator<Item = idx::Class> + 'me {
        self.classes.indices()
    }
    pub fn abstract_classes(&self) -> impl Iterator<Item = &Class> {
        self.classes.iter().filter(|c| c.is_abstract())
    }
    pub fn concrete_classes(&self) -> impl Iterator<Item = &Class> {
        self.classes.iter().filter(|c| c.is_concrete())
    }

    /// Classes appearing in a given package.
    pub fn classes_in<'a>(&'a self, path: &'a Path) -> impl Iterator<Item = &'a Class> + 'a {
        self.classes.iter().filter(move |c| &c.path == path)
    }
    pub fn abstract_classes_in<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Iterator<Item = &'a Class> + 'a {
        self.classes_in(path).filter(|c| c.is_abstract())
    }
    pub fn concrete_classes_in<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Iterator<Item = &'a Class> + 'a {
        self.classes_in(path).filter(|c| c.is_concrete())
    }

    pub fn enter_pack(&mut self, path: Path) -> Res<PathCtx<'_>> {
        Ok(PathCtx { path, ctx: self })
    }

    pub fn enter_root_pack(&'_ mut self) -> Res<PathCtx<'_>> {
        self.enter_pack(Path::new(self.top_pack))
    }

    /// Finalizes the context.
    ///
    /// - checks there are no forward-referenced classes left;
    /// - checks there are no forward-referenced packages left;
    pub fn finalize(&mut self) -> Res<()> {
        let mut errors = false;

        if !self.forward_ref_classes.is_empty() {
            errors = true;
            log::error!("some forward-referenced classes have not been defined:");
            for idx in self.forward_ref_classes.iter().cloned() {
                log::error!("- `{}`", self[idx].display(self));
            }
        }

        if !self.forward_ref_packs.is_empty() {
            errors = true;
            log::error!("some forward-referenced classes have not been defined:");
            for idx in self.forward_ref_packs.iter().cloned() {
                log::error!("- `{}`", self[idx].path(self).display(self.packs()));
            }
        }

        if errors {
            bail!("failed to finalize parsing context")
        } else {
            Ok(())
        }
    }
}

pub struct PathCtx<'a> {
    path: Path,
    ctx: &'a mut Ctx,
}

impl<'a> std::ops::Index<idx::Class> for PathCtx<'a> {
    type Output = Class;
    fn index(&self, idx: idx::Class) -> &Self::Output {
        &self.ctx[idx]
    }
}
impl<'a> std::ops::IndexMut<idx::Class> for PathCtx<'a> {
    fn index_mut(&mut self, idx: idx::Class) -> &mut Self::Output {
        &mut self.ctx[idx]
    }
}

// impl<'a> std::ops::Index<idx::Pack> for PathCtx<'a> {
//     type Output = Pack;
//     fn index(&self, idx: idx::Pack) -> &Self::Output {
//         &self.ctx[idx]
//     }
// }
// impl<'a> std::ops::IndexMut<idx::Pack> for PathCtx<'a> {
//     fn index_mut(&mut self, idx: idx::Pack) -> &mut Self::Output {
//         &mut self.ctx[idx]
//     }
// }

impl<'a> PathCtx<'a> {
    pub fn current(&self) -> &Pack {
        &self.ctx[self.path.last()]
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn display_path(&self) -> String {
        self.path.display(&self.ctx.packs)
    }
    pub fn display_path_sep(&self) -> String {
        self.path.display_sep(&self.ctx.packs)
    }
    pub fn current_pack(&self) -> &Pack {
        &self.ctx[self.path.last()]
    }
    pub fn current_pack_mut(&mut self) -> &mut Pack {
        let idx = self.path.last();
        &mut self.ctx[idx]
    }

    pub fn ctx(&self) -> &Ctx {
        self.ctx
    }
    pub fn ctx_mut(&mut self) -> &mut Ctx {
        self.ctx
    }

    pub fn resolve_etype(&mut self, s: impl AsRef<str>) -> Res<idx::Class> {
        let etype = repr::Path::resolve_etype(self, s.as_ref())?;
        let class = &self.ctx[etype];

        if class.path.last() != self.ctx.builtin_pack()
            && !self.ctx.is_ecore_class(etype)
            && class.path != self.path
        {
            bail!(
                "inter-package links are not supported: current package `{}` cannot reference type `{}` from package `{}`",
                self.path.display(self.ctx.packs()),
                class.name(),
                class.path.display(self.ctx.packs()),
            );
        }

        Ok(etype)
    }

    pub fn add_sub_pack(&mut self, name: impl Into<String>) -> Res<idx::Pack> {
        self.ctx.add_pack(self.path.clone(), name)
    }
    /// Same as [`Self::add_sub_pack`] but makes the context enter the new sub-package.
    pub fn add_and_enter_sub_pack_mut(&mut self, name: impl Into<String>) -> Res<idx::Pack> {
        let p_idx = self.ctx.add_pack(self.path.clone(), name)?;
        self.path.push(p_idx);
        Ok(p_idx)
    }
    /// Same as [`Self::add_sub_pack`] but makes the context enter the new sub-package.
    pub fn add_and_enter_sub_pack(mut self, name: impl Into<String>) -> Res<(idx::Pack, Self)> {
        let p_idx = self.add_and_enter_sub_pack_mut(name)?;
        Ok((p_idx, self))
    }

    pub fn add_class(
        &mut self,
        typ: impl Into<String>,
        name: impl Into<String>,
        inst_name: Option<impl Into<String>>,
        is_abstract: Option<bool>,
        is_interface: Option<bool>,
    ) -> Res<idx::Class> {
        self.ctx.add_class(
            self.path.clone(),
            typ,
            name,
            inst_name,
            is_abstract,
            is_interface,
        )
    }

    pub fn class_idx<K>(&self, name: &K) -> Res<idx::Class>
    where
        String: Borrow<K>,
        K: std::hash::Hash + Eq + Display + ?Sized,
    {
        self.ctx.get_class_idx_in(&self.path, name)
    }
    pub fn forward_ref_or_class_idx<K>(&mut self, name: impl AsRef<str>) -> Res<idx::Class> {
        self.ctx.get_class_idx_or_forward_ref_in(&self.path, name)
    }

    pub fn class_idx_in<K>(&self, path: &Path, name: &K) -> Res<idx::Class>
    where
        String: Borrow<K>,
        K: std::hash::Hash + Eq + Display + ?Sized,
    {
        self.ctx.get_class_idx_in(path, name)
    }
    pub fn forward_ref_or_class_idx_in(
        &mut self,
        path: &Path,
        name: impl AsRef<str>,
    ) -> Res<idx::Class> {
        self.ctx.get_class_idx_or_forward_ref_in(path, name)
    }

    /// Classes appear in the order they were added in.
    pub fn classes(&self) -> impl Iterator<Item = &Class> {
        self.ctx.classes_in(&self.path)
    }
    pub fn abstract_classes(&self) -> impl Iterator<Item = &Class> {
        self.ctx.abstract_classes_in(&self.path)
    }
    pub fn concrete_classes(&self) -> impl Iterator<Item = &Class> {
        self.ctx.concrete_classes_in(&self.path)
    }

    /// Adds an annotation to the current package.
    pub fn add_annotation(&mut self, annot: repr::Annot) {
        self.ctx[self.path.last()].add_annotation(annot)
    }

    /// Enters a sub-package.
    ///
    /// Error if `sub` is not a sub-package of the current package.
    pub fn enter_sub_pack_mut(&mut self, sub: idx::Pack) -> Res<()> {
        if !self.current_pack().has_sub(sub) {
            let path = self.path().display(&self.ctx.packs);
            let sub = self.ctx[sub].name();
            bail!("package `{path}` has no sub-package called `{sub}`")
        }
        self.path.push(sub);
        Ok(())
    }

    /// Enters a sub-package by consuming itself.
    pub fn enter_sub_pack(mut self, sub: idx::Pack) -> Res<Self> {
        self.enter_sub_pack_mut(sub)?;
        Ok(self)
    }

    /// Enters the super-package of the current package.
    ///
    /// Error if at top-level.
    pub fn enter_sup_pack(&mut self) -> Res<idx::Pack> {
        if let Some(popped) = self.path.pop() {
            Ok(popped)
        } else {
            bail!("trying to enter super package of top-level package")
        }
    }

    /// Enters a different package specified with an absolute path.
    pub fn change_pack(&mut self, path: Path) {
        self.path = path;
    }

    /// Registers `sup` as a super-class of `sub`.
    pub fn add_sup_class(&mut self, sup: idx::Class, sub: idx::Class) {
        self.ctx.add_sup_class(sup, sub)
    }

    pub fn enter_class<'me>(
        &'me mut self,
        typ: impl Into<String>,
        name: impl Into<String>,
        inst_name: Option<impl Into<String>>,
        is_abstract: Option<bool>,
        is_interface: Option<bool>,
    ) -> Res<ClassCtx<'a, 'me>> {
        let c_idx = self.ctx.add_class(
            self.path.clone(),
            typ,
            name,
            inst_name,
            is_abstract,
            is_interface,
        )?;
        Ok(ClassCtx { ctx: self, c_idx })
    }
}

pub struct ClassCtx<'a, 'b> {
    ctx: &'b mut PathCtx<'a>,
    c_idx: idx::Class,
}

impl<'a, 'b> std::ops::Index<idx::Class> for ClassCtx<'a, 'b> {
    type Output = Class;
    fn index(&self, idx: idx::Class) -> &Self::Output {
        &self.ctx[idx]
    }
}
impl<'a, 'b> std::ops::IndexMut<idx::Class> for ClassCtx<'a, 'b> {
    fn index_mut(&mut self, idx: idx::Class) -> &mut Self::Output {
        &mut self.ctx[idx]
    }
}

impl<'a, 'b> ClassCtx<'a, 'b> {
    pub fn current(&self) -> &repr::Class {
        &self.ctx[self.c_idx]
    }

    pub fn path_ctx(&self) -> &PathCtx<'a> {
        self.ctx
    }
    pub fn path_ctx_mut(&mut self) -> &mut PathCtx<'a> {
        self.ctx
    }

    pub fn ctx(&self) -> &Ctx {
        self.path_ctx().ctx()
    }
    pub fn ctx_mut(&mut self) -> &mut Ctx {
        self.ctx.ctx
    }

    pub fn resolve_etype(&mut self, s: impl AsRef<str>) -> Res<idx::Class> {
        repr::Path::resolve_etype(self.ctx, s)
    }

    pub fn add_annotation(&mut self, annot: repr::Annot) {
        self.ctx[self.c_idx].add_annotation(annot)
    }
    pub fn set_instance_class_name(&mut self, instance_class_name: impl Into<String>) {
        self.ctx[self.c_idx].set_instance_class_name(instance_class_name)
    }
    pub fn add_sup_class(&mut self, sup: idx::Class) {
        self.ctx.add_sup_class(sup, self.c_idx)
    }
    pub fn add_literal(&mut self, lit: repr::ELit) {
        self.ctx[self.c_idx].add_literal(lit)
    }
    pub fn add_operation(&mut self, op: repr::Operation) {
        self.ctx[self.c_idx].add_operation(op)
    }
    pub fn add_attribute(&mut self) {
        // println!("registering attribute")
    }
    pub fn add_structural(&mut self, s: repr::Structural) {
        self.ctx[self.c_idx].add_structural(s)
    }
    pub fn finalize(self) {
        // // not shrink-to-fit-ing since some stuff is postponed
        // self.ctx[self.c_idx].shrink_to_fit()
    }
}

#[cfg(test)]
mod tests {
    use super::{Class, Ctx};

    /// A package with one class and one attribute, with no XML declaration before it and no
    /// newline after it.
    const BARE_PACKAGE: &str = r##"<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Node">
        <eStructuralFeatures xsi:type="ecore:EAttribute"
            name="label"
            eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    </eClassifiers>
</ecore:EPackage>"##;

    const XML_DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";

    fn assert_parses_bare_package(ecore: &str) {
        let ctx = match Ctx::parse(ecore) {
            Ok(ctx) => ctx,
            Err(e) => panic!("refused: {e}"),
        };
        let node = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Node")
            .expect("Node class should exist");
        assert!(node
            .structural()
            .iter()
            .any(|feature| feature.name == "label"));
    }

    /// `top` skips everything up to the first `>` without looking at it, `at_path` accepts the
    /// end of input, and nothing asks for a package, so input with no `EPackage` in it parses as
    /// an empty model.
    #[test]
    fn refuses_input_with_no_epackage_root() {
        for input in [
            "",
            "hello world",
            "<not ecore",
            XML_DECLARATION,
            "<foo>bar</foo>\n",
        ] {
            match Ctx::parse(input) {
                Ok(_) => panic!("input {input:?} parsed as an empty model"),
                Err(e) => assert!(
                    e.to_string().contains("`ecore:EPackage`"),
                    "input {input:?} refused without naming the missing root: {e}"
                ),
            }
        }
    }

    /// `top` takes the first tag to be the XML declaration and throws it away, so a file that
    /// starts with its `EPackage` loses that tag and is refused at its first classifier.
    #[test]
    fn parses_a_file_with_no_xml_declaration() {
        assert_parses_bare_package(&format!("{BARE_PACKAGE}\n"));
    }

    /// `try_raw_tag` only matches a tag strictly shorter than the rest of the input, so a file
    /// whose last bytes are `</ecore:EPackage>` is refused.
    #[test]
    fn parses_a_file_with_no_trailing_newline() {
        assert_parses_bare_package(&format!("{XML_DECLARATION}{BARE_PACKAGE}"));
    }

    /// `top` skips to the first `<` one byte per character, so the three bytes of a byte order
    /// mark leave the cursor inside it.
    #[test]
    fn parses_a_file_starting_with_a_byte_order_mark() {
        assert_parses_bare_package(&format!("\u{feff}{XML_DECLARATION}{BARE_PACKAGE}\n"));
    }

    #[test]
    fn parses_structural_feature_default_values() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Node">
        <eStructuralFeatures xsi:type="ecore:EAttribute"
            name="label"
            defaultValue="Untitled"
            defaultValueLiteral="Untitled"
            eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    </eClassifiers>
</ecore:EPackage>
"##;

        let ctx = Ctx::parse(ecore).expect("ecore should parse");
        let node = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Node")
            .expect("Node class should exist");
        let label = node
            .structural()
            .iter()
            .find(|feature| feature.name == "label")
            .expect("label feature should exist");

        assert_eq!(label.default_value.as_deref(), Some("Untitled"));
        assert_eq!(label.default_value_literal.as_deref(), Some("Untitled"));
    }

    #[test]
    fn parses_reference_resolve_proxies() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Node">
        <eStructuralFeatures xsi:type="ecore:EReference"
            name="target"
            resolveProxies="false"
            eType="#//Target"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Target"/>
</ecore:EPackage>
"##;

        let ctx = Ctx::parse(ecore).expect("ecore should parse");
        let node = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Node")
            .expect("Node class should exist");
        let target = node
            .structural()
            .iter()
            .find(|feature| feature.name == "target")
            .expect("target reference should exist");

        assert_eq!(target.resolve_proxies, Some(false));
    }

    /// Runs `parse` and returns the warnings it logged on the current thread.
    ///
    /// The parser reports what it drops only through `log::warn!`, so this installs, once for the
    /// test binary, a logger that keeps each thread's warnings apart.
    fn with_warnings<T>(parse: impl FnOnce() -> T) -> (T, Vec<String>) {
        use std::{cell::RefCell, sync::Once};

        thread_local! {
            static WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
        }

        struct Capture;
        impl log::Log for Capture {
            fn enabled(&self, metadata: &log::Metadata) -> bool {
                metadata.level() == log::Level::Warn
            }
            fn log(&self, record: &log::Record) {
                if self.enabled(record.metadata()) {
                    WARNINGS.with(|warnings| warnings.borrow_mut().push(record.args().to_string()));
                }
            }
            fn flush(&self) {}
        }

        static INSTALL: Once = Once::new();
        INSTALL.call_once(|| {
            log::set_logger(&Capture).expect("no other logger should be installed in unit tests");
            log::set_max_level(log::LevelFilter::Warn);
        });

        WARNINGS.with(|warnings| warnings.borrow_mut().clear());
        let res = parse();
        (res, WARNINGS.with(RefCell::take))
    }

    /// `class_structural` reads `unsettable` and then neither stores it nor warns about it, so the
    /// flag is dropped without a trace.
    #[test]
    fn warns_when_dropping_unsettable() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Feature">
        <eStructuralFeatures xsi:type="ecore:EAttribute"
            name="direction"
            unsettable="true"
            eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
        <eStructuralFeatures xsi:type="ecore:EAttribute"
            name="label"
            unsettable="false"
            eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    </eClassifiers>
</ecore:EPackage>
"##;

        let (parsed, warnings) = with_warnings(|| Ctx::parse(ecore));
        parsed.expect("ecore should parse");

        let unsettable: Vec<_> = warnings
            .iter()
            .filter(|warning| warning.contains("unsettable"))
            .collect();
        assert_eq!(
            unsettable.len(),
            1,
            "expected one warning about `unsettable`, got {warnings:?}"
        );
        assert!(
            unsettable[0].contains("`Feature`") && unsettable[0].contains("`direction`"),
            "the warning does not name `Feature.direction`: {}",
            unsettable[0]
        );
    }

    /// The warning for a dropped `eOpposite` names neither the feature nor its opposite, so a
    /// metamodel with many of them gives no way to tell which were lost.
    #[test]
    fn names_the_feature_when_dropping_eopposite() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Teacher">
        <eStructuralFeatures xsi:type="ecore:EReference"
            name="advises"
            upperBound="-1"
            eType="#//Student"
            eOpposite="#//Student/advisor"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Student">
        <eStructuralFeatures xsi:type="ecore:EReference"
            name="advisor"
            eType="#//Teacher"
            eOpposite="#//Teacher/advises"/>
    </eClassifiers>
</ecore:EPackage>
"##;

        let (parsed, warnings) = with_warnings(|| Ctx::parse(ecore));
        parsed.expect("ecore should parse");

        let opposite: Vec<_> = warnings
            .iter()
            .filter(|warning| warning.contains("eOpposite"))
            .collect();
        assert_eq!(
            opposite.len(),
            2,
            "expected one warning about `eOpposite` per reference, got {warnings:?}"
        );
        for (class, feature, path) in [
            ("Teacher", "advises", "#//Student/advisor"),
            ("Student", "advisor", "#//Teacher/advises"),
        ] {
            assert!(
                opposite.iter().any(|warning| {
                    warning.contains(&format!("`{class}`"))
                        && warning.contains(&format!("`{feature}`"))
                        && warning.contains(path)
                }),
                "no warning names `{class}.{feature}` and `{path}`: {opposite:?}"
            );
        }
    }

    #[test]
    fn parses_operation_annotations() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Node">
        <eOperations name="getSignals" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString">
            <eAnnotations source="http://www.eclipse.org/emf/2002/GenModel">
                <details key="body" value="ArrayList&lt;SignalType> signals = new ArrayList&lt;SignalType>();&#xA;return signals;"/>
            </eAnnotations>
        </eOperations>
    </eClassifiers>
</ecore:EPackage>
"##;

        let ctx = Ctx::parse(ecore).expect("ecore should parse");
        let node = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Node")
            .expect("Node class should exist");
        let operation = node
            .operations()
            .iter()
            .find(|operation| operation.name() == "getSignals")
            .expect("operation should exist");
        let annotation = operation
            .annotations()
            .iter()
            .find(|annotation| annotation.source() == "http://www.eclipse.org/emf/2002/GenModel")
            .expect("GenModel annotation should exist");

        assert!(annotation
            .details()
            .get("body")
            .is_some_and(|body| body.contains("ArrayList&lt;SignalType>")));
    }

    /// `annotation` reads a `source` attribute and then requires `>`, so an annotation closed
    /// with `/>` is refused, although it is how Ecore writes an annotation with no details.
    #[test]
    fn parses_self_closing_annotations() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eAnnotations source="on-package"/>
    <eClassifiers xsi:type="ecore:EClass" name="Node">
        <eAnnotations source="on-class" />
        <eStructuralFeatures xsi:type="ecore:EAttribute"
            name="label"
            eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString">
            <eAnnotations source="on-feature"/>
        </eStructuralFeatures>
        <eStructuralFeatures xsi:type="ecore:EAttribute"
            name="count"
            eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
    </eClassifiers>
</ecore:EPackage>
"##;

        let ctx = Ctx::parse(ecore).expect("ecore should parse");
        let sources = |annotations: &[crate::repr::Annot]| {
            annotations
                .iter()
                .map(|annotation| annotation.source().to_string())
                .collect::<Vec<_>>()
        };

        let test = ctx
            .packs()
            .iter()
            .find(|pack| pack.name() == "test")
            .expect("test package should exist");
        assert_eq!(sources(test.annotations()), ["on-package"]);

        let node = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Node")
            .expect("Node class should exist");
        assert_eq!(sources(node.annotations()), ["on-class"]);

        let label = node
            .structural()
            .iter()
            .find(|feature| feature.name == "label")
            .expect("label feature should exist");
        assert_eq!(sources(label.annotations()), ["on-feature"]);
        assert!(label.annotations()[0].details().is_empty());
        assert!(label.annotations()[0].references().is_empty());

        assert!(node
            .structural()
            .iter()
            .any(|feature| feature.name == "count"));
    }

    /// `annotation` reads no attribute but `source`, so the `references` attribute that holds
    /// `EAnnotation.references` is refused, in whichever order the two attributes come and
    /// whether or not the annotation has details.
    #[test]
    fn parses_annotation_references() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Step">
        <eStructuralFeatures xsi:type="ecore:EReference" name="parameter" upperBound="-1" eType="#//Step"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Usage">
        <eStructuralFeatures xsi:type="ecore:EReference" name="nested" upperBound="-1" eType="#//Usage"/>
        <eStructuralFeatures xsi:type="ecore:EReference" name="documented" eType="#//Usage">
            <eAnnotations source="subsets" references="#//Step/parameter
                #//Usage/nested">
                <details key="note" value="kept"/>
            </eAnnotations>
        </eStructuralFeatures>
        <eStructuralFeatures xsi:type="ecore:EReference" name="subsetting" upperBound="-1" eType="#//Usage">
            <eAnnotations source="subsets" references="#//Usage/nested #//Step/parameter"/>
        </eStructuralFeatures>
        <eStructuralFeatures xsi:type="ecore:EReference" name="redefining" eType="#//Usage">
            <eAnnotations references="#//Usage/nested" source="redefines"/>
        </eStructuralFeatures>
    </eClassifiers>
</ecore:EPackage>
"##;

        let ctx = Ctx::parse(ecore).expect("ecore should parse");
        let usage = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Usage")
            .expect("Usage class should exist");
        let annotation = |feature: &str| {
            let feature = usage
                .structural()
                .iter()
                .find(|candidate| candidate.name == feature)
                .expect("feature should exist");
            assert_eq!(feature.annotations().len(), 1);
            feature.annotations()[0].clone()
        };

        let subsetting = annotation("subsetting");
        assert_eq!(subsetting.source(), "subsets");
        assert_eq!(
            subsetting.references(),
            ["#//Usage/nested", "#//Step/parameter"]
        );
        assert!(subsetting.details().is_empty());

        let redefining = annotation("redefining");
        assert_eq!(redefining.source(), "redefines");
        assert_eq!(redefining.references(), ["#//Usage/nested"]);
        assert!(redefining.details().is_empty());

        let documented = annotation("documented");
        assert_eq!(documented.source(), "subsets");
        assert_eq!(
            documented.references(),
            ["#//Step/parameter", "#//Usage/nested"]
        );
        assert_eq!(
            documented.details().get("note").map(String::as_str),
            Some("kept")
        );
    }

    #[test]
    fn parses_operation_and_parameter_typed_element_attributes() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Node">
        <eOperations name="collect"
            ordered="false"
            unique="false"
            lowerBound="0"
            upperBound="-1"
            eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString">
            <eParameters name="count"
                ordered="true"
                unique="false"
                lowerBound="2"
                upperBound="4"
                eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
        </eOperations>
        <eOperations name="notify">
            <eParameters name="value"/>
        </eOperations>
    </eClassifiers>
</ecore:EPackage>
"##;

        let ctx = Ctx::parse(ecore).expect("ecore should parse");
        let node = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Node")
            .expect("Node class should exist");

        let collect = node
            .operations()
            .iter()
            .find(|operation| operation.name() == "collect")
            .expect("collect operation should exist");
        assert_eq!(collect.bounds().lbound, 0);
        assert_eq!(collect.bounds().ubound, None);
        assert_eq!(collect.ordered(), Some(false));
        assert_eq!(collect.unique(), Some(false));
        assert!(!collect.is_ordered());
        assert!(!collect.is_unique());
        assert_eq!(
            ctx.class(collect.typ().expect("collect should have a type"))
                .name(),
            "EString"
        );

        let count = collect
            .parameters()
            .iter()
            .find(|parameter| parameter.name() == "count")
            .expect("count parameter should exist");
        assert_eq!(count.bounds().lbound, 2);
        assert_eq!(count.bounds().ubound, Some(4));
        assert_eq!(count.ordered(), Some(true));
        assert_eq!(count.unique(), Some(false));
        assert!(count.is_ordered());
        assert!(!count.is_unique());
        assert_eq!(
            ctx.class(count.typ().expect("count should have a type"))
                .name(),
            "EInt"
        );

        let notify = node
            .operations()
            .iter()
            .find(|operation| operation.name() == "notify")
            .expect("notify operation should exist");
        assert_eq!(notify.bounds().lbound, 0);
        assert_eq!(notify.bounds().ubound, Some(1));
        assert_eq!(notify.typ(), None);
        assert_eq!(notify.ordered(), None);
        assert_eq!(notify.unique(), None);
        assert!(notify.is_ordered());
        assert!(notify.is_unique());

        let value = notify
            .parameters()
            .iter()
            .find(|parameter| parameter.name() == "value")
            .expect("value parameter should exist");
        assert_eq!(value.bounds().lbound, 0);
        assert_eq!(value.bounds().ubound, Some(1));
        assert_eq!(value.typ(), None);
        assert_eq!(value.ordered(), None);
        assert_eq!(value.unique(), None);
        assert!(value.is_ordered());
        assert!(value.is_unique());
    }

    #[test]
    fn parses_classifier_instance_class_name() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EDataType"
        name="Identifier"
        instanceClassName="java.lang.String"
        instanceTypeName="java.lang.CharSequence"/>
    <eClassifiers xsi:type="ecore:EClass"
        name="Node"
        instanceClassName="org.example.Node"/>
</ecore:EPackage>
"##;

        let ctx = Ctx::parse(ecore).expect("ecore should parse");
        let identifier = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Identifier")
            .expect("Identifier classifier should exist");
        let node = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Node")
            .expect("Node classifier should exist");

        assert_eq!(identifier.instance_class_name(), Some("java.lang.String"));
        assert_eq!(identifier.inst_name(), Some("java.lang.CharSequence"));
        assert_eq!(node.instance_class_name(), Some("org.example.Node"));
        assert_eq!(node.inst_name(), None);
    }

    const ECORE_NS_URI: &str = "http://www.eclipse.org/emf/2002/Ecore";

    /// The five classes of Ecore that metamodels use, written as `Ecore.ecore` declares them,
    /// except that `EString` is the builtin datatype, `EObject` is abstract and has none of its
    /// operations, whose types are Ecore classes outside these five.
    const ECORE_SUBSET: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore" name="ecore" nsURI="http://www.eclipse.org/emf/2002/Ecore" nsPrefix="ecore">
  <eClassifiers xsi:type="ecore:EClass" name="EAnnotation" eSuperTypes="#//EModelElement">
    <eAnnotations source="http://www.eclipse.org/emf/2002/Ecore">
      <details key="constraints" value="WellFormed WellFormedSourceURI"/>
    </eAnnotations>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="source" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="details" upperBound="-1"
        eType="#//EStringToStringMapEntry" containment="true" resolveProxies="false"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="eModelElement" eType="#//EModelElement"
        transient="true" resolveProxies="false" eOpposite="#//EModelElement/eAnnotations"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="contents" upperBound="-1"
        eType="#//EObject" containment="true" resolveProxies="false"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="references" upperBound="-1"
        eType="#//EObject"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="EModelElement" abstract="true">
    <eOperations name="getEAnnotation" eType="#//EAnnotation">
      <eParameters name="source" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    </eOperations>
    <eStructuralFeatures xsi:type="ecore:EReference" name="eAnnotations" upperBound="-1"
        eType="#//EAnnotation" containment="true" resolveProxies="false" eOpposite="#//EAnnotation/eModelElement"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="ENamedElement" abstract="true" eSuperTypes="#//EModelElement">
    <eAnnotations source="http://www.eclipse.org/emf/2002/Ecore">
      <details key="constraints" value="WellFormedName"/>
    </eAnnotations>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="EObject" abstract="true"/>
  <eClassifiers xsi:type="ecore:EClass" name="EStringToStringMapEntry" instanceClassName="java.util.Map$Entry">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="key" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="value" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
  </eClassifiers>
</ecore:EPackage>
"##;

    /// A user metamodel whose `Element` extends Ecore's `EModelElement`, as SysON's does, and
    /// whose `Part` has features typed by Ecore classes.
    const EXTENDS_ECORE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Element" abstract="true"
        eSuperTypes="http://www.eclipse.org/emf/2002/Ecore#//EModelElement">
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="elementId"
            eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Part" eSuperTypes="#//Element">
        <eStructuralFeatures xsi:type="ecore:EReference" name="payload" upperBound="-1"
            eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EObject" containment="true"/>
        <eStructuralFeatures xsi:type="ecore:EReference" name="notes" upperBound="-1"
            eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EAnnotation"/>
    </eClassifiers>
</ecore:EPackage>
"##;

    fn class_named<'ctx>(ctx: &'ctx Ctx, pack: crate::repr::idx::Pack, name: &str) -> &'ctx Class {
        ctx.classes()
            .iter()
            .find(|class| class.path.last() == pack && class.name() == name)
            .unwrap_or_else(|| panic!("no class `{name}` in package `{}`", ctx[pack].name()))
    }

    fn pack_named(ctx: &Ctx, name: &str) -> crate::repr::idx::Pack {
        ctx.packs()
            .iter()
            .find(|pack| pack.name() == name)
            .unwrap_or_else(|| panic!("no package `{name}`"))
            .idx
    }

    /// Everything the context holds about a class, with classes named by package and name
    /// instead of by index.
    fn describe_class(ctx: &Ctx, class: &Class) -> String {
        use std::fmt::Write;

        let name = |idx: Option<crate::repr::idx::Class>| {
            idx.map(|idx| format!("{}/{}", ctx[ctx[idx].path.last()].name(), ctx[idx].name()))
        };
        let annots = |out: &mut String, annots: &[crate::repr::Annot]| {
            for annot in annots {
                let mut details: Vec<_> = annot.details().iter().collect();
                details.sort();
                writeln!(
                    out,
                    "  annotation {:?} {details:?} {:?}",
                    annot.source(),
                    annot.references()
                )
                .unwrap();
            }
        };

        let mut out = String::new();
        writeln!(
            out,
            "{} {} abstract={} interface={} inst_name={:?} instance_class_name={:?} literals={}",
            class.name(),
            class.typ(),
            class.is_abstract(),
            class.is_interface(),
            class.inst_name(),
            class.instance_class_name(),
            class.literals().len(),
        )
        .unwrap();
        let sups: Vec<_> = class.sup().iter().map(|idx| name(Some(*idx))).collect();
        writeln!(out, "  supertypes {sups:?}").unwrap();
        annots(&mut out, class.annotations());
        for s in class.structural() {
            writeln!(
                out,
                "  feature {} {} {:?} {:?} {} containment={} iD={} ordered={:?} changeable={:?} \
                volatile={:?} transient={:?} derived={:?} unsettable={:?} unique={:?} \
                default={:?}/{:?} resolve_proxies={:?}",
                s.name,
                s.kind,
                name(s.typ),
                s.typ_path,
                s.bounds,
                s.containment,
                s.is_id,
                s.ordered,
                s.changeable,
                s.volatile,
                s.transient,
                s.derived,
                s.unsettable,
                s.unique,
                s.default_value,
                s.default_value_literal,
                s.resolve_proxies,
            )
            .unwrap();
            annots(&mut out, s.annotations());
        }
        for op in class.operations() {
            writeln!(
                out,
                "  operation {} {:?} {} ordered={:?} unique={:?}",
                op.name(),
                name(op.typ()),
                op.bounds(),
                op.ordered(),
                op.unique(),
            )
            .unwrap();
            for param in op.parameters() {
                writeln!(
                    out,
                    "    parameter {} {:?} {} ordered={:?} unique={:?}",
                    param.name(),
                    name(param.typ()),
                    param.bounds(),
                    param.ordered(),
                    param.unique(),
                )
                .unwrap();
            }
            annots(&mut out, op.annotations());
        }
        out
    }

    macro_rules! example {
        ($ecore:literal, $pretty:literal) => {
            (
                $ecore,
                include_str!(concat!("../../", $ecore)),
                include_str!(concat!("../rsc/pretty/", $pretty)),
            )
        };
    }

    /// The pretty print of each example as the parser printed it before Ecore's own classes were
    /// built in, except that `multiple_inheritance.ecore` lists the supertypes of `C` in declared
    /// order (`aas.ecore` is left out: it refers to UML and is refused).
    const EXAMPLES: [(&str, &str, &str); 12] = [
        example!("examples/behavior_tree.ecore", "behavior_tree.pretty"),
        example!("examples/class_hierarchy.ecore", "class_hierarchy.pretty"),
        example!("examples/conference.ecore", "conference.pretty"),
        example!("examples/json.ecore", "json.pretty"),
        example!(
            "examples/pet_metamodels/abstract_inherits_concrete.ecore",
            "abstract_inherits_concrete.pretty"
        ),
        example!(
            "examples/pet_metamodels/concrete_inherits_concrete.ecore",
            "concrete_inherits_concrete.pretty"
        ),
        example!(
            "examples/pet_metamodels/concrete_polymorphic_targets.ecore",
            "concrete_polymorphic_targets.pretty"
        ),
        example!(
            "examples/pet_metamodels/kitchen_sink.ecore",
            "kitchen_sink.pretty"
        ),
        example!(
            "examples/pet_metamodels/multiple_inheritance.ecore",
            "multiple_inheritance.pretty"
        ),
        example!(
            "arachne-parser/rsc/AbstractEcore.ecore",
            "AbstractEcore.pretty"
        ),
        example!("arachne-parser/rsc/bt.ecore", "bt.pretty"),
        example!(
            "arachne-parser/rsc/ExampleEcore.ecore",
            "ExampleEcore.pretty"
        ),
    ];

    /// A metamodel that never refers to an Ecore class must not see any of them: same packages,
    /// same class indices, same pretty print.
    #[test]
    fn examples_without_ecore_classes_parse_as_before() {
        for (path, ecore, pretty) in EXAMPLES {
            let ctx = Ctx::parse(ecore).unwrap_or_else(|e| panic!("{path} refused: {e}"));
            assert!(
                ctx.packs()
                    .iter()
                    .all(|pack| pack.ns_uri() != Some(ECORE_NS_URI)),
                "{path} has a package for Ecore"
            );
            assert_eq!(
                format!("{}\n", ctx.to_pretty_string()),
                pretty,
                "{path} does not parse as before"
            );
        }
    }

    /// `resolve_etype` knows Ecore's datatypes and nothing else of Ecore, so a class extending
    /// `EModelElement` or a feature typed by `EObject` is refused.
    #[test]
    fn injects_ecore_classes_once_when_referenced() {
        let ctx = Ctx::parse(EXTENDS_ECORE).unwrap_or_else(|e| panic!("refused: {e}"));

        let ecore_packs: Vec<_> = ctx
            .packs()
            .iter()
            .filter(|pack| pack.ns_uri() == Some(ECORE_NS_URI))
            .collect();
        assert_eq!(ecore_packs.len(), 1, "expected one Ecore package");
        let ecore = ecore_packs[0];
        assert_eq!(ecore.name(), "ecore");
        assert_eq!(ecore.ns_prefix(), Some("ecore"));
        assert_eq!(ecore.sup(), Some(ctx.top_pack()));
        assert!(ctx[ctx.top_pack()].has_sub(ecore.idx));
        let mut names: Vec<_> = ecore.classes().iter().map(|idx| ctx[*idx].name()).collect();
        names.sort();
        assert_eq!(
            names,
            [
                "EAnnotation",
                "EModelElement",
                "ENamedElement",
                "EObject",
                "EStringToStringMapEntry"
            ]
        );
        assert_eq!(ctx[ctx.builtin_pack()].classes().len(), 10);

        let test = pack_named(&ctx, "test");
        let element = class_named(&ctx, test, "Element");
        let sups: Vec<_> = element.sup().iter().copied().collect();
        assert_eq!(sups, [class_named(&ctx, ecore.idx, "EModelElement").idx]);
        assert_eq!(
            element.structural()[0].typ,
            Some(ctx.builtins()[&crate::repr::builtin::Typ::EString])
        );

        let part = class_named(&ctx, test, "Part");
        let typ = |feature: &str| {
            part.structural()
                .iter()
                .find(|candidate| candidate.name == feature)
                .and_then(|feature| feature.typ)
        };
        assert_eq!(
            typ("payload"),
            Some(class_named(&ctx, ecore.idx, "EObject").idx)
        );
        assert_eq!(
            typ("notes"),
            Some(class_named(&ctx, ecore.idx, "EAnnotation").idx)
        );
    }

    /// The classes built in for Ecore hold what the parser would hold for them if it read their
    /// declarations in `Ecore.ecore`.
    #[test]
    fn built_in_ecore_classes_match_their_declarations() {
        let injected = Ctx::parse(EXTENDS_ECORE).unwrap_or_else(|e| panic!("refused: {e}"));
        let declared = Ctx::parse(ECORE_SUBSET).unwrap_or_else(|e| panic!("refused: {e}"));
        let injected_pack = pack_named(&injected, "ecore");
        let declared_pack = pack_named(&declared, "ecore");

        for name in [
            "EAnnotation",
            "EModelElement",
            "ENamedElement",
            "EObject",
            "EStringToStringMapEntry",
        ] {
            assert_eq!(
                describe_class(&injected, class_named(&injected, injected_pack, name)),
                describe_class(&declared, class_named(&declared, declared_pack, name)),
            );
        }
    }

    /// A metamodel that is Ecore itself names its own classes by Ecore's URI; they must resolve
    /// to the file's classes and no second Ecore package may appear.
    #[test]
    fn ecore_itself_resolves_ecore_uris_to_its_own_classes() {
        let probe = r##"<eClassifiers xsi:type="ecore:EClass" name="Probe" eSuperTypes="http://www.eclipse.org/emf/2002/Ecore#//EModelElement">
    <eStructuralFeatures xsi:type="ecore:EReference" name="payload" eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EObject"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="EAnnotation""##;
        let ecore = ECORE_SUBSET.replacen(
            r#"<eClassifiers xsi:type="ecore:EClass" name="EAnnotation""#,
            probe,
            1,
        );
        let ctx = Ctx::parse(&ecore).unwrap_or_else(|e| panic!("refused: {e}"));

        assert_eq!(
            ctx.packs()
                .iter()
                .filter(|pack| pack.ns_uri() == Some(ECORE_NS_URI))
                .count(),
            1,
            "a second Ecore package was added"
        );
        let own = pack_named(&ctx, "ecore");
        let probe = class_named(&ctx, own, "Probe");
        let sups: Vec<_> = probe.sup().iter().copied().collect();
        assert_eq!(sups, [class_named(&ctx, own, "EModelElement").idx]);
        assert_eq!(
            probe.structural()[0].typ,
            Some(class_named(&ctx, own, "EObject").idx)
        );
    }

    /// A user metamodel naming Ecore's classes in the given `eSuperTypes` and `eType` spellings.
    fn naming_ecore(super_types: &str, etype: &str) -> String {
        format!(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Element" eSuperTypes="{super_types}">
        <eStructuralFeatures xsi:type="ecore:EReference" name="payload" eType="{etype}"/>
    </eClassifiers>
</ecore:EPackage>
"##
        )
    }

    /// `resolve_etype` recognises Ecore's classes by Ecore's namespace URI only, so the
    /// `platform:/plugin` location of `Ecore.ecore`, which the Ecore editor inserts by default and
    /// SysON's metamodel uses, is refused.
    #[test]
    fn resolves_the_platform_spelling_of_ecore_classes() {
        const NS_URI: &str = "http://www.eclipse.org/emf/2002/Ecore#//";
        const PLATFORM: &str = "platform:/plugin/org.eclipse.emf.ecore/model/Ecore.ecore#//";

        for (super_types, etype) in [
            (format!("{NS_URI}EModelElement"), format!("{NS_URI}EObject")),
            (
                format!("{PLATFORM}EModelElement"),
                format!("ecore:EClass {PLATFORM}EObject"),
            ),
            (
                format!("{PLATFORM}EModelElement"),
                format!("{PLATFORM}EObject"),
            ),
            (
                format!("{PLATFORM}EModelElement {NS_URI}ENamedElement"),
                format!("ecore:EClass {NS_URI}EObject"),
            ),
        ] {
            let ecore = naming_ecore(&super_types, &etype);
            let ctx = Ctx::parse(&ecore)
                .unwrap_or_else(|e| panic!("`{super_types}` and `{etype}` refused: {e}"));
            let ecore_packs: Vec<_> = ctx
                .packs()
                .iter()
                .filter(|pack| pack.ns_uri() == Some(ECORE_NS_URI))
                .map(|pack| pack.idx)
                .collect();
            assert_eq!(ecore_packs.len(), 1, "`{super_types}`: one Ecore package");
            let ecore = ecore_packs[0];

            let element = class_named(&ctx, pack_named(&ctx, "test"), "Element");
            let sups: Vec<_> = element.sup().iter().map(|idx| ctx[*idx].name()).collect();
            let expected: Vec<_> = super_types
                .split_whitespace()
                .map(|token| token.rsplit('/').next().unwrap())
                .collect();
            assert_eq!(sups, expected, "`{super_types}`");
            assert!(element
                .sup()
                .iter()
                .all(|idx| ctx[*idx].path.last() == ecore));
            assert_eq!(
                element.structural()[0].typ,
                Some(class_named(&ctx, ecore, "EObject").idx),
                "`{etype}`"
            );
        }
    }

    /// Ecore itself may name its classes by the `platform:/plugin` location of `Ecore.ecore`,
    /// which is the file itself.
    #[test]
    fn ecore_itself_resolves_platform_uris_to_its_own_classes() {
        let probe = r##"<eClassifiers xsi:type="ecore:EClass" name="Probe" eSuperTypes="platform:/plugin/org.eclipse.emf.ecore/model/Ecore.ecore#//EModelElement">
    <eStructuralFeatures xsi:type="ecore:EReference" name="payload" eType="ecore:EClass platform:/plugin/org.eclipse.emf.ecore/model/Ecore.ecore#//EObject"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="EAnnotation""##;
        let ecore = ECORE_SUBSET.replacen(
            r#"<eClassifiers xsi:type="ecore:EClass" name="EAnnotation""#,
            probe,
            1,
        );
        let ctx = Ctx::parse(&ecore).unwrap_or_else(|e| panic!("refused: {e}"));

        assert_eq!(ctx.packs().len(), 3, "a package was added to Ecore itself");
        let own = pack_named(&ctx, "ecore");
        let probe = class_named(&ctx, own, "Probe");
        let sups: Vec<_> = probe.sup().iter().copied().collect();
        assert_eq!(sups, [class_named(&ctx, own, "EModelElement").idx]);
        assert_eq!(
            probe.structural()[0].typ,
            Some(class_named(&ctx, own, "EObject").idx)
        );
    }

    /// A name of Ecore outside the classes built in falls through to the generic refusal, which
    /// says `eType` for a supertype and does not say which of Ecore's classes are supported.
    #[test]
    fn refuses_other_ecore_classes_naming_the_supported_ones() {
        for (super_types, etype, class) in [
            (
                "http://www.eclipse.org/emf/2002/Ecore#//EClass",
                "ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EObject",
                "EClass",
            ),
            (
                "http://www.eclipse.org/emf/2002/Ecore#//EModelElement",
                "ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EReference",
                "EReference",
            ),
            (
                "platform:/plugin/org.eclipse.emf.ecore/model/Ecore.ecore#//EPackage",
                "ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EObject",
                "EPackage",
            ),
        ] {
            let error = match Ctx::parse(naming_ecore(super_types, etype)) {
                Ok(_) => panic!("`{class}` accepted"),
                Err(e) => e.to_string(),
            };
            assert!(
                error.contains(&format!("`{class}`")),
                "the error does not name `{class}`: {error}"
            );
            for supported in [
                "EAnnotation",
                "EModelElement",
                "ENamedElement",
                "EObject",
                "EStringToStringMapEntry",
            ] {
                assert!(
                    error.contains(&format!("`{supported}`")),
                    "the error does not list `{supported}`: {error}"
                );
            }
        }
    }

    /// EMF resolves `Ecore.ecore#//X` against the metamodel's own location and `#//X` in the
    /// metamodel itself, so neither means the built-in Ecore; both keep their behaviour, and so do
    /// Ecore's datatypes.
    #[test]
    fn relative_ecore_spellings_and_datatypes_resolve_as_before() {
        for (super_types, etype) in [
            (
                "Ecore.ecore#//EModelElement",
                "ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString",
            ),
            ("", "ecore:EClass Ecore.ecore#//EObject"),
        ] {
            let error = match Ctx::parse(naming_ecore(super_types, etype)) {
                Ok(_) => panic!("`{super_types}` and `{etype}` accepted"),
                Err(e) => e.to_string(),
            };
            assert!(
                error.contains("unsupported `eType` path `") && error.contains("Ecore.ecore#//"),
                "unexpected refusal: {error}"
            );
        }

        // no class of the metamodel is named `EObject`: an undefined forward reference
        let error = match Ctx::parse(naming_ecore("#//EObject", "#//EObject")) {
            Ok(_) => panic!("`#//EObject` accepted with no local `EObject`"),
            Err(e) => e.to_string(),
        };
        assert!(
            error.contains("failed to finalize parsing context"),
            "unexpected refusal: {error}"
        );

        let ecore = naming_ecore(
            "#//EObject",
            "ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString",
        )
        .replace(
            "</ecore:EPackage>",
            r#"<eClassifiers xsi:type="ecore:EClass" name="EObject"/></ecore:EPackage>"#,
        );
        let ctx = Ctx::parse(&ecore).unwrap_or_else(|e| panic!("refused: {e}"));
        assert!(ctx
            .packs()
            .iter()
            .all(|pack| pack.ns_uri() != Some(ECORE_NS_URI)));
        let test = pack_named(&ctx, "test");
        let element = class_named(&ctx, test, "Element");
        let sups: Vec<_> = element.sup().iter().copied().collect();
        assert_eq!(sups, [class_named(&ctx, test, "EObject").idx]);
        assert_eq!(
            element.structural()[0].typ,
            Some(ctx.builtins()[&crate::repr::builtin::Typ::EString])
        );
    }

    /// `class` parses the whole body of a class before it resolves the supertypes, so a supertype
    /// that fails to resolve is reported at the end of the class, where the parser stands.
    #[test]
    fn reports_a_failed_supertype_at_its_class() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Base"/>
    <eClassifiers xsi:type="ecore:EClass" name="Derived" eSuperTypes="#//Base other.ecore#//Base">
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="label"
            eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="count"
            eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Next"/>
</ecore:EPackage>
"##;
        let line = 1 + ecore
            .lines()
            .position(|line| line.contains(r#"name="Derived""#))
            .unwrap();

        let error = match Ctx::parse(ecore) {
            Ok(_) => panic!("`other.ecore#//Base` accepted"),
            Err(e) => e.to_string(),
        };
        assert!(
            error.contains(&format!("line {line}, ")),
            "the error is not reported at line {line}: {error}"
        );
        assert!(
            error.contains("`Derived`") && error.contains("other.ecore#//Base"),
            "the error names neither `Derived` nor its supertype: {error}"
        );
    }

    /// `Class` keeps its supertypes in a `BTreeSet`, so they come back ordered by class index,
    /// that is by first mention in the file, and not in the order `eSuperTypes` declares them,
    /// which EMF uses to order inherited features and to pick the implementation's base class.
    #[test]
    fn keeps_supertypes_in_declaration_order() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="A" abstract="true"/>
    <eClassifiers xsi:type="ecore:EClass" name="B" abstract="true"/>
    <eClassifiers xsi:type="ecore:EClass" name="C" abstract="true"/>
    <eClassifiers xsi:type="ecore:EClass" name="D" eSuperTypes="#//C #//A #//B"/>
    <eClassifiers xsi:type="ecore:EClass" name="E" eSuperTypes="#//Later #//A"/>
    <eClassifiers xsi:type="ecore:EClass" name="F" eSuperTypes="#//B #//A #//B"/>
    <eClassifiers xsi:type="ecore:EClass" name="Later" abstract="true"/>
</ecore:EPackage>
"##;

        let ctx = Ctx::parse(ecore).unwrap_or_else(|e| panic!("refused: {e}"));
        let test = pack_named(&ctx, "test");
        let sups = |name: &str| {
            class_named(&ctx, test, name)
                .sup()
                .iter()
                .map(|idx| ctx[*idx].name().to_string())
                .collect::<Vec<_>>()
        };

        assert_eq!(sups("D"), ["C", "A", "B"]);
        assert_eq!(sups("E"), ["Later", "A"]);
        assert_eq!(sups("F"), ["B", "A"]);

        let subs = |name: &str| {
            class_named(&ctx, test, name)
                .sub()
                .iter()
                .map(|idx| ctx[*idx].name().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(subs("A"), ["D", "E", "F"]);
        assert_eq!(subs("B"), ["D", "F"]);
        assert_eq!(subs("Later"), ["E"]);

        assert!(ctx.to_pretty_string().contains("supers: C, A, B"));
    }
}
