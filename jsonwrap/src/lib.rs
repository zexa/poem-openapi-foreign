#![feature(specialization)]

use poem_openapi::registry::{MetaSchema, MetaSchemaRef, Registry};
use poem_openapi::types::{ToJSON, Type};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use serde_reflection::{
    ContainerFormat, Format, Registry as SerdeRegistry, Tracer, TracerConfig, VariantFormat,
};

pub struct Foreign<T>(pub T);

impl<T> From<T> for Foreign<T> {
    fn from(value: T) -> Self {
        Foreign(value)
    }
}

fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}

fn format_to_schema(
    format: &Format,
    serde_reg: &SerdeRegistry,
    poem_reg: &mut Registry,
) -> MetaSchemaRef {
    match format {
        Format::Str => MetaSchemaRef::Inline(Box::new(MetaSchema {
            ty: "string",
            ..MetaSchema::ANY
        })),
        Format::I8
        | Format::I16
        | Format::I32
        | Format::I64
        | Format::I128
        | Format::U8
        | Format::U16
        | Format::U32
        | Format::U64
        | Format::U128 => MetaSchemaRef::Inline(Box::new(MetaSchema {
            ty: "integer",
            ..MetaSchema::ANY
        })),
        Format::F32 | Format::F64 => MetaSchemaRef::Inline(Box::new(MetaSchema {
            ty: "number",
            ..MetaSchema::ANY
        })),
        Format::Bool => MetaSchemaRef::Inline(Box::new(MetaSchema {
            ty: "boolean",
            ..MetaSchema::ANY
        })),
        Format::Char => MetaSchemaRef::Inline(Box::new(MetaSchema {
            ty: "string",
            ..MetaSchema::ANY
        })),
        Format::Unit => MetaSchemaRef::Inline(Box::new(MetaSchema {
            ty: "null",
            ..MetaSchema::ANY
        })),
        Format::Option(inner) => format_to_schema(inner, serde_reg, poem_reg),
        Format::Seq(inner) => {
            let items = format_to_schema(inner, serde_reg, poem_reg);
            MetaSchemaRef::Inline(Box::new(MetaSchema {
                ty: "array",
                items: Some(Box::new(items)),
                ..MetaSchema::ANY
            }))
        }
        Format::Map { key: _, value } => {
            let additional = format_to_schema(value, serde_reg, poem_reg);
            MetaSchemaRef::Inline(Box::new(MetaSchema {
                ty: "object",
                additional_properties: Some(Box::new(additional)),
                ..MetaSchema::ANY
            }))
        }
        Format::Tuple(formats) => {
            let items: Vec<_> = formats
                .iter()
                .map(|f| format_to_schema(f, serde_reg, poem_reg))
                .collect();
            MetaSchemaRef::Inline(Box::new(MetaSchema {
                ty: "array",
                all_of: items,
                ..MetaSchema::ANY
            }))
        }
        Format::TypeName(name) => {
            register_type(name, serde_reg, poem_reg);
            MetaSchemaRef::Reference(name.clone())
        }
        _ => MetaSchemaRef::Inline(Box::new(MetaSchema {
            ty: "object",
            ..MetaSchema::ANY
        })),
    }
}

fn variant_to_schema(
    variant_format: &VariantFormat,
    serde_reg: &SerdeRegistry,
    poem_reg: &mut Registry,
) -> MetaSchemaRef {
    match variant_format {
        VariantFormat::Unit => MetaSchemaRef::Inline(Box::new(MetaSchema {
            ty: "null",
            ..MetaSchema::ANY
        })),
        VariantFormat::NewType(inner) => format_to_schema(inner, serde_reg, poem_reg),
        VariantFormat::Tuple(formats) => MetaSchemaRef::Inline(Box::new(MetaSchema {
            ty: "array",
            all_of: formats
                .iter()
                .map(|f| format_to_schema(f, serde_reg, poem_reg))
                .collect(),
            ..MetaSchema::ANY
        })),
        VariantFormat::Struct(fields) => MetaSchemaRef::Inline(Box::new(MetaSchema {
            ty: "object",
            properties: fields
                .iter()
                .map(|field| {
                    (
                        leak_str(&field.name),
                        format_to_schema(&field.value, serde_reg, poem_reg),
                    )
                })
                .collect(),
            ..MetaSchema::ANY
        })),
        VariantFormat::Variable(_) => MetaSchemaRef::Inline(Box::new(MetaSchema {
            ty: "object",
            ..MetaSchema::ANY
        })),
    }
}

fn container_to_schema(
    format: &ContainerFormat,
    serde_reg: &SerdeRegistry,
    poem_reg: &mut Registry,
) -> MetaSchema {
    match format {
        ContainerFormat::Struct(fields) => MetaSchema {
            ty: "object",
            properties: fields
                .iter()
                .map(|field| {
                    (
                        leak_str(&field.name),
                        format_to_schema(&field.value, serde_reg, poem_reg),
                    )
                })
                .collect(),
            ..MetaSchema::ANY
        },
        ContainerFormat::NewTypeStruct(inner) => {
            // For newtype structs, we want to be transparent and expose the inner type's schema
            match format_to_schema(inner, serde_reg, poem_reg) {
                MetaSchemaRef::Inline(schema) => *schema,
                MetaSchemaRef::Reference(name) => {
                    // Register the inner type and return its schema
                    if let Some(inner_format) = serde_reg.get(&name) {
                        let inner_format = inner_format.clone();
                        container_to_schema(&inner_format, serde_reg, poem_reg)
                    } else {
                        MetaSchema {
                            ty: "object",
                            ..MetaSchema::ANY
                        }
                    }
                }
            }
        }
        ContainerFormat::TupleStruct(formats) => MetaSchema {
            ty: "array",
            all_of: formats
                .iter()
                .map(|f| format_to_schema(f, serde_reg, poem_reg))
                .collect(),
            ..MetaSchema::ANY
        },
        ContainerFormat::Enum(variants) => MetaSchema {
            ty: "object",
            any_of: variants
                .iter()
                .map(|(_idx, variant)| {
                    MetaSchemaRef::Inline(Box::new(MetaSchema {
                        ty: "object",
                        properties: vec![(
                            leak_str(&variant.name),
                            variant_to_schema(&variant.value, serde_reg, poem_reg),
                        )],
                        ..MetaSchema::ANY
                    }))
                })
                .collect(),
            ..MetaSchema::ANY
        },
        ContainerFormat::UnitStruct => MetaSchema {
            ty: "null",
            ..MetaSchema::ANY
        },
    }
}

fn register_type(name: &str, serde_reg: &SerdeRegistry, poem_reg: &mut Registry) {
    if let Some(format) = serde_reg.get(name) {
        let format = format.clone();
        poem_reg.create_schema::<(), _>(name.to_string(), |poem_reg| {
            container_to_schema(&format, serde_reg, poem_reg)
        });
    }
}

fn type_name<T: 'static>() -> String {
    let full = std::any::type_name::<T>();
    full.rsplit("::").next().unwrap_or(full).to_string()
}

fn trace_type<T: DeserializeOwned>() -> Option<SerdeRegistry> {
    let mut tracer = Tracer::new(TracerConfig::default());
    tracer.trace_simple_type::<T>().ok()?;
    tracer.registry().ok()
}

impl<T: Serialize + DeserializeOwned + Send + Sync + 'static> Type for Foreign<T> {
    default const IS_REQUIRED: bool = true;
    type RawValueType = Self;
    type RawElementValueType = Self;

    default fn name() -> std::borrow::Cow<'static, str> {
        let name = type_name::<T>();
        // For newtype structs, expose the inner type's name
        if let Some(serde_reg) = trace_type::<T>() {
            if let Some(ContainerFormat::NewTypeStruct(inner_format)) = serde_reg.get(&name) {
                if let Format::TypeName(inner_name) = inner_format.as_ref() {
                    return inner_name.clone().into();
                }
            }
        }
        name.into()
    }

    default fn schema_ref() -> MetaSchemaRef {
        let name = type_name::<T>();
        // For newtype structs, reference the inner type's schema
        if let Some(serde_reg) = trace_type::<T>() {
            if let Some(ContainerFormat::NewTypeStruct(inner_format)) = serde_reg.get(&name) {
                if let Format::TypeName(inner_name) = inner_format.as_ref() {
                    return MetaSchemaRef::Reference(inner_name.clone());
                }
            }
        }
        MetaSchemaRef::Reference(name)
    }

    default fn register(poem_reg: &mut Registry) {
        let name = type_name::<T>();
        let Some(serde_reg) = trace_type::<T>() else {
            poem_reg.create_schema::<Self, _>(name, |_| MetaSchema {
                ty: "object",
                ..MetaSchema::ANY
            });
            return;
        };

        if let Some(format) = serde_reg.get(&name) {
            let format = format.clone();
            // For newtype structs, use the inner type's name
            let schema_name = match &format {
                ContainerFormat::NewTypeStruct(inner_format) => {
                    if let Format::TypeName(inner_name) = inner_format.as_ref() {
                        inner_name.clone()
                    } else {
                        name
                    }
                }
                _ => name,
            };
            poem_reg.create_schema::<Self, _>(schema_name, |poem_reg| {
                container_to_schema(&format, &serde_reg, poem_reg)
            });
        }
    }

    fn as_raw_value(&self) -> Option<&Self::RawValueType> {
        Some(self)
    }

    fn raw_element_iter<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = &'a Self::RawElementValueType> + 'a> {
        Box::new(std::iter::once(self))
    }
}

impl<T: Serialize + DeserializeOwned + Send + Sync + 'static> ToJSON for Foreign<T> {
    default fn to_json(&self) -> Option<Value> {
        serde_json::to_value(&self.0).ok()
    }
}

impl<T: Serialize + DeserializeOwned + Send + Sync + 'static> Type for Foreign<Option<T>> {
    const IS_REQUIRED: bool = false;

    fn name() -> std::borrow::Cow<'static, str> {
        Foreign::<T>::name()
    }

    fn schema_ref() -> MetaSchemaRef {
        // Return an inline schema that marks the type as nullable
        let base_ref = Foreign::<T>::schema_ref();
        match base_ref {
            MetaSchemaRef::Reference(name) => {
                MetaSchemaRef::Inline(Box::new(MetaSchema {
                    title: Some(name.clone()),
                    nullable: true,
                    all_of: vec![MetaSchemaRef::Reference(name)],
                    ..MetaSchema::ANY
                }))
            }
            MetaSchemaRef::Inline(mut schema) => {
                schema.nullable = true;
                MetaSchemaRef::Inline(schema)
            }
        }
    }

    fn register(poem_reg: &mut Registry) {
        Foreign::<T>::register(poem_reg);
    }
}

impl<T: Serialize + DeserializeOwned + Send + Sync + 'static> ToJSON for Foreign<Option<T>> {
    fn to_json(&self) -> Option<Value> {
        self.0.as_ref().and_then(|v| serde_json::to_value(v).ok())
    }
}
