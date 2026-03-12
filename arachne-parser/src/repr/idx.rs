#![allow(clippy::implied_bounds_in_impls, clippy::len_without_is_empty)]

safe_index::new! {
    /// Concrete class index.
    Class,
    /// Maps a [`Class`] to something.
    map: ClassMap,
}

safe_index::new! {
    /// Package index.
    Pack,
    /// Maps a [`Pack`] to something.
    map: PackMap,
}
