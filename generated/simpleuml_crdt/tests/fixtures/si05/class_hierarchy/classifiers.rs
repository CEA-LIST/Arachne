/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_protocol::state::graph_log::GraphLog;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_crdt::list::nested_list::NestedListLog;
    pub use moirai_protocol::state::log::BoxedLog;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_crdt::counter::resettable_counter::Counter;
    pub use moirai_crdt::flag::ew_flag::EWFlag;
}
__classifiers::union!(
    ModelElementKind = Classifier(ClassifierKind, ClassifierKindLog =>
    ClassifierKindValue) | Package(Package, PackageLog => PackageValue) |
    StructuralFeature(StructuralFeatureKind, StructuralFeatureKindLog =>
    StructuralFeatureKindValue)
);
__classifiers::record!(
    ModelElement { name : __classifiers::GraphLog < __classifiers::List < char > > => Vec
    < char >, }
);
__classifiers::union!(
    ClassifierKind = Class(Class, ClassLog => ClassValue) | DataType(DataType,
    DataTypeLog => DataTypeValue)
);
__classifiers::record!(
    Classifier { model_element_super : ModelElementLog => ModelElementValue, }
);
__classifiers::record!(
    Package { model_element_super : ModelElementLog => ModelElementValue, content :
    __classifiers::NestedListLog < __classifiers::BoxedLog < ModelElementKindLog > > =>
    Vec < Box < ModelElementKindValue > >, }
);
__classifiers::union!(
    StructuralFeatureKind = Attribute(Attribute, AttributeLog => AttributeValue) |
    Reference(Reference, ReferenceLog => ReferenceValue)
);
__classifiers::record!(
    StructuralFeature { model_element_super : ModelElementLog => ModelElementValue, lower
    : __classifiers::VecLog < __classifiers::Counter < i32 > > => i32, upper :
    __classifiers::VecLog < __classifiers::Counter < i32 > > => i32, is_ordered :
    __classifiers::VecLog < __classifiers::EWFlag > => bool, is_unique :
    __classifiers::VecLog < __classifiers::EWFlag > => bool, }
);
__classifiers::record!(
    Attribute { structural_feature_super : StructuralFeatureLog =>
    StructuralFeatureValue, }
);
__classifiers::record!(
    Reference { structural_feature_super : StructuralFeatureLog =>
    StructuralFeatureValue, is_container : __classifiers::VecLog < __classifiers::EWFlag
    > => bool, }
);
__classifiers::record!(
    Class { classifier_super : ClassifierLog => ClassifierValue, is_abstract :
    __classifiers::VecLog < __classifiers::EWFlag > => bool, features :
    __classifiers::NestedListLog < StructuralFeatureKindLog > => Vec <
    StructuralFeatureKindValue >, }
);
__classifiers::record!(
    DataType { classifier_super : ClassifierLog => ClassifierValue, }
);
