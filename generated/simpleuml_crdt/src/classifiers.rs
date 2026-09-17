/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_protocol::state::graph_log::GraphLog;
    pub use moirai_crdt::option::OptionLog;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_crdt::set::aw_set::AWSet;
    pub use moirai_crdt::list::nested_list::NestedListLog;
    pub use moirai_protocol::state::log::BoxedLog;
    pub use moirai_crdt::flag::ew_flag::EWFlag;
}
__classifiers::union!(
    ModelElementKind = Classifier(ClassifierKind, ClassifierKindLog) | Property(Property,
    PropertyLog) | Association(Association, AssociationLog)
);
__classifiers::record!(
    ModelElement { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >>, stereotype : __classifiers::VecLog <
    __classifiers::AWSet < std::string::String >>, tagged_value :
    __classifiers::NestedListLog < TaggedValueLog >, }
);
__classifiers::union!(
    ClassifierKind = Package(PackageKind, PackageKindLog) | TType(TTypeKind,
    TTypeKindLog)
);
__classifiers::record!(Classifier { model_element_super : ModelElementLog, });
__classifiers::union!(
    PackageableKind = Package(PackageKind, PackageKindLog) | TType(TTypeKind,
    TTypeKindLog) | Association(Association, AssociationLog)
);
__classifiers::record!(Packageable {});
__classifiers::record!(
    Package { classifier_super : ClassifierLog, packageable_super : PackageableLog,
    owned_elements : __classifiers::NestedListLog < __classifiers::BoxedLog <
    PackageableKindLog > >, }
);
__classifiers::union!(
    PackageKind = Package(Package, PackageLog) | Model(Model, ModelLog)
);
__classifiers::record!(Model { package_super : PackageLog, });
__classifiers::union!(
    TTypeKind = DataType(DataTypeKind, DataTypeKindLog) | PrimitiveType(PrimitiveType,
    PrimitiveTypeLog) | Enumeration(Enumeration, EnumerationLog)
);
__classifiers::record!(
    TType { classifier_super : ClassifierLog, packageable_super : PackageableLog, }
);
__classifiers::record!(
    DataType { t_type_super : TTypeLog, attributes : __classifiers::NestedListLog <
    PropertyLog >, }
);
__classifiers::union!(
    DataTypeKind = DataType(DataType, DataTypeLog) | Class(Class, ClassLog)
);
__classifiers::record!(
    Class { data_type_super : DataTypeLog, abstract_field : __classifiers::OptionLog <
    __classifiers::VecLog < __classifiers::EWFlag >>, generalizations :
    __classifiers::NestedListLog < GeneralizationLog >, }
);
__classifiers::record!(
    Generalization { is_substitutable : __classifiers::OptionLog < __classifiers::VecLog
    < __classifiers::EWFlag >>, }
);
__classifiers::record!(Property { model_element_super : ModelElementLog, });
__classifiers::record!(
    Association { model_element_super : ModelElementLog, packageable_super :
    PackageableLog, }
);
__classifiers::record!(PrimitiveType { t_type_super : TTypeLog, });
__classifiers::record!(
    Enumeration { t_type_super : TTypeLog, owned_literal : __classifiers::NestedListLog <
    EnumerationLiteralLog >, }
);
__classifiers::record!(
    EnumerationLiteral { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >>, }
);
__classifiers::record!(
    TaggedValue { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >>, value : __classifiers::OptionLog <
    __classifiers::GraphLog < __classifiers::List < char > >>, }
);
