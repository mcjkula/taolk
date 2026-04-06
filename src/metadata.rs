//! Locate `frame_system::AccountInfo.data.free` in the live runtime so balances
//! decode at the width the chain actually uses (Bittensor: `u64`; vanilla
//! Substrate: `u128`). Single-pass stream walk over V14 SCALE metadata; no
//! derived schema shells, no hardcoded offsets.

use parity_scale_codec::{Compact, Decode, Error as CodecError, Input};

const METADATA_MAGIC: u32 = 0x6174_656d; // "meta"

/// Byte position of `data.free` inside the SCALE-encoded `AccountInfo` blob,
/// discovered from the runtime's own type registry.
#[derive(Clone, Debug)]
pub struct AccountInfoLayout {
    pub free_offset: usize,
    pub free_width: usize,
}

impl AccountInfoLayout {
    /// Parse a `RuntimeMetadataPrefixed` (V14) blob and resolve the layout of
    /// `frame_system::AccountInfo.data.free`.
    pub fn from_runtime_metadata(bytes: &[u8]) -> Result<Self, String> {
        let input = &mut &bytes[..];

        let magic = u32::decode(input).map_err(scale)?;
        if magic != METADATA_MAGIC {
            return Err(format!("metadata magic mismatch: 0x{magic:08x}"));
        }
        let version = u8::decode(input).map_err(scale)?;
        if version != 14 {
            return Err(format!("metadata version {version} unsupported (need V14)"));
        }

        let registry = read_registry(input)?;
        let account_info_id = find_storage_value_type(input, "System", "Account")?;
        Self::resolve(&registry, account_info_id)
    }

    /// Decode `data.free` from a raw `System.Account` storage value.
    pub fn decode_free(&self, account_info: &[u8]) -> Result<u128, String> {
        let end = self.free_offset + self.free_width;
        if account_info.len() < end {
            return Err(format!(
                "AccountInfo storage truncated: have {} bytes, need {end}",
                account_info.len()
            ));
        }
        let mut buf = [0u8; 16];
        buf[..self.free_width].copy_from_slice(&account_info[self.free_offset..end]);
        Ok(u128::from_le_bytes(buf))
    }

    fn resolve(registry: &[TypeShape], account_info_id: u32) -> Result<Self, String> {
        let mut offset = 0;
        for (name, ty) in type_at(registry, account_info_id)?.composite("AccountInfo")? {
            if name == "data" {
                let mut inner = 0;
                for (df, dt) in type_at(registry, *ty)?.composite("AccountData")? {
                    if df == "free" {
                        return Ok(Self {
                            free_offset: offset + inner,
                            free_width: type_at(registry, *dt)?
                                .unsigned_int_width("AccountData.free")?,
                        });
                    }
                    inner += byte_size(registry, *dt)?;
                }
                return Err("AccountData.free not found".into());
            }
            offset += byte_size(registry, *ty)?;
        }
        Err("AccountInfo.data not found".into())
    }
}

// ---------------------------------------------------------------------------
// Type registry: only the shapes we need to size composites and identify the
// `free` primitive. Names are kept only on composite fields so we can walk to
// `AccountInfo.data.free` by name; everything else is anonymous.
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum TypeShape {
    Primitive { width: usize, unsigned_int: bool },
    Composite(Vec<(String, u32)>),
    Array { len: u32, inner: u32 },
    Tuple(Vec<u32>),
    Variable,
}

impl TypeShape {
    fn composite(&self, ctx: &'static str) -> Result<&[(String, u32)], String> {
        match self {
            Self::Composite(fields) => Ok(fields),
            _ => Err(format!("{ctx} is not a composite")),
        }
    }

    fn unsigned_int_width(&self, ctx: &'static str) -> Result<usize, String> {
        match self {
            Self::Primitive {
                width,
                unsigned_int: true,
            } => Ok(*width),
            _ => Err(format!("{ctx} is not an unsigned integer primitive")),
        }
    }
}

fn type_at(registry: &[TypeShape], id: u32) -> Result<&TypeShape, String> {
    registry
        .get(id as usize)
        .ok_or_else(|| format!("type id {id} missing from registry"))
}

fn byte_size(registry: &[TypeShape], id: u32) -> Result<usize, String> {
    match type_at(registry, id)? {
        TypeShape::Primitive { width, .. } => Ok(*width),
        TypeShape::Composite(fields) => fields
            .iter()
            .try_fold(0, |sum, (_, t)| Ok(sum + byte_size(registry, *t)?)),
        TypeShape::Array { len, inner } => Ok((*len as usize) * byte_size(registry, *inner)?),
        TypeShape::Tuple(ids) => ids
            .iter()
            .try_fold(0, |sum, t| Ok(sum + byte_size(registry, *t)?)),
        TypeShape::Variable => Err(format!("type id {id} has variable width")),
    }
}

// ---------------------------------------------------------------------------
// SCALE stream walk. Imperative, single pass; allocates only the registry and
// composite-field name lists. Unused fields are decoded-and-dropped via the
// built-in `Decode` impls so we never lose positional sync.
// ---------------------------------------------------------------------------

fn read_registry<I: Input>(input: &mut I) -> Result<Vec<TypeShape>, String> {
    let n = compact(input)?;
    let mut registry = Vec::with_capacity(n as usize);
    for expected in 0..n {
        let id = compact(input)?;
        if id != expected {
            return Err(format!("non-sequential type id {id} (expected {expected})"));
        }
        // Type { path, type_params, type_def, docs }
        skip_strings(input)?;
        skip_type_params(input)?;
        registry.push(read_type_def(input)?);
        skip_strings(input)?;
    }
    Ok(registry)
}

fn read_type_def<I: Input>(input: &mut I) -> Result<TypeShape, String> {
    Ok(match u8::decode(input).map_err(scale)? {
        0 => TypeShape::Composite(read_fields(input)?),
        1 => {
            // Variant { variants: Vec<{ name, fields, index, docs }> }
            for _ in 0..compact(input)? {
                let _ = String::decode(input).map_err(scale)?;
                let _ = read_fields(input)?;
                let _ = u8::decode(input).map_err(scale)?;
                skip_strings(input)?;
            }
            TypeShape::Variable
        }
        2 => {
            // Sequence { type_param }
            compact(input)?;
            TypeShape::Variable
        }
        3 => TypeShape::Array {
            len: u32::decode(input).map_err(scale)?,
            inner: compact(input)?,
        },
        4 => {
            // Tuple { fields: Vec<type_id> }
            let n = compact(input)?;
            let mut ids = Vec::with_capacity(n as usize);
            for _ in 0..n {
                ids.push(compact(input)?);
            }
            TypeShape::Tuple(ids)
        }
        5 => primitive_shape(u8::decode(input).map_err(scale)?)?,
        6 => {
            // Compact { type_param }
            compact(input)?;
            TypeShape::Variable
        }
        7 => {
            // BitSequence { bit_store_type, bit_order_type }
            compact(input)?;
            compact(input)?;
            TypeShape::Variable
        }
        tag => return Err(format!("unknown TypeDef tag {tag}")),
    })
}

fn read_fields<I: Input>(input: &mut I) -> Result<Vec<(String, u32)>, String> {
    let n = compact(input)?;
    let mut fields = Vec::with_capacity(n as usize);
    for _ in 0..n {
        // Field { name: Option<String>, ty: Compact<u32>, type_name: Option<String>, docs }
        let name = <Option<String>>::decode(input)
            .map_err(scale)?
            .unwrap_or_default();
        let ty = compact(input)?;
        let _ = <Option<String>>::decode(input).map_err(scale)?;
        skip_strings(input)?;
        fields.push((name, ty));
    }
    Ok(fields)
}

fn skip_type_params<I: Input>(input: &mut I) -> Result<(), String> {
    for _ in 0..compact(input)? {
        let _ = String::decode(input).map_err(scale)?;
        match u8::decode(input).map_err(scale)? {
            0 => {}
            1 => {
                compact(input)?;
            }
            tag => return Err(format!("invalid Option tag {tag}")),
        }
    }
    Ok(())
}

fn primitive_shape(tag: u8) -> Result<TypeShape, String> {
    let (width, unsigned_int) = match tag {
        0 => (1, false),                     // Bool
        1 => (4, false),                     // Char (u32 codepoint)
        2 => return Ok(TypeShape::Variable), // Str
        3 => (1, true),                      // U8
        4 => (2, true),                      // U16
        5 => (4, true),                      // U32
        6 => (8, true),                      // U64
        7 => (16, true),                     // U128
        8 => (32, true),                     // U256
        9 => (1, false),                     // I8
        10 => (2, false),                    // I16
        11 => (4, false),                    // I32
        12 => (8, false),                    // I64
        13 => (16, false),                   // I128
        14 => (32, false),                   // I256
        _ => return Err(format!("unknown primitive tag {tag}")),
    };
    Ok(TypeShape::Primitive {
        width,
        unsigned_int,
    })
}

// ---------------------------------------------------------------------------
// Pallet stream walk: locate the value type id of `<pallet>.<entry>` storage
// and return as soon as it's found, leaving the rest of the metadata stream
// unread.
// ---------------------------------------------------------------------------

fn find_storage_value_type<I: Input>(
    input: &mut I,
    pallet: &str,
    entry: &str,
) -> Result<u32, String> {
    for _ in 0..compact(input)? {
        let pallet_name = String::decode(input).map_err(scale)?;
        // storage: Option<{ prefix, entries }>
        if option_tag(input)? {
            let _ = String::decode(input).map_err(scale)?;
            for _ in 0..compact(input)? {
                let entry_name = String::decode(input).map_err(scale)?;
                let _ = u8::decode(input).map_err(scale)?; // modifier
                let value_ty = read_storage_entry_value_type(input)?;
                if pallet_name == pallet && entry_name == entry {
                    return Ok(value_ty);
                }
                let _ = <Vec<u8>>::decode(input).map_err(scale)?; // default
                skip_strings(input)?; // docs
            }
        }
        skip_optional_compact(input)?; // calls:    Option<{ ty }>
        skip_optional_compact(input)?; // event:    Option<{ ty }>
        for _ in 0..compact(input)? {
            // constants: Vec<{ name, ty, value, docs }>
            let _ = String::decode(input).map_err(scale)?;
            compact(input)?;
            let _ = <Vec<u8>>::decode(input).map_err(scale)?;
            skip_strings(input)?;
        }
        skip_optional_compact(input)?; // error:    Option<{ ty }>
        let _ = u8::decode(input).map_err(scale)?; // pallet index
    }
    Err(format!("{pallet}.{entry} storage entry not found"))
}

fn read_storage_entry_value_type<I: Input>(input: &mut I) -> Result<u32, String> {
    match u8::decode(input).map_err(scale)? {
        0 => compact(input), // Plain(value)
        1 => {
            // Map { hashers: Vec<StorageHasher>, key, value }
            for _ in 0..compact(input)? {
                let _ = u8::decode(input).map_err(scale)?;
            }
            compact(input)?; // key
            compact(input) // value
        }
        tag => Err(format!("unknown StorageEntryType tag {tag}")),
    }
}

// ---------------------------------------------------------------------------
// SCALE primitives
// ---------------------------------------------------------------------------

fn compact<I: Input>(input: &mut I) -> Result<u32, String> {
    Ok(<Compact<u32>>::decode(input).map_err(scale)?.0)
}

fn skip_strings<I: Input>(input: &mut I) -> Result<(), String> {
    let _ = <Vec<String>>::decode(input).map_err(scale)?;
    Ok(())
}

fn option_tag<I: Input>(input: &mut I) -> Result<bool, String> {
    match u8::decode(input).map_err(scale)? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(format!("invalid Option tag {tag}")),
    }
}

fn skip_optional_compact<I: Input>(input: &mut I) -> Result<(), String> {
    if option_tag(input)? {
        compact(input)?;
    }
    Ok(())
}

fn scale(e: CodecError) -> String {
    format!("scale decode: {e}")
}
