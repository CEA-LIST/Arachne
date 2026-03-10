# Cycle Detection Algorithm - Implementation Guide

## Overview

The cycle detection algorithm (`src/codegen/cycles.rs`) automatically detects cycles in the Ecore containment hierarchy and determines where `Box<T>` wrappers are needed. This guide explains how to integrate it into the code generation pipeline.

## Algorithm Phases

### Phase 1: Build Containment Graph

```
Input:  Ecore model (parsed into ctx: Ctx)
Output: Vec<ContainmentEdge> - all containment relationships

For each class in the model:
  For each containment reference in that class:
    - Add edge (source → target)
    - For abstract targets: add edges to all concrete subtypes
    - For inherited features: add edge to superclass
```

**Key insight**: This creates a complete graph including transitive relationships through inheritance and union types.

### Phase 2: Find All Cycles

```
Input:  Graph (Vec<ContainmentEdge>)
Output: Vec<Vec<ContainmentEdge>> - all elementary cycles

Algorithm: Depth-First Search with recursion stack
  - Use DFS to traverse from each unvisited node
  - Detect back edges (edges to nodes in current recursion stack)
  - Extract the cycle path when a back edge is found
  - Remove duplicates (same cycle through different starting points)
```

**Key data structures**:

- `visited`: Tracks all discovered nodes
- `rec_stack`: Current DFS path (recursion stack)
- `rec_stack_set`: HashSet for O(1) membership check in recursion stack

### Phase 3: Determine Boxing Strategy

```
Input:  Cycles (Vec<Vec<ContainmentEdge>>)
Output: HashMap<(ClassIdx, FieldName), BoxingStrategy>

For each cycle:
  Select the BEST edge to break using heuristics:
    1. Union variant edges (highest priority)
    2. Collection element edges (many-cardinality)
    3. Edges at deeper nesting levels
    4. Edges that appear in multiple cycles
```

**Boxing strategies**:

- `DirectReference`: Box the field itself → `field: Box<T>`
- `CollectionElement`: Box elements in collection → `field: ListLog<Box<T>>`
- `NoBox`: Cycle broken elsewhere, no boxing needed

## Integration Points

### 1. Early Analysis (During Project Setup)

Add cycle analysis to `ctx` creation:

```rust
// In src/main.rs or wherever the parser is initialized
use crate::codegen::cycles::analyze_cycles;

let parser = EcoreParser::from_file(&ecore_file)?;
let ctx = parser.ctx;

// Run cycle analysis once
let cycle_analysis = analyze_cycles(&ctx)?;
```

### 2. Store in Context

Extend the analysis scope by storing analysis results (optional, for efficiency):

```rust
// Extended context structure
pub struct AnalysisContext<'a> {
    pub ecore_ctx: &'a Ctx,
    pub cycles: CycleAnalysis,
}

// Create once per generation
let analysis = AnalysisContext {
    ecore_ctx: &ctx,
    cycles: analyze_cycles(&ctx)?,
};
```

### 3. Use in ReferenceGenerator

Modify `src/codegen/feature/reference.rs` to check boxing requirements:

```rust
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

        // NEW: Check if this field needs boxing
        let needs_boxing = false;  // Would be replaced by querying cycle analysis
        // let needs_boxing = cycle_analysis.needs_boxing(source_class_idx, self.reference.name);

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
```

### 4. Architectural Pattern

```
┌──────────────────┐
│  Ecore Model     │
│  (parsed)        │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ analyze_cycles() │  ◄─── NEW
│ (Phase 1-3)      │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│CycleAnalysis     │
│ - cycles         │
│ - boxing_req.    │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ReferenceGen.     │
│generate()        │  ◄─── Queries boxing_req
│                  │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│Generated code    │
│(with Box<T>)     │
└──────────────────┘
```

## Algorithm Complexity

| Phase | Time Complexity | Space Complexity |
|-------|-----------------|------------------|
| Build Graph | O(V + E) where V=classes, E=references | O(V + E) |
| DFS Cycle Detection | O(V + E) | O(V + E) |
| Select Boxing | O(C × L) where C=cycles, L=cycle length | O(C × L) |
| **Total** | **O(V + E + C×L)** | **O(V + E)** |

In practice, typical Ecore models have:

- V = 20-500 classes
- E = 30-1000 references
- C = 0-10 cycles

So the algorithm runs in **well under 1ms** for typical models.

## Heuristics for Boxing Selection

### Heuristic 1: Union Variant Edges (Highest Priority)

```
Why: Abstract/interface types can create complex recursive patterns
     Breaking at the most specific variant is safest

Example:
  TreeNode is abstract
  Decorator extends TreeNode
  Decorator has reference to TreeNode
  
  Boxing break: Decorator.child: Box<TreeNodeLog> ✓
```

### Heuristic 2: Collection Element Edges

```
Why: Rust collections can grow without size constraints
     Boxing the element allows efficient memory layout

Example:
  ControlNode has many TreeNode children
  
  Boxing break: children: ListLog<Box<TreeNodeLog>> ✓
  NOT:         children: Box<ListLog<TreeNodeLog>> ✗ (wrong!)
```

### Heuristic 3: Inheritance-Level Boxing

```
Why: If all subclasses need boxing at the same point, 
     apply at parent level to avoid duplication

Example:
  abstract TreeNodeFeat { child: Box<TreeNodeLog> }
  
  All concrete subclasses inherit this field
  No need to re-box in each subclass ✓
```

### Heuristic 4: Leaf-Level Boxing

```
Why: Minimize boxing scope
     Don't box abstract types if only concrete subtypes need it

Example:
  ❌ abstract TreeNode { child: Box<TreeNode> }
  ✓  Decorator(extends TreeNode) { child: Box<TreeNodeLog> }
```

## Testing the Algorithm

### Test Case 1: Simple Self-Cycle

```ecore
class Node {
  ref Node next
}
```

**Expected**: Detects cycle (Node → Node), boxes `next` field

### Test Case 2: Mutual Cycle

```ecore
class A {
  ref B b
}
class B {
  ref A a
}
```

**Expected**: Detects cycle (A ↔ B), boxes one edge

### Test Case 3: Indirect Cycle Through Inheritance

```ecore
abstract TreeNode {
  ref TreeNode child
}
class Leaf extends TreeNode
```

**Expected**: Detects inheritance cycle, boxes at most specific level

### Test Case 4: Union Type Cycle

```ecore
abstract TreeNode
class Decorator extends TreeNode {
  ref TreeNode child
}
```

**Expected**: Detects union variant cycle (TreeNode → Decorator → TreeNode),
boxes Decorator.child

## Future Optimizations

1. **Strongly Connected Components (SCC)**: Use Tarjan's algorithm for better cycle detection
2. **Minimal Feedback Arc Set**: NP-hard, but good approximations exist (branch-and-bound)
3. **Caching**: Store results in `Ctx` struct for multi-pass generation
4. **Warnings**: Generate warnings for complex cycles requiring multiple Box wrappers
5. **Documentation Generation**: Auto-generate comments explaining why Box was needed

## Related Code

- **Implementation**: [cycles.rs](../src/codegen/cycles.rs)
- **Reference Generator**: [reference.rs](../src/codegen/feature/reference.rs)
- **Class Generator**: [class.rs](../src/codegen/classifier/class.rs)
- **Algorithm Document**: [BOX_DETECTION_ALGORITHM.md](./BOX_DETECTION_ALGORITHM.md)
