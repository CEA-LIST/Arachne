pub mod analysis;
pub mod containment;
pub mod generate;

use ecore_rs::{ctx::Ctx, prelude::idx};
use proc_macro2::TokenStream;
use quote::quote;

use crate::codegen::{
    cycles::CycleAnalysis,
    generate::Fragment,
    package::generate_package_log,
    reference::{
        analysis::analyze_references,
        containment::find_creation_paths,
        generate::{
            generate_edge_structs, generate_id_structs, generate_package_enum, generate_typed_graph,
        },
    },
};

pub const PATH_MOD: &str = "classifiers";

/// Top-level generator for non-containment reference support.
///
/// Given the Ecore context and the root class, generates:
/// - ID structs for referenceable classes
/// - Edge structs for each non-containment reference
/// - `typed_graph!` macro invocation (ReferenceManager)
/// - Package enum (root class + Reference operations)
/// - Package log struct with `IsLog` and `EvalNested` implementations
pub struct PackageGenerator<'a> {
    ctx: &'a Ctx,
    root_class: idx::Class,
    package_classes: Vec<idx::Class>,
    package_name: &'a str,
    cycle_analysis: &'a CycleAnalysis,
}

impl<'a> PackageGenerator<'a> {
    pub fn new(
        ctx: &'a Ctx,
        root_class: idx::Class,
        package_classes: Vec<idx::Class>,
        package_name: &'a str,
        cycle_analysis: &'a CycleAnalysis,
    ) -> Self {
        Self {
            ctx,
            root_class,
            package_name,
            package_classes,
            cycle_analysis,
        }
    }

    /// Generate the complete reference management code.
    /// Returns `None` if there are no non-containment references.
    pub fn generate(&self) -> Option<Fragment> {
        let analysis = analyze_references(self.ctx, &self.package_classes);

        if !analysis.has_references() {
            return None;
        }

        let root_class = &self.ctx.classes()[*self.root_class];
        let root_class_name = root_class.name();

        // Phase 1: Find creation paths
        let creation_paths =
            find_creation_paths(self.ctx, self.root_class, &analysis, self.cycle_analysis);

        // Phase 2: Generate ID structs
        let id_structs = generate_id_structs(self.ctx, &analysis);

        // Phase 3: Generate Edge structs
        let edge_structs = generate_edge_structs(&analysis);

        // Phase 4: Generate typed_graph! macro
        let typed_graph = generate_typed_graph(self.ctx, &analysis);

        // Phase 5: Generate Package enum
        let package_enum = generate_package_enum(self.package_name, root_class_name);

        // Phases 6-7: Generate PackageLog + IsLog + EvalNested
        let package_log = generate_package_log(
            self.ctx,
            &analysis,
            self.package_name,
            root_class_name,
            &creation_paths,
        );

        // Collect all generated imports
        let imports = generate_imports();

        let path: syn::Path = syn::parse_str(PATH_MOD).unwrap();

        let tokens = quote! {
            use crate::#path;
            #imports

            #id_structs
            #edge_structs
            #typed_graph
            #package_enum
            #package_log
        };

        Some(Fragment::new(tokens, vec![], vec![]))
    }
}

/// Generate the use statements needed by the model code.
fn generate_imports() -> TokenStream {
    quote! {
        use moirai_crdt::{list::nested_list::List, policy::LwwPolicy};
        use moirai_macros::typed_graph;
        use moirai_protocol::{
            clock::version_vector::Version,
            crdt::{
                eval::EvalNested,
                pure_crdt::PureCRDT,
                query::{QueryOperation, Read},
            },
            event::{Event, id::EventId},
            state::{log::IsLog, po_log::VecLog},
        };
    }
}
