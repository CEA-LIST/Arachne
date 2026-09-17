/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_protocol::state::graph_log::GraphLog;
    pub use moirai_crdt::option::OptionLog;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_crdt::set::aw_set::AWSet;
    pub use moirai_macros::HashSet;
    pub use moirai_crdt::list::nested_list::NestedListLog;
    pub use moirai_protocol::state::log::BoxedLog;
    pub use moirai_crdt::flag::ew_flag::EWFlag;
}
__classifiers::union!(
    ModelElementKind = Classifier(ClassifierKind, ClassifierKindLog =>
    ClassifierKindValue) | Property(Property, PropertyLog => PropertyValue) |
    Association(Association, AssociationLog => AssociationValue)
);
__classifiers::record!(
    ModelElement { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, stereotype :
    __classifiers::VecLog < __classifiers::AWSet < std::string::String >> =>
    __classifiers::HashSet < std::string::String >, tagged_value :
    __classifiers::NestedListLog < TaggedValueLog > => Vec < TaggedValueValue >, }
);
__classifiers::union!(
    ClassifierKind = Package(PackageKind, PackageKindLog => PackageKindValue) |
    TType(TTypeKind, TTypeKindLog => TTypeKindValue)
);
__classifiers::record!(
    Classifier { model_element_super : ModelElementLog => ModelElementValue, }
);
__classifiers::union!(
    PackageableKind = Package(PackageKind, PackageKindLog => PackageKindValue) |
    TType(TTypeKind, TTypeKindLog => TTypeKindValue) | Association(Association,
    AssociationLog => AssociationValue)
);
__classifiers::record!(Packageable {});
__classifiers::record!(
    Package { classifier_super : ClassifierLog => ClassifierValue, packageable_super :
    PackageableLog => PackageableValue, owned_elements : __classifiers::NestedListLog <
    __classifiers::BoxedLog < PackageableKindLog > > => Vec < Box < PackageableKindValue
    > >, }
);
__classifiers::union!(
    PackageKind = Package(Package, PackageLog => PackageValue) | Model(Model, ModelLog =>
    ModelValue)
);
__classifiers::record!(Model { package_super : PackageLog => PackageValue, });
__classifiers::union!(
    TTypeKind = DataType(DataTypeKind, DataTypeKindLog => DataTypeKindValue) |
    PrimitiveType(PrimitiveType, PrimitiveTypeLog => PrimitiveTypeValue) |
    Enumeration(Enumeration, EnumerationLog => EnumerationValue)
);
__classifiers::record!(
    TType { classifier_super : ClassifierLog => ClassifierValue, packageable_super :
    PackageableLog => PackageableValue, }
);
__classifiers::record!(
    DataType { t_type_super : TTypeLog => TTypeValue, attributes :
    __classifiers::NestedListLog < PropertyLog > => Vec < PropertyValue >, }
);
__classifiers::union!(
    DataTypeKind = DataType(DataType, DataTypeLog => DataTypeValue) | Class(Class,
    ClassLog => ClassValue)
);
__classifiers::record!(
    Class { data_type_super : DataTypeLog => DataTypeValue, abstract_field :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::EWFlag >> => Option
    < bool >, generalizations : __classifiers::NestedListLog < GeneralizationLog > => Vec
    < GeneralizationValue >, }
);
__classifiers::record!(
    Generalization { is_substitutable : __classifiers::OptionLog < __classifiers::VecLog
    < __classifiers::EWFlag >> => Option < bool >, }
);
__classifiers::record!(
    Property { model_element_super : ModelElementLog => ModelElementValue, }
);
__classifiers::record!(
    Association { model_element_super : ModelElementLog => ModelElementValue,
    packageable_super : PackageableLog => PackageableValue, }
);
__classifiers::record!(PrimitiveType { t_type_super : TTypeLog => TTypeValue, });
__classifiers::record!(
    Enumeration { t_type_super : TTypeLog => TTypeValue, owned_literal :
    __classifiers::NestedListLog < EnumerationLiteralLog > => Vec <
    EnumerationLiteralValue >, }
);
__classifiers::record!(
    EnumerationLiteral { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, }
);
__classifiers::record!(
    TaggedValue { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, value :
    __classifiers::OptionLog < __classifiers::GraphLog < __classifiers::List < char > >>
    => Option < Vec < char > >, }
);
