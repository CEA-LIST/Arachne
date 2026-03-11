# Stratégie de génération des références non-containment

## Contexte

Les CRDTs générés ne peuvent pas directement représenter des références entre objets.
Le fichier `model.rs` montre une implémentation manuelle utilisant le pattern `typed_graph!` pour gérer ces références.
Ce document décrit comment généraliser ce pattern dans le générateur de code.

## Analyse du pattern manuel

### Exemple : bt.ecore

La référence non-containment dans le métamodèle :

```xml
<eClassifiers xsi:type="ecore:EClass" name="DataFlowPort" abstract="true">
    <eStructuralFeatures xsi:type="ecore:EReference" name="entry" eType="#//BlackboardEntry" />
</eClassifiers>
```

Ici, `DataFlowPort.entry → BlackboardEntry` est une référence non-containment avec bounds `[0, 1]`.

### Éléments générés manuellement

1. **Structs ID pour les classes référençables** :

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryId(pub EventId);        // Pour BlackboardEntry

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataFlowPortId(pub EventId); // Pour DataFlowPort
```

1. **Struct pour l'arête** :

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryEdge;
```

1. **Macro typed_graph!** :

```rust
typed_graph! {
    graph: ReferenceManager,
    vertex: Instance,
    edge: Ref,
    arcs_type: Refs,
    vertices { EntryId, DataFlowPortId },
    connections {
        Entry: DataFlowPortId -> EntryId (EntryEdge) [0, 1],
    }
}
```

1. **Enum Model** :

```rust
pub enum Model {
    Root(Root),
    Reference(Refs),
}
```

1. **ModelLog avec IsLog** :

- Contient `root_log` et `reference_manager_log`
- Intercepte les opérations créant/supprimant des objets référençables
- Crée/supprime les sommets correspondants dans le graphe

---

## Stratégie de généralisation

### Phase 1 : Analyse des références

**Entrée** : `Ctx` (contexte Ecore parsé)

**Sortie** : Structure `ReferenceAnalysis`

```rust
struct NonContainmentRef {
    source_class: Class,           // Classe contenant la référence
    target_class: Class,           // Classe référencée
    reference_name: String,        // Nom de la référence
    bounds: (usize, usize),        // Cardinalité [lower, upper]
}

struct ReferenceAnalysis {
    /// Toutes les références non-containment
    non_containment_refs: Vec<NonContainmentRef>,
    
    /// Classes qui sont sources d'au moins une référence non-containment
    source_classes: HashSet<Class>,
    
    /// Classes qui sont cibles d'au moins une référence non-containment  
    target_classes: HashSet<Class>,
    
    /// Classes référençables = source_classes ∪ target_classes
    referenceable_classes: HashSet<Class>,
    
    /// Pour chaque classe abstraite, ses sous-classes concrètes
    concrete_subclasses: HashMap<Class, Vec<Class>>,
}
```

**Algorithme** :

```rust
fn analyze_references(ctx: &Ctx) -> ReferenceAnalysis {
    let mut analysis = ReferenceAnalysis::default();
    
    for class in ctx.classes() {
        for feature in class.structural() {
            if feature.kind == EReference && !feature.containment {
                let ref_info = NonContainmentRef {
                    source_class: class.idx,
                    target_class: feature.typ.unwrap(),
                    reference_name: feature.name.clone(),
                    bounds: feature.bounds,
                };
                
                analysis.non_containment_refs.push(ref_info);
                analysis.source_classes.insert(class.idx);
                analysis.target_classes.insert(feature.typ.unwrap());
            }
        }
    }
    
    analysis.referenceable_classes = 
        analysis.source_classes.union(&analysis.target_classes).collect();
    
    // Calculer les sous-classes concrètes pour chaque classe abstraite
    for class in &analysis.referenceable_classes {
        if ctx.classes()[*class].is_abstract() {
            analysis.concrete_subclasses.insert(
                *class,
                find_concrete_subclasses(ctx, *class)
            );
        }
    }
    
    analysis
}
```

### Phase 2 : Génération des structs ID

Pour chaque classe dans `referenceable_classes` :

```rust
fn generate_id_structs(analysis: &ReferenceAnalysis, ctx: &Ctx) -> TokenStream {
    let mut structs = vec![];
    
    for class_idx in &analysis.referenceable_classes {
        let class = &ctx.classes()[*class_idx];
        let id_name = format_ident!("{}Id", class.name());
        
        structs.push(quote! {
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub struct #id_name(pub EventId);
        });
    }
    
    quote! { #(#structs)* }
}
```

### Phase 3 : Génération des structs Edge

Pour chaque référence non-containment :

```rust
fn generate_edge_structs(analysis: &ReferenceAnalysis) -> TokenStream {
    let mut edges = vec![];
    
    for ref_info in &analysis.non_containment_refs {
        let edge_name = format_ident!("{}Edge", ref_info.reference_name.to_pascal_case());
        
        edges.push(quote! {
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub struct #edge_name;
        });
    }
    
    quote! { #(#edges)* }
}
```

### Phase 4 : Génération de la macro typed_graph

```rust
fn generate_typed_graph(analysis: &ReferenceAnalysis, ctx: &Ctx) -> TokenStream {
    // Générer la liste des vertices
    let vertices: Vec<_> = analysis.referenceable_classes
        .iter()
        .map(|c| format_ident!("{}Id", ctx.classes()[*c].name()))
        .collect();
    
    // Générer les connexions
    let connections: Vec<_> = analysis.non_containment_refs
        .iter()
        .map(|r| {
            let conn_name = format_ident!("{}", r.reference_name.to_pascal_case());
            let source_id = format_ident!("{}Id", ctx.classes()[r.source_class].name());
            let target_id = format_ident!("{}Id", ctx.classes()[r.target_class].name());
            let edge_name = format_ident!("{}Edge", r.reference_name.to_pascal_case());
            let (lower, upper) = r.bounds;
            let upper_str = if upper == usize::MAX { "*".to_string() } else { upper.to_string() };
            
            quote! {
                #conn_name: #source_id -> #target_id (#edge_name) [#lower, #upper_str]
            }
        })
        .collect();
    
    quote! {
        typed_graph! {
            graph: ReferenceManager,
            vertex: Instance,
            edge: Ref,
            arcs_type: Refs,
            vertices { #(#vertices),* },
            connections {
                #(#connections),*
            }
        }
    }
}
```

### Phase 5 : Génération de l'enum Model

```rust
fn generate_model_enum(root_name: &str) -> TokenStream {
    let root_ident = format_ident!("{}", root_name);
    
    quote! {
        #[derive(Debug, Clone)]
        pub enum Model {
            Root(#root_ident),
            Reference(Refs),
        }
    }
}
```

### Phase 6 : Génération du ModelLog et IsLog

C'est la partie la plus complexe car elle nécessite de :

1. Trouver les chemins de containment depuis la racine jusqu'à chaque classe référençable
2. Générer les pattern matches correspondants

#### 6.1 Analyse des chemins de containment

```rust
struct ContainmentPath {
    /// Chemin depuis la racine jusqu'à la classe cible
    /// Chaque élément = (classe, nom_feature, est_liste)
    segments: Vec<(Class, String, bool)>,
    target_class: Class,
}

fn find_containment_paths(
    ctx: &Ctx, 
    root_class: Class, 
    target_class: Class
) -> Vec<ContainmentPath> {
    // DFS pour trouver tous les chemins de containment
    // Considérer les classes abstraites (unions) comme des embranchements
}
```

#### 6.2 Génération des patterns pour création d'objets

Pour chaque chemin de containment, générer le pattern match correspondant :

**Exemple pour `BlackboardEntry`** :

```
Root → main (BehaviorTree) → blackboard (Blackboard) → entries (List<BlackboardEntry>)
```

Pattern généré :

```rust
Model::Root(Root::Main(BehaviorTree::Blackboard(Blackboard::Entries(
    List::Insert { .. },
)))) => {
    let entry_id = BlackboardEntryId(event.id().clone());
    let new_vertex = ReferenceManager::<LwwPolicy>::AddVertex {
        id: Instance::BlackboardEntryId(entry_id),
    };
    self.reference_manager_log.effect(Event::unfold(event.clone(), new_vertex));
}
```

**Exemple pour `DataFlowPort` (via InFlowPort/OutFlowPort)** :

Chemins possibles via les sous-classes concrètes d'`ExecutionNode` :

```
Root → main → child (TreeNode) → ExecutionNode → Action → OpenDoor → ActionFeat → ExecutionNodeFeat → inflowports
Root → main → child (TreeNode) → ExecutionNode → Action → OpenDoor → ActionFeat → ExecutionNodeFeat → outflowports
... (pour chaque Action/Condition concrète)
```

La fonction `created_vertex_from_execution_node` dans model.rs gère cela en matchant sur le type d'ExecutionNode.

#### 6.3 Stratégie pour gérer les hiérarchies profondes

Pour éviter une explosion combinatoire, extraire des fonctions helper :

```rust
fn generate_vertex_creation_handlers(
    analysis: &ReferenceAnalysis,
    ctx: &Ctx,
    root_class: Class,
) -> TokenStream {
    let mut handlers = vec![];
    
    for target_class in &analysis.referenceable_classes {
        let paths = find_containment_paths(ctx, root_class, *target_class);
        let class_name = ctx.classes()[*target_class].name();
        
        // Si la classe a plusieurs chemins via une classe abstraite commune,
        // générer une fonction helper
        if paths_share_common_abstract_ancestor(&paths) {
            handlers.push(generate_helper_function(target_class, paths));
        } else {
            handlers.push(generate_direct_match(target_class, paths));
        }
    }
    
    quote! { #(#handlers)* }
}
```

### Phase 7 : Génération de IsLog::effect()

```rust
fn generate_is_log_effect(
    analysis: &ReferenceAnalysis,
    ctx: &Ctx,
) -> TokenStream {
    let vertex_creation_patterns = generate_creation_patterns(analysis, ctx);
    let vertex_deletion_patterns = generate_deletion_patterns(analysis, ctx);
    
    quote! {
        fn effect(&mut self, event: Event<Self::Op>) {
            // Interception des créations
            match &event.op() {
                #(#vertex_creation_patterns)*
                _ => {}
            }
            
            // Interception des suppressions
            match &event.op() {
                #(#vertex_deletion_patterns)*
                _ => {}
            }
            
            // Application de l'opération sous-jacente
            match event.op().clone() {
                Model::Root(root) => self.root_log.effect(Event::unfold(event, root)),
                Model::Reference(refs) => self.reference_manager_log
                    .effect(Event::unfold(event, ReferenceManager::AddArc(refs))),
            }
        }
    }
}
```

---

## Structure des nouveaux fichiers du générateur

### Nouveaux modules à créer

```
src/codegen/
├── reference/
│   ├── mod.rs              # Module principal
│   ├── analysis.rs         # Phase 1: Analyse des références
│   ├── id_struct.rs        # Phase 2: Génération des structs ID
│   ├── edge_struct.rs      # Phase 3: Génération des structs Edge
│   ├── typed_graph.rs      # Phase 4: Génération de typed_graph!
│   ├── model_enum.rs       # Phase 5: Génération de l'enum Model
│   ├── model_log.rs        # Phase 6-7: Génération de ModelLog et IsLog
│   └── containment.rs      # Analyse des chemins de containment
```

### Interface principale

```rust
// src/codegen/reference/mod.rs
pub struct ReferenceGenerator<'a> {
    ctx: &'a Ctx,
    analysis: ReferenceAnalysis,
    root_class: Class,
}

impl<'a> ReferenceGenerator<'a> {
    pub fn new(ctx: &'a Ctx, root_class: Class) -> Self {
        let analysis = analyze_references(ctx);
        Self { ctx, analysis, root_class }
    }
    
    pub fn generate(&self) -> Fragment {
        if self.analysis.non_containment_refs.is_empty() {
            return Fragment::empty();
        }
        
        let id_structs = self.generate_id_structs();
        let edge_structs = self.generate_edge_structs();
        let typed_graph = self.generate_typed_graph();
        let model_enum = self.generate_model_enum();
        let model_log = self.generate_model_log();
        
        Fragment::combine(vec![
            id_structs,
            edge_structs,
            typed_graph,
            model_enum,
            model_log,
        ])
    }
}
```

---

## Cas particuliers à gérer

### 1. Classes abstraites comme source/cible

Quand une classe abstraite est source ou cible, il faut considérer ses sous-classes concrètes :

```rust
// Pour DataFlowPort (abstrait) avec sous-classes InFlowPort, OutFlowPort
// On doit intercepter la création de InFlowPort ET OutFlowPort
```

### 2. Références multiples vers la même classe

Si plusieurs références pointent vers la même classe cible :

```xml
<eReference name="parent" eType="#//Node"/>
<eReference name="children" upperBound="-1" eType="#//Node"/>
```

Un seul struct ID suffit (`NodeId`), mais plusieurs Edge types.

### 3. Références bidirectionnelles

Si deux classes se référencent mutuellement :

```xml
<eClass name="A">
    <eReference name="toB" eType="#//B"/>
</eClass>
<eClass name="B">
    <eReference name="toA" eType="#//A"/>
</eClass>
```

Les deux classes sont à la fois sources et cibles.

### 4. Auto-références

Une classe qui se référence elle-même :

```xml
<eClass name="Node">
    <eReference name="next" eType="#//Node"/>
</eClass>
```

Le struct ID est le même pour source et cible.

### 5. Suppression d'objets référencés

Quand un objet référencé est supprimé (`List::Delete`), il faut :

1. Retrouver l'EventId de l'objet supprimé via la position
2. Supprimer le sommet correspondant du graphe

---

## Exemple de code généré complet (pour bt.ecore)

```rust
use moirai_crdt::{list::nested_list::List, policy::LwwPolicy};
use moirai_macros::typed_graph;
use moirai_protocol::{
    clock::version_vector::Version,
    crdt::{eval::EvalNested, pure_crdt::PureCRDT, query::{QueryOperation, Read}},
    event::{Event, id::EventId},
    state::{log::IsLog, po_log::VecLog},
};

use crate::generated::{
    Action, ActionFeat, BehaviorTree, Blackboard, Condition, ConditionFeat,
    ExecutionNode, ExecutionNodeFeat, Root, RootLog, RootValue, TreeNode,
    // ... autres imports
};

// Phase 2: ID structs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlackboardEntryId(pub EventId);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataFlowPortId(pub EventId);

// Phase 3: Edge structs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryEdge;

// Phase 4: typed_graph!
typed_graph! {
    graph: ReferenceManager,
    vertex: Instance,
    edge: Ref,
    arcs_type: Refs,
    vertices { BlackboardEntryId, DataFlowPortId },
    connections {
        Entry: DataFlowPortId -> BlackboardEntryId (EntryEdge) [0, 1],
    }
}

// Phase 5: Model enum
#[derive(Debug, Clone)]
pub enum Model {
    Root(Root),
    Reference(Refs),
}

// Phase 6-7: ModelLog et IsLog
#[derive(Debug, Clone, Default)]
pub struct ModelLog {
    pub root_log: RootLog,
    pub reference_manager_log: VecLog<ReferenceManager<LwwPolicy>>,
}

impl IsLog for ModelLog {
    type Value = (RootValue, <ReferenceManager<LwwPolicy> as PureCRDT>::Value);
    type Op = Model;

    fn is_enabled(&self, op: &Self::Op) -> bool {
        match op {
            Model::Root(o) => self.root_log.is_enabled(o),
            Model::Reference(o) => self.reference_manager_log
                .is_enabled(&ReferenceManager::AddArc(o.clone())),
        }
    }

    fn effect(&mut self, event: Event<Self::Op>) {
        // Création de BlackboardEntry
        if let Model::Root(Root::Main(BehaviorTree::Blackboard(
            Blackboard::Entries(List::Insert { .. })
        ))) = &event.op() {
            let entry_id = BlackboardEntryId(event.id().clone());
            self.reference_manager_log.effect(Event::unfold(
                event.clone(),
                ReferenceManager::<LwwPolicy>::AddVertex {
                    id: Instance::BlackboardEntryId(entry_id),
                },
            ));
        }
        
        // Suppression de BlackboardEntry
        if let Model::Root(Root::Main(BehaviorTree::Blackboard(
            Blackboard::Entries(List::Delete { pos })
        ))) = &event.op() {
            let positions = self.root_log.main.blackboard.entries.position.execute_query(Read::new());
            let event_id = positions[*pos].clone();
            self.reference_manager_log.effect(Event::unfold(
                event.clone(),
                ReferenceManager::<LwwPolicy>::RemoveVertex {
                    id: Instance::BlackboardEntryId(BlackboardEntryId(event_id)),
                },
            ));
        }
        
        // Création de DataFlowPort (via sous-classes)
        if let Model::Root(Root::Main(BehaviorTree::Child(child))) = &event.op() {
            if let Some(new_vertex) = created_vertex_from_tree_node(event.id(), child.as_ref()) {
                self.reference_manager_log.effect(Event::unfold(event.clone(), new_vertex));
            }
        }

        // Application de l'opération
        match event.op().clone() {
            Model::Root(root) => self.root_log.effect(Event::unfold(event, root)),
            Model::Reference(refs) => self.reference_manager_log
                .effect(Event::unfold(event, ReferenceManager::AddArc(refs))),
        }
    }

    fn stabilize(&mut self, version: &Version) {
        self.root_log.stabilize(version);
        self.reference_manager_log.stabilize(version);
    }

    fn redundant_by_parent(&mut self, version: &Version, conservative: bool) {
        self.root_log.redundant_by_parent(version, conservative);
        self.reference_manager_log.redundant_by_parent(version, conservative);
    }

    fn is_default(&self) -> bool {
        self.root_log.is_default()
    }
}

// Helper généré pour gérer les différentes variantes d'ExecutionNode
fn created_vertex_from_tree_node(
    event_id: &EventId,
    node: &TreeNode,
) -> Option<ReferenceManager<LwwPolicy>> {
    match node {
        TreeNode::ExecutionNode(exec_node) => {
            created_vertex_from_execution_node(event_id, exec_node)
        }
        _ => None,
    }
}

fn created_vertex_from_execution_node(
    event_id: &EventId,
    execution_node: &ExecutionNode,
) -> Option<ReferenceManager<LwwPolicy>> {
    let is_flow_port_insert = matches!(
        execution_node,
        ExecutionNode::Action(Action::OpenDoor(OpenDoor::ActionFeat(
            ActionFeat::ExecutionNodeFeat(ExecutionNodeFeat::Inflowports(List::Insert { .. }))
        ))) | ExecutionNode::Action(Action::OpenDoor(OpenDoor::ActionFeat(
            ActionFeat::ExecutionNodeFeat(ExecutionNodeFeat::Outflowports(List::Insert { .. }))
        ))) | /* ... tous les autres cas ... */
    );

    if is_flow_port_insert {
        Some(ReferenceManager::<LwwPolicy>::AddVertex {
            id: Instance::DataFlowPortId(DataFlowPortId(event_id.clone())),
        })
    } else {
        None
    }
}

impl EvalNested<Read<<Self as IsLog>::Value>> for ModelLog {
    fn execute_query(
        &self,
        _q: Read<<Self as IsLog>::Value>,
    ) -> <Read<<Self as IsLog>::Value> as QueryOperation>::Response {
        (
            self.root_log.execute_query(Read::new()),
            self.reference_manager_log.execute_query(Read::new()),
        )
    }
}
```

---

## Prochaines étapes d'implémentation

1. [ ] Implémenter `ReferenceAnalysis` dans `src/codegen/reference/analysis.rs`
2. [ ] Implémenter l'analyse des chemins de containment
3. [ ] Implémenter les générateurs pour chaque phase
4. [ ] Intégrer dans le flux de génération principal
5. [ ] Tester avec bt.ecore et d'autres modèles
6. [ ] Gérer les cas particuliers (auto-références, références multiples, etc.)
