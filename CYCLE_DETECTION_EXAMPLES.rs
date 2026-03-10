// Example usage patterns for the cycle detection algorithm
//
// This file demonstrates how to integrate cycle detection into the code generation pipeline

use crate::codegen::cycles::{BoxingStrategy, CycleAnalysis, analyze_cycles};
use ecore_rs::ctx::Ctx;

/// Example 1: Basic cycle detection from an Ecore model
pub fn example_basic_analysis(ctx: &Ctx) -> anyhow::Result<()> {
    // Analyze cycles in the model
    let cycle_analysis = analyze_cycles(ctx)?;

    println!("Found {} cycles", cycle_analysis.cycles.len());

    // Iterate through all boxing requirements
    for ((class_idx, field_name), strategy) in &cycle_analysis.boxing_requirements {
        match strategy {
            BoxingStrategy::DirectReference => {
                println!(
                    "Class {} field {}: Need Box<T>",
                    ctx.classes()[*class_idx].name(),
                    field_name
                );
            }
            BoxingStrategy::CollectionElement => {
                println!(
                    "Class {} field {}: Need ListLog<Box<T>>",
                    ctx.classes()[*class_idx].name(),
                    field_name
                );
            }
            BoxingStrategy::NoBox => {
                // Default - no action needed
            }
        }
    }

    Ok(())
}

/// Example 2: Using cycle analysis within a reference generator
pub fn example_reference_generator_integration(
    ctx: &Ctx,
    cycle_analysis: &CycleAnalysis,
    source_class_idx: ecore_rs::repr::ClassIdx,
    field_name: &str,
) -> bool {
    // Query whether this field needs boxing
    cycle_analysis.needs_boxing(source_class_idx, field_name)
}

/// Example 3: Building with cycle analysis (conceptual code generator flow)
pub fn example_code_generation_flow(ecore_model_path: &str) -> anyhow::Result<()> {
    // Step 1: Parse Ecore model
    use crate::parser::EcoreParser;
    let parser = EcoreParser::from_file(ecore_model_path)?;
    let ctx = parser.ctx;

    // Step 2: Run cycle analysis ONCE before generating code
    let cycle_analysis = analyze_cycles(&ctx)?;

    // Step 3: Pass to generators that need it
    // (Would be done through constructor or thread-local)

    // In pseudo-code, each reference generator would do:
    // for field in class.structural_features() {
    //     let needs_boxing = cycle_analysis.needs_boxing(class_idx, field.name);
    //     if needs_boxing {
    //         generate_with_box(field)
    //     } else {
    //         generate_without_box(field)
    //     }
    // }

    Ok(())
}

/// Example 4: Generating a summary report
pub fn example_cycle_report(ctx: &Ctx, cycle_analysis: &CycleAnalysis) -> String {
    let mut report = String::from("=== Cycle Analysis Report ===\n\n");

    // Count boxing requirements
    let direct_refs = cycle_analysis
        .boxing_requirements
        .values()
        .filter(|s| **s == BoxingStrategy::DirectReference)
        .count();

    let collection_elems = cycle_analysis
        .boxing_requirements
        .values()
        .filter(|s| **s == BoxingStrategy::CollectionElement)
        .count();

    report.push_str(&format!(
        "Total cycles detected: {}\n",
        cycle_analysis.cycles.len()
    ));
    report.push_str(&format!(
        "Box<T> needed for direct references: {}\n",
        direct_refs
    ));
    report.push_str(&format!(
        "ListLog<Box<T>> needed for collections: {}\n",
        collection_elems
    ));
    report.push_str(&format!(
        "Total boxing requirements: {}\n\n",
        direct_refs + collection_elems
    ));

    report.push_str("Details:\n");
    for ((class_idx, field_name), strategy) in &cycle_analysis.boxing_requirements {
        let class_name = ctx.classes()[*class_idx].name();
        let strategy_str = match strategy {
            BoxingStrategy::DirectReference => "Box<T>",
            BoxingStrategy::CollectionElement => "ListLog<Box<T>>",
            BoxingStrategy::NoBox => "No boxing",
        };

        report.push_str(&format!(
            "  {}.{}: {}\n",
            class_name, field_name, strategy_str
        ));
    }

    report
}

/// Example 5: Error handling with context
pub fn example_error_handling(ecore_model_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use crate::parser::EcoreParser;

    let parser = EcoreParser::from_file(ecore_model_path)
        .map_err(|e| format!("Failed to parse Ecore model: {}", e))?;

    let ctx = parser.ctx;

    let cycle_analysis =
        analyze_cycles(&ctx).map_err(|e| format!("Failed to analyze cycles: {}", e))?;

    if !cycle_analysis.boxing_requirements.is_empty() {
        eprintln!(
            "⚠️  {} boxing requirements detected",
            cycle_analysis.boxing_requirements.len()
        );
    } else {
        println!("✓ No cycles detected - no boxing needed");
    }

    Ok(())
}

/// Example 6: Extension point - custom analysis logic
pub fn example_custom_analysis(cycle_analysis: &CycleAnalysis) {
    // Find classes with multiple boxing requirements (potential refactoring candidates)
    use std::collections::HashMap;

    let mut class_boxing_count: HashMap<String, usize> = HashMap::new();

    for (class_idx, _) in &cycle_analysis.boxing_requirements {
        // Count would be computed in real scenario
        // Placeholder to show the pattern
        let _ = class_idx;
    }

    // Could emit warnings or recommendations
    for (class_name, count) in class_boxing_count {
        if count > 2 {
            eprintln!(
                "⚠️  Class '{}' has {} boxing requirements - consider refactoring",
                class_name, count
            );
        }
    }
}

/// Example 7: Integration with caching (for multiple passes)
pub struct GenerationContext<'a> {
    ctx: &'a Ctx,
    cycle_analysis: CycleAnalysis,
}

impl<'a> GenerationContext<'a> {
    pub fn new(ctx: &'a Ctx) -> anyhow::Result<Self> {
        let cycle_analysis = analyze_cycles(ctx)?;
        Ok(Self {
            ctx,
            cycle_analysis,
        })
    }

    pub fn should_box_reference(&self, class_idx: ecore_rs::repr::ClassIdx, field: &str) -> bool {
        self.cycle_analysis.needs_boxing(class_idx, field)
    }

    pub fn get_boxing_strategy(
        &self,
        class_idx: ecore_rs::repr::ClassIdx,
        field: &str,
    ) -> BoxingStrategy {
        self.cycle_analysis.boxing_strategy(class_idx, field)
    }
}

/// Example 8: Testing cycle detection with mock data
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_cycle_detection() {
        // This would use ecore-rs test fixtures
        // Example structure:
        // - Load test Ecore model with known cycles
        // - Run analyze_cycles
        // - Assert expected boxing requirements

        // Pseudocode:
        // let (ctx, expected_cycles) = load_test_fixture("cycles.ecore");
        // let analysis = analyze_cycles(&ctx).unwrap();
        // assert_eq!(analysis.cycles.len(), expected_cycles.count);
    }

    #[test]
    fn test_union_type_boxing() {
        // Test that union variant cycles are detected
        // and boxing is applied to the variant edge
    }

    #[test]
    fn test_collection_element_boxing() {
        // Test that collection references box the element type
        // not the collection itself
    }
}

// ============================================================================
// Pseudo-code showing where to integrate in existing generators
// ============================================================================

/*
CURRENT CODE (in src/codegen/feature/reference.rs):

impl<'a> Generate for ReferenceGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        if !self.reference.containment {
            return Ok(Fragment::new(TokenStream::new(), vec![], vec![]));
        }

        let path: syn::Path = syn::parse_str(PATH_MOD).unwrap();
        let (bound_kind, warnings) = normalize_bounds(self.reference.bounds, &self.reference.name);

        let target_class = self
            .ctx
            .classes()
            .get(*self.reference.typ.unwrap())
            .unwrap();

        let name = Ident::new(&self.reference.name.to_snake_case(), Span::call_site());
        let target_type = format_ident!("{}Log", target_class.name());

        let (field_type, imports) = match bound_kind {
            BoundKind::Single => (quote! { #target_type }, vec![]),
            BoundKind::Optional => (
                quote! { #path::OptionLog<#target_type> },
                vec![Import::Crdt(Crdt::Nested(NestedCrdt::Optional))],
            ),
            BoundKind::Many => (
                quote! { #path::ListLog<#target_type> },
                vec![Import::Crdt(Crdt::Nested(NestedCrdt::List))],
            ),
        };

        let stream = quote! { #name: #field_type };

        Ok(Fragment::new(stream, imports, warnings))
    }
}


MODIFIED CODE (with cycle detection):

pub struct ReferenceGenerator<'a> {
    reference: &'a Structural,
    ctx: &'a Ctx,
    cycle_analysis: &'a CycleAnalysis,  // NEW: Add cycle analysis
}

impl<'a> ReferenceGenerator<'a> {
    pub fn new(
        reference: &'a Structural,
        ctx: &'a Ctx,
        cycle_analysis: &'a CycleAnalysis,  // NEW
    ) -> Self {
        assert_eq!(reference.kind, ecore_rs::repr::structural::Typ::EReference);
        Self {
            reference,
            ctx,
            cycle_analysis,  // NEW
        }
    }
}

impl<'a> Generate for ReferenceGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        if !self.reference.containment {
            return Ok(Fragment::new(TokenStream::new(), vec![], vec![]));
        }

        let path: syn::Path = syn::parse_str(PATH_MOD).unwrap();
        let (bound_kind, warnings) = normalize_bounds(self.reference.bounds, &self.reference.name);

        let target_class = self
            .ctx
            .classes()
            .get(*self.reference.typ.unwrap())
            .unwrap();

        let name = Ident::new(&self.reference.name.to_snake_case(), Span::call_site());
        let target_type = format_ident!("{}Log", target_class.name());

        // NEW: Check cycle analysis
        let source_class_idx = /* get from context */;
        let needs_boxing = self
            .cycle_analysis
            .needs_boxing(source_class_idx, &self.reference.name);

        let (field_type, imports) = match bound_kind {
            BoundKind::Single => {
                if needs_boxing {
                    (quote! { Box<#target_type> }, vec![])
                } else {
                    (quote! { #target_type }, vec![])
                }
            }
            BoundKind::Optional => {
                let inner = if needs_boxing {
                    quote! { Box<#target_type> }
                } else {
                    quote! { #target_type }
                };
                (
                    quote! { #path::OptionLog<#inner> },
                    vec![Import::Crdt(Crdt::Nested(NestedCrdt::Optional))],
                )
            }
            BoundKind::Many => {
                let inner = if needs_boxing {
                    quote! { Box<#target_type> }
                } else {
                    quote! { #target_type }
                };
                (
                    quote! { #path::ListLog<#inner> },
                    vec![Import::Crdt(Crdt::Nested(NestedCrdt::List))],
                )
            }
        };

        let stream = quote! { #name: #field_type };

        Ok(Fragment::new(stream, imports, warnings))
    }
}

*/
