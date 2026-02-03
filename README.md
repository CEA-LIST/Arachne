# Atraktos

**Atraktos** is a Rust-based code generator that compile Domain-Specific Modeling Language (DSML) designed using Ecore metamodels to Conflict-free Replicated Data Types (CRDT) using the Moirai library.

## Customizing the code generator mapping

The Ecore metamodeling language allows annotating model elements with `EAnnotation`s. A language engineer can use them to give hints to the code generator on the kind of replicated data type it wants to be used for specific model elements.

### Specifying a particular data type

```xml
<eAnnotations source="urn:atraktos:semantics">
    <details key="datatype" value="lww-register"/>
</eAnnotations>
```

### Specifying a total or partial order among the litterals of an `EEnum`

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

## Mapping

### Primitive Data Types

|Ecore|CRDT|
|-----|----|
|`EByte`|`Counter<i8>` or any `Register`|
|`EShort|`Counter<i16>` or any `Register`|
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
|`EClass`|See [`Eclass`](#eclass)|
|`EEnum`|Rust enum + any `Register`|

#### `EClass`

### Bounds
