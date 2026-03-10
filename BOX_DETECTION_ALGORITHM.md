# Box Indirection Detection Algorithm for Cycle-Free Type Definitions

## Analysis of Current Generated Code

### Cyclic Dependencies Identified

In the behavior tree example, Box is placed at three locations to break cycles:

1. **`DecoratorFeat.child: Box<TreeNodeLog>`**
   - Cycle: `TreeNodeLog` → `Decorator` → `TreeNodeFeat` → `Box<TreeNodeLog>`

2. **`ControlNodeFeat.children: __gen::ListLog<Box<TreeNodeLog>>`**
   - Cycle: `TreeNodeLog` → `ControlNode` → `ControlNodeFeat` → `ListLog<Box<TreeNodeLog>>`

3. **`SubTree.tree: Box<BehaviorTreeLog>`**
   - Cycle: `BehaviorTreeLog` → `TreeNodeLog` → `SubTree` → `Box<BehaviorTreeLog>`

### Pattern Analysis

The cycles arise because:

- **Union types** can contain variants that reference back to ancestor types
- **Recursive containment** returns to the same type or a parent type
- **Multi-level inheritance hierarchies** create indirect cycles through super classes

---

## Concrete Algorithm: Cycle-Breaking Box Placement

### Phase 1: Build the Type Dependency Graph

```
Input: Ecore model (classes, references, unions, inheritance)
Output: Directed graph G = (V, E)

Algorithm BUILD_DEPENDENCY_GRAPH:
  
  V ← all classes (concrete and abstract)
  E ← empty set
  
  for each class C in V:
    for each containment reference R in C:
      target ← R.eType (the type contained)
      
      // Add direct edge
      add edge (C, target) to E
      label edge with (R.name, R.lowerBound, R.upperBound)
      
      // For union types, add edges to all variants
      if target is abstract/interface:
        for each concrete subtype S of target:
          add edge (C, S) to E
          label edge with (R.name, R.lowerBound, R.upperBound, "via_union")
      
      // For inheritance, add edge to parent
      if C has superclass P:
        add edge (C, P) to E
        label edge with ("inheritance", inherited_features)

  return G
```

### Phase 2: Detect Minimal Feedback Arc Set (Cycles)

```
Algorithm FIND_FEEDBACK_ARC_SET:
  
  Input: Graph G = (V, E)
  Output: Set of edges F ⊆ E (minimal set whose removal makes G acyclic)
  
  // Simple approximation for practical code generation:
  
  F ← empty set
  visited ← empty set
  rec_stack ← empty set  // recursive call stack
  
  for each vertex v in V:
    if v not in visited:
      DFS_VISIT(v, visited, rec_stack, G, F)
  
  return F


Algorithm DFS_VISIT(vertex v, visited, rec_stack, graph G, feedback_set F):
  
  visited.add(v)
  rec_stack.add(v)
  
  for each edge (v, u) in G.outgoing_edges(v):
    if u not in visited:
      DFS_VISIT(u, visited, rec_stack, G, F)
    else if u in rec_stack:
      // Cycle detected: v → ... → u → v
      add edge (v, u) to F
  
  rec_stack.remove(v)
```

### Phase 3: Analyze Edge Context to Determine Box Placement Strategy

```
Algorithm DETERMINE_BOX_STRATEGY:
  
  Input: Graph G, Feedback arc set F, Edge context information
  Output: BoxPlacement = Map<Edge, Boolean>
  
  BoxPlacement ← empty map
  
  for each edge e = (source, target) in F:
    should_box ← FALSE
    
    // Check 1: Is this edge in a UNION VARIANT path?
    // Union variants that can recursively contain their parent should box
    if e.label contains "via_union":
      should_box ← TRUE
      reason ← "Union variant can recursively reference parent"
    
    // Check 2: Is this a CARDINALITY issue?
    // Many-to-many (collection) relationships should box the element type
    if e.lowerBound is MANY:
      should_box ← TRUE
      reason ← "Collection element type in recursive reference"
    
    // Check 3: DIRECT RECURSION through inherited fields?
    // If boxing the parent super-class would suffice, prefer that
    if e.target has superclass and NOT e.label contains "inheritance":
      // Might be able to box at inheritance level instead
      should_box ← MAYBE_BOX_AT_PARENT
    
    // Check 4: Position in hierarchy - prefer early breaks
    // If both parent and child could be boxed, box at parent level
    if source is superclass of target OR target is abstract:
      should_box ← PREFER_PARENT_BOX
    
    BoxPlacement[e] ← (should_box, reason)
  
  return BoxPlacement
```

### Phase 4: Apply Boxing to Type Definitions

```
Algorithm APPLY_BOX_WRAPPING:
  
  Input: Type definitions T, BoxPlacement decisions
  Output: Modified type definitions with Box<...> applied
  
  for each class C in T:
    for each field F in C:
      field_type ← F.type
      
      // Check if field's reference edge needs boxing
      source_edge ← (C, F.eType)
      
      if source_edge in BoxPlacement and BoxPlacement[source_edge]:
        if F.isMany:
          // For collections: Box<T> inside the container
          F.type ← ListLog<Box<field_type>>  // or OptionLog<Box<...>>
        else:
          // For single references
          F.type ← Box<field_type>
      
      // Recursively check parent classes
      if source_edge in BoxPlacement and BoxPlacement[source_edge].reason == "inheritance":
        propagate_box_to_parent(C.parent, field_type)
  
  return modified T
```

---

## Optimization: Minimize Box Count

The algorithm above may over-box. Apply these heuristics to reduce:

### Heuristic 1: Union Variant Consolidation

```
If a union has multiple variants that would need boxing, consider:
- Box the union type itself rather than individual variants
- Example: Instead of Union = A(BoxedType) | B(BoxedType)
           Use: Union = Box<(A | B)>
```

### Heuristic 2: Inheritance-Level Boxing

```
If all subclasses of an abstract class would need boxing in the same position,
box at the inheritance level (Feat strut) instead of multiple subclass definitions.

Example: 
  ❌ Bad: DecoratorFeat has child: Box<TreeNodeLog>
  ✓ Good: TreeNodeFeatBase has child: Box<TreeNodeLog> 
          All subclasses inherit it
```

### Heuristic 3: Cardinality-Driven Boxing

```
For collection types (Many references):
- ALWAYS box the ELEMENT type, NOT the collection itself
  
  ❌ Bad:  Box<ListLog<TreeNodeLog>>
  ✓ Good: ListLog<Box<TreeNodeLog>>
  
This allows the collection to grow without size constraints.
```

### Heuristic 4: Prefer "Leaf-Level" Boxing

```
In the inheritance hierarchy, prefer boxing at more specific types:

  ❌ Box abstract/interface level      (affects all subclasses)
  ✓ Box concrete/leaf subclass level   (only needed variants)
  
Exception: If ALL subclasses need it, box at parent.
```

---

## Algorithm Pseudocode Summary (Simplified Version)

```pseudocode
FUNCTION DetectAndApplyBoxing(ecore_model):
  
  // Phase 1: Build type dependency graph
  graph ← BuildDependencyGraph(ecore_model)
  
  // Phase 2: Find cycles
  cycles ← FindAllElementaryCycles(graph)
  
  // Phase 3: Determine which edge in each cycle to break
  edges_to_box ← {}
  
  FOR EACH cycle IN cycles:
    // For each cycle, select ONE edge to apply Box
    // Selection criteria (in priority order):
    
    1. UNION VARIANT edges (prefer breaking union recursive references)
    2. COLLECTION element references (box the element type)
    3. INDIRECT references (prefer over direct self-references)
    4. LEAF-LEVEL references (in deep hierarchies, prefer leaf over root)
    
    edge_to_break ← SelectBestEdgeToBreak(cycle, ecore_model)
    edges_to_box.add(edge_to_break)
  
  // Phase 4: Apply Boxing
  FOREACH edge IN edges_to_box:
    source_class ← edge.source
    field_name ← edge.field
    target_type ← edge.target
    cardinality ← edge.cardinality
    
    IF cardinality == MANY:
      source_class[field_name].type ← ListLog<Box<target_type>>
    ELSE:
      source_class[field_name].type ← Box<target_type>
  
  RETURN modified_ecore_model
```

---

## Implementation Considerations for Atraktos

### 1. Integration Point

- Modify the `ReferenceGenerator::generate()` function
- Check if the reference edge is marked for boxing in the feedback arc set
- Wrap the field type with `Box<...>` when appropriate

### 2. Cycle Detection Approach

For Rust code generation, use a **lightweight topological sort**:

```rust
// Pseudocode
fn detect_cycles_and_box_needs(classes: &[Class]) -> HashMap<(ClassId, FieldName), bool> {
    let mut graph = build_containment_graph(classes);
    let mut needs_box = HashMap::new();
    
    // Tarjan's algorithm to find strongly connected components
    let sccs = tarjan_scc(&graph);
    
    for scc in sccs {
        if scc.len() > 1 {  // Cycle exists
            // Find minimum edges to remove
            let feedback_edges = find_minimum_feedback_edges(&scc, &graph);
            
            for (source, target) in feedback_edges {
                needs_box.insert((source, target), true);
            }
        }
    }
    
    needs_box
}
```

### 3. Data Structure

```rust
pub struct CycleAnalysis {
    /// Maps (source_class, reference_field) → needs_box
    pub boxing_requirements: HashMap<(ClassId, String), BoxingContext>,
}

pub enum BoxingContext {
    /// Box single reference
    DirectReference { target: ClassId },
    /// Box elements within a collection (List/Option)
    CollectionElement { target: ClassId, cardinality: Cardinality },
    /// Box entire union variant
    UnionVariant { union_type: ClassId },
}
```

### 4. Performance

- Run cycle detection once at model load time
- Cache results in the `Ctx` struct
- `ReferenceGenerator::generate()` queries the cache:

  ```rust
  let needs_boxing = ctx.cycle_analysis().needs_box(source, field_name);
  ```

---

## Expected Impact on Generated Code

| Metric | Before | After |
|--------|--------|-------|
| Total `Box` count | Manual (variable) | Minimal, optimal |
| Compilation time | Same | Same (boxing is zero-cost) |
| Runtime overhead | None | None (Box is compile-time only) |
| Code clarity | Must reason about cycles | Auto-detected, fully documented |
