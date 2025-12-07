# poem-openapi-foreign

Wrap foreign types (types you don't own) to use them in [poem-openapi](https://github.com/poem-web/poem) endpoints without implementing custom derives.

## What is this?

This library provides `Foreign<T>` and `ForeignOpt<T>` wrappers that automatically generate OpenAPI schemas for types that implement `Serialize` and `DeserializeOwned`, without requiring you to derive `poem_openapi::Object` or other poem-openapi traits.

## When is this useful?

Use this when you need to use types in poem-openapi endpoints that:
- Come from external crates you don't control
- Are defined elsewhere in your codebase without poem-openapi derives
- Would be tedious to manually wrap with newtype patterns

**Example use cases:**
- Using types from database ORMs (SQLx, Diesel, SeaORM) directly in API responses
- Exposing types from business logic layers that use only serde
- Working with types from other API clients or SDKs

## Usage

Add to your `Cargo.toml`:
```toml
[dependencies]
jsonwrap = { path = "jsonwrap" }  # Or publish to crates.io
```

### Basic Example

```rust
use jsonwrap::{Foreign, ForeignOpt};
use poem_openapi::{OpenApi, payload::Json};

// A type you don't own or can't modify
#[derive(Serialize, Deserialize)]
struct ExternalType {
    id: i64,
    name: String,
}

struct Api;

#[OpenApi]
impl Api {
    /// Returns a required (non-nullable) response
    #[oai(path = "/item", method = "get")]
    async fn get_item(&self) -> Json<Foreign<ExternalType>> {
        Json(Foreign(ExternalType {
            id: 1,
            name: "Example".to_string(),
        }))
    }

    /// Returns an optional (nullable) response
    #[oai(path = "/maybe-item", method = "get")]
    async fn get_maybe_item(&self) -> Json<ForeignOpt<ExternalType>> {
        Json(ForeignOpt(Some(ExternalType {
            id: 2,
            name: "Maybe".to_string(),
        })))
    }

    /// Returns None
    #[oai(path = "/no-item", method = "get")]
    async fn get_no_item(&self) -> Json<ForeignOpt<ExternalType>> {
        Json(ForeignOpt(None))
    }
}
```

### Generated OpenAPI Schema

For `Foreign<ExternalType>`:
```json
{
  "$ref": "#/components/schemas/ExternalType"
}
```

For `ForeignOpt<ExternalType>`:
```json
{
  "title": "ExternalType",
  "nullable": true,
  "allOf": [
    {
      "$ref": "#/components/schemas/ExternalType"
    }
  ]
}
```

The schema definition in `components/schemas`:
```json
{
  "ExternalType": {
    "type": "object",
    "properties": {
      "id": { "type": "integer" },
      "name": { "type": "string" }
    }
  }
}
```

## How it works

The library uses [serde_reflection](https://docs.rs/serde_reflection/) to introspect the structure of types at runtime:

1. **Type Introspection**: Uses serde's serialization format to discover fields, variants, and nested types
2. **Schema Generation**: Converts the serde format into OpenAPI `MetaSchema` definitions
3. **Registry Integration**: Registers schemas with poem-openapi's type registry
4. **Nullable Handling**: `ForeignOpt<T>` wraps types in a nullable schema for optional responses

### Type Mapping

| Serde Type | OpenAPI Type |
|------------|--------------|
| `String`, `char` | `string` |
| `i8..i128`, `u8..u128` | `integer` |
| `f32`, `f64` | `number` |
| `bool` | `boolean` |
| `()` | `null` |
| `Vec<T>`, `[T]` | `array` with `items` |
| `HashMap<K, V>` | `object` with `additionalProperties` |
| `struct { .. }` | `object` with `properties` |
| `enum { .. }` | `object` with `anyOf` |
| `Option<T>` | Unwrapped (use `ForeignOpt<T>` for nullable schemas) |

## Shortcomings

### 1. No Metadata Support

**The library cannot extract:**
- Doc comments (`/// ...`)
- Examples (`#[oai(example = "...")]`)
- Deprecation markers (`#[deprecated]`)
- Descriptions or custom attributes

**Why?** `serde_reflection` only introspects runtime structure, not compile-time metadata. Doc comments and attributes are not available at runtime.

**Impact:**
```rust
/// This documentation will NOT appear in OpenAPI schema
#[derive(Serialize, Deserialize)]
struct MyType {
    /// This field description will NOT appear either
    field: String,
}
```

The generated schema will only contain structure (type, fields) without any documentation.

### 2. Limited Validation

**The library cannot enforce:**
- String patterns (`#[oai(pattern = "...")]`)
- Number ranges (`#[oai(minimum = 0, maximum = 100)]`)
- Array length constraints
- Custom validators

These require poem-openapi's derive macros to work.

### 3. Newtype Struct Transparency

Newtype structs are "unwrapped" to expose their inner type:

```rust
#[derive(Serialize, Deserialize)]
struct UserId(i64);

// Foreign<UserId> will generate schema:
// { "type": "integer" }
// NOT { "type": "object", "properties": { "0": { "type": "integer" } } }
```

This is usually desired but may cause issues if you want the newtype to be opaque.

### 4. Complex Enum Handling

Serde's enum representation can be complex. The library does its best to map variants but may not handle all serde attributes perfectly:
- `#[serde(tag = "type")]` (internally tagged)
- `#[serde(untagged)]` (untagged)
- `#[serde(tag = "type", content = "value")]` (adjacently tagged)

These may produce schemas that don't exactly match your expectations.

### 5. Performance Overhead

Type introspection happens at registration time. For large type hierarchies, this may add startup time to your application.

## Alternative: Nightly Branch with Specialization

The `nightly` branch uses Rust's `#![feature(specialization)]` to provide a cleaner API:

```rust
// On nightly branch:
use jsonwrap::Foreign;

// Required field
async fn handler(&self) -> Json<Foreign<T>> { ... }

// Optional field - no separate ForeignOpt type needed!
async fn handler(&self) -> Json<Foreign<Option<T>>> { ... }
```

**Trade-offs:**
- ✅ Cleaner API (no `ForeignOpt`)
- ✅ More intuitive (`Option<T>` maps to nullable)
- ❌ Requires nightly Rust
- ❌ Uses unstable `specialization` feature (may cause compiler issues)

See the `nightly` branch for details.

## Comparison with Native poem-openapi

| Feature | `Foreign<T>` | `#[derive(Object)]` |
|---------|--------------|---------------------|
| Works with external types | ✅ Yes | ❌ No |
| Doc comments in schema | ❌ No | ✅ Yes |
| Field descriptions | ❌ No | ✅ Yes |
| Examples | ❌ No | ✅ Yes |
| Validation (min/max/pattern) | ❌ No | ✅ Yes |
| Stable Rust | ✅ Yes (main) | ✅ Yes |
| Runtime introspection | ✅ Yes | ❌ No |
| Compile-time safety | ⚠️ Partial | ✅ Full |

## License

MIT OR Apache-2.0

## Contributing

Contributions welcome! Please test both `main` and `nightly` branches if making changes to core functionality.
