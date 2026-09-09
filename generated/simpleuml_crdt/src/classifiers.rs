/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_protocol::state::event_graph::EventGraph;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_crdt::bag::aw_bag::AWBagLog;
    pub use moirai_crdt::list::nested_list::NestedListLog;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_crdt::flag::ew_flag::EWFlag;
}
__classifiers::union!(
    PackageableKind = Package(PackageKind, PackageKindLog) | TType(TTypeKind,
    TTypeKindLog) | Association(Association, AssociationLog)
);
__classifiers::record!(Packageable {});
__classifiers::union!(
    ModelElementKind = Classifier(ClassifierKind, ClassifierKindLog) | Property(Property,
    PropertyLog) | Association(Association, AssociationLog)
);
__classifiers::record!(
    ModelElement { name : __classifiers::EventGraph < __classifiers::List < char > >,
    stereotype : __classifiers::AWBagLog < std::string::String >, tagged_value :
    __classifiers::NestedListLog < TaggedValueLog >, }
);
__classifiers::union!(
    ClassifierKind = Package(PackageKind, PackageKindLog) | TType(TTypeKind,
    TTypeKindLog)
);
__classifiers::record!(Classifier { model_element_super : ModelElementLog, });
__classifiers::record!(
    Package { packageable_super : PackageableLog, classifier_super : ClassifierLog,
    owned_elements : __classifiers::NestedListLog < Box < PackageableKindLog > >, }
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
    TType { packageable_super : PackageableLog, classifier_super : ClassifierLog, }
);
__classifiers::record!(
    DataType { t_type_super : TTypeLog, attributes : __classifiers::NestedListLog <
    PropertyLog >, }
);
__classifiers::union!(
    DataTypeKind = DataType(DataType, DataTypeLog) | Class(Class, ClassLog)
);
__classifiers::record!(
    Class { data_type_super : DataTypeLog, r#abstract : __classifiers::VecLog <
    __classifiers::EWFlag >, generalizations : __classifiers::NestedListLog <
    GeneralizationLog >, }
);
__classifiers::record!(
    Generalization { is_substitutable : __classifiers::VecLog < __classifiers::EWFlag >,
    }
);
__classifiers::record!(Property { model_element_super : ModelElementLog, });
__classifiers::record!(
    Association { packageable_super : PackageableLog, model_element_super :
    ModelElementLog, }
);
__classifiers::record!(PrimitiveType { t_type_super : TTypeLog, });
__classifiers::record!(
    Enumeration { t_type_super : TTypeLog, owned_literal : __classifiers::NestedListLog <
    EnumerationLiteralLog >, }
);
__classifiers::record!(
    EnumerationLiteral { name : __classifiers::EventGraph < __classifiers::List < char >
    >, }
);
__classifiers::record!(
    TaggedValue { name : __classifiers::EventGraph < __classifiers::List < char > >,
    value : __classifiers::EventGraph < __classifiers::List < char > >, }
);
