# ecore.rs

A parser and internal representation for the [Ecore format][ecore] used by the Eclipse Modeling
Framework (EMF).

## Origin, license and contributors

This crate is a modified fork of [OCamlPro/ecore.rs](https://github.com/OCamlPro/ecore.rs).
Upstream commit [`bbc6bfe2e7aa5481d2e4e0384116d7a4cbc70f7a`](https://github.com/OCamlPro/ecore.rs/commit/bbc6bfe2e7aa5481d2e4e0384116d7a4cbc70f7a)
declares `LGPL-3.0-or-later` in its `Cargo.toml`. That license applies to the
inherited code. CEA modifications are provided under Apache-2.0; these do not
replace the upstream license. The crate therefore records both sets of terms
as `LGPL-3.0-or-later AND Apache-2.0`.

## TODO

- [ ] investigate type parameters for classes
- [ ] investigate type parameters for operations
- [ ] builtin type constructors (`Vec`, `Option`, ...)?

## Input (XML) format

[`roxmltree`](https://docs.rs/roxmltree/0.21.1/roxmltree/) reads the XML document.
The [adapter](./src/parser.rs) maps its nodes into the existing Ecore `Ctx`
and finalizes forward references. XML syntax, namespaces, comments, processing
instructions and source positions are handled by the library.

A document has one `EPackage` root in the
`http://www.eclipse.org/emf/2002/Ecore` namespace. Namespace prefixes are
arbitrary; the adapter matches namespace URIs and resolves the QNames in
`xsi:type` and builtin `eType` attributes. The XML declaration and a final
newline are optional. Both quote styles and empty-element forms are accepted.

Attribute values are XML-decoded before they enter the representation.
For example, `List&lt;Signal&gt; &amp; ℕ` becomes `List<Signal> & ℕ`.
Annotation references retain their order as decoded, unresolved paths.
Malformed XML and unsupported Ecore attributes or elements return errors with
source positions.

The adapter supports packages and `eSubpackages`, classifiers, structural
features, operations and parameters, enum literals, and annotations with
details and references. Type links, inheritance links, bounds, defaults and
supported flags use the existing context and representation. `unsettable` is
retained for code generation, which skips unsupported features and reports its
warnings after generation. Unsupported `eOpposite` settings still produce parser
warnings. Generics are not supported; generation has its own
[supported subset](../arachne-codegen/README.md#supported-metalanguage).

```xml
<e:EPackage xmlns:e="http://www.eclipse.org/emf/2002/Ecore"
    xmlns:t="http://www.w3.org/2001/XMLSchema-instance" name="example">
    <eClassifiers name="Node" t:type="e:EClass">
        <eStructuralFeatures name="label" t:type="e:EAttribute"
            eType="e:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    </eClassifiers>
</e:EPackage>
```

More examples are available in the [rsc](./rsc) folder.

[ecore]: https://download.eclipse.org/modeling/emf/emf/javadoc/2.9.0/org/eclipse/emf/ecore/package-summary.html#details
