# Examples

## Running the examples

```sh
RUST_LOG=debug cargo run generate -vv -o <WHERE_TO_GENERATE_PROJECT> <PATH_TO_ECORE_METAMODEL>
```

## Case studies

The `example` folder contains three metamodel case studies that demonstrate the capabilities of the code generator: Class Hierarchy, Behavior Tree, and JSON.

| DSML            | Domain                             | # Model elements                                      | Characteristics                                  |
| --------------- | ---------------------------------- | ----------------------------------------------------- | ------------------------------------------------ |
| Behavior Tree   | Task planning<br>for robotics      | 22 classes (6 abstract),<br>16 features (1 reference) | Inheritance, references,<br>enumerations         |
| Class Hierarchy | Structural modeling<br>of a system | 8 classes (3 abstract),<br>12 features (4 references) | Inheritance, bounds,<br>self-references          |
| JSON            | Structured data<br>exchange        | 7 classes (1 abstract),<br>7 features                 | Inheritance, recursive<br>structure, annotations |

_Table: Overview of the DSML case studies used._

### Metamodel diagrams

#### Behavior Tree

![Behavior Tree](../images/behavior_tree.jpg)

#### Class Hierarchy

![Class Hierarchy](../images/class_hierarchy.png)

#### JSON

![JSON](../images/json.png)

## Pet metamodels

We also provide a set of "pet" metamodels ([/pet_metamodels](./pet_metamodels/)) that highlight specific supported features of the code generator.

| Pet                                                                                       | Highlighted feature(s)                                                 |
| ----------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| [abstract_inherits_concrete.ecore](./pet_metamodels/abstract_inherits_concrete.ecore)     | Abstract class inherits from concrete class                            |
| [concrete_inherits_concrete.ecore](./pet_metamodels/concrete_inherits_concrete.ecore)     | Concrete class inherits from concrete class                            |
| [concrete_polymorphic_targets.ecore](./pet_metamodels/concrete_polymorphic_targets.ecore) | References targeting concrete superclass implementations               |
| [kitchen_sink.ecore](./pet_metamodels/kitchen_sink.ecore)                                 | EDataTypes, bounds, collection semantics, references, abstract classes |
| [multiple_inheritance.ecore](./pet_metamodels/multiple_inheritance.ecore)                 | Multiple inheritance from abstract classes                             |
