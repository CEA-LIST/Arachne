# ecore.rs

A parser and internal representation for the [Ecore format][ecore] used by the Eclipse Modeling
Framework (EMF).

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
supported flags use the existing context and representation. Unsupported
`unsettable` and `eOpposite` settings still produce warnings. Generics are
not supported; generation has its own
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
