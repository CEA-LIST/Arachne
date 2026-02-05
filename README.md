# Atraktos

**Atraktos** is a Rust-based code generator that compiles Domain-Specific Modeling Languages (DSML) designed using Ecore metamodels to Conflict-free Replicated Data Types (CRDT) using the Moirai library.

## Overview

This code generator bridges the gap between high-level domain models (defined in Ecore) and distributed, eventually consistent data structures (CRDTs). It automatically generates Rust code that leverages the Moirai library to provide conflict-free replication semantics for your domain models.

## Management of References

An important challenge in generating code from a metamodel into a composition of CRDTs is the management of references. The approach to CRDT composition and nesting proposed by *Bauwens et al.* is hierarchical: a parent CRDT can propagate its conflict-resolution policy to its children using a causal reset. However, references represent relationships between siblings in the hierarchy. To support them, we adopt the following design:

- Each class instance is assigned a unique identifier and stored in a `UWMap<ID, Object>` within its containing package.
- An auxiliary, specialized *typed graph CRDT*, called the `ReferenceManager`, is responsible for registering references between classifiers. This CRDT encodes which classes may reference which other classes, together with the associated multiplicity constraints (in particular, upper bounds). When interpreting the state of the model, elements are first evaluated independently; the links between them are then established by reading and applying the state of the `ReferenceManager`.

## Customizing the Code Generator Mapping

The Ecore metamodeling language allows annotating model elements with `EAnnotation`s. A language engineer can use them to give hints to the code generator on the kind of replicated data type it wants to be used for specific model elements.

### Specifying a Particular Data Type

```xml
<eAnnotations source="urn:atraktos:semantics">
    <details key="datatype" value="lww-register"/>
</eAnnotations>
```

### Specifying a Total or Partial Order Among the Literals of an `EEnum`

```xml
<eClassifiers xsi:type="ecore:EEnum" name="name">
    <eAnnotations source="urn:atraktos:order">
        <details key="order" value="partial-order"/>
    </eAnnotations>
    <eLiterals name="ADD">
        <eAnnotations source="urn:atraktos:order">
            <details key="rank" value="1"/>
        </eAnnotations>
    </eLiterals>
    <eLiterals name="UPDATE">
        <eAnnotations source="urn:atraktos:order">
            <details key="rank" value="1"/>
        </eAnnotations>
    </eLiterals>
    <eLiterals name="REMOVE">
        <eAnnotations source="urn:atraktos:order">
            <details key="rank" value="2"/>
        </eAnnotations>
    </eLiterals>
</eClassifiers>
```

## Mapping Reference

For detailed Ecore documentation, see: [Ecore API Documentation](https://download.eclipse.org/modeling/emf/emf/javadoc/2.9.0/org/eclipse/emf/ecore/package-summary.html#details)

### Primitive Data Types

|Ecore|CRDT|
|-----|----|
|`EByte`|`Counter<i8>` or any `Register`|
|`EShort`|`Counter<i16>` or any `Register`|
|`EInt`|`Counter<i32>` or any `Register`|
|`ELong`|`Counter<i64>` or any `Register`|
|`EFloat`|`Counter<f32>` or any `Register`|
|`EDouble`|`Counter<f64>` or any `Register`|
|`EBoolean`|`Enable-Wins Flag`, `Disable-Wins Flag`, or any `Register`|
|`EChar`|Any `Register`|
|`EString`|`List<String>` or any `Register`|

### Classifiers

|Ecore|CRDT|
|-----|----|
|`EDataType`|See [Primitive Data Types](#primitive-data-types)|
|`EClass`|See [`EClass`](#eclass)|
|`EEnum`|Rust enum (see [Customizing](#specifying-a-total-or-partial-order-among-the-literals-of-an-eenum)) + any `Register`|

#### `EClass`

A (concrete) `EClass` is generated as a `record`.

##### Abstract class

For abstract classes, the code generator does not produce an abstract type in the target language. Instead, it replaces abstract classes with a closed `union` type that represents all their concrete subclasses. This union type preserves subtyping by allowing any concrete subclass to be used wherever the abstract type is expected. Features defined in the abstract class are not inherited at runtime but are statically flattened into each concrete subclass during generation, so that each generated class explicitly contains all required properties.

This approach eliminates runtime inheritance while preserving substitutability and structural reuse, and it is well suited to a closed-world setting where all concrete variants are known at generation time.

##### Interface

Not supported. See [Operations](#operations).

### Structural features

- `changeable`: if `false`, then the feature is immutable. Could be implemented by generating the Rust struct of an object rather than using a CRDT.
- `volatile`: if `true`, the value is computed on access and never stored. Not supported.
- `derived`: if `true`, the value is computed from other features. Not supported.
- `transient`: if `true`, the value is not serialized. Not supported.
- `unsettable`: if `true`, the feature distinguish between "unset" and "set to default value". May be implemented depending on the CRDT.
- `defaultValue`: the feature has a default value. Could be implemented.

- `ordered`: if `true`, the feature is ordered (i.e., is a list). Implemented as a List.
- `unique`: every element is unique (i.e., is a set). Implemented as a Set if it is a primitive data type.
- Case `ordered` and `unique`: need for a `OrderedSet` CRDT.
- `lowerbound` and `upperbound`: see [Bounds](#bounds).

#### Reference

A reference from an object A to another object B is materialized by an arc between the identifier of these objects in the `ReferenceManager`.

- `containment`: the reference owns the referenced `EClass`. The generated object has a field owning the referenced object.
- `container`: the reference points to the parent `EClass`. Not supported.

#### Attribute

- `iD`: the attribute uniquely identifies instances of this class. Could be supported.

#### Bounds

|Ecore|CRDT|
|-----|----|
|`0..1`|`Option<T>`|
|`1`|`T`|
|`0..*`|`List<T>`|
|`1..*`|?|
|`1..n`|?|
|`n..*`|?|
|`n..m`|?|

### Operations

The code generator intentionally does not support Ecore operations nor interfaces at this stage. This decision is primarily motivated by the semantic constraints of conflict-free replicated data types (CRDTs) and by limitations of the Ecore metamodel.

- First, the implementation of operations is not specified in Ecore, which means a code generator cannot automatically derive their semantics in a meaningful or correct way. Generating operation signatures without being able to generate their behavior would therefore provide little practical value.
- Second, Ecore does not distinguish between *pure queries* (side-effect free operations that return a value) and *updates* (operations with side effects). This distinction is essential in the context of CRDTs, since the underlying CRDT runtime only supports pure operations and a fixed, explicit set of update operations. Allowing users to implement operations manually would risk introducing side effects or updates that are incompatible with the CRDT's convergence guarantees.
- Third, the CRDT interface exposes a closed set of available updates. Extending this set is not a local change: it typically requires extending the CRDT's semantics. Supporting such extensions in generated code would therefore be complex and error-prone, and is precisely one of the reasons for developing a specialized code generator in the first place.
- While adding new pure queries is conceptually simpler, queries in CRDTs are evaluated over a partially ordered set of updates to compute a deterministic value. For the same reasons as above, the generator should not require users to manually implement queries whose correctness depends on the CRDT's semantics.

An exception is made for queries on values derived from the CRDT state. For example, a `read()` operation may project the CRDT state into a deterministic value (such as a Behavior Tree), and pure query operations can then be defined on this projected value. Since these queries operate on a stable, materialized representation and do not affect the CRDT's semantics, they can be supported safely.

### Package

An `EPackage` is supported as an object holding all the generated collaborative metamodel. Interaction between packages is not currently supported.

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

See the LICENSE file for details.
