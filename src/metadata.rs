//! Decode V14 SCALE runtime metadata into the artifacts taolk needs at
//! startup: the byte layout of `frame_system::AccountInfo.data.free` (so
//! balances decode at the width the chain actually uses), and the table of
//! pallet error variants (so `DispatchError` codes can be translated to
//! `Pallet::Variant` strings without hardcoding). Single-pass stream walk.

use parity_scale_codec::{Compact, Decode, Error as CodecError, Input};
use std::collections::HashMap;

pub use crate::error::MetadataError;

const METADATA_MAGIC: u32 = 0x6174_656d; // "meta"

#[derive(Clone, Debug)]
pub struct AccountInfoLayout {
    pub free_offset: usize,
    pub free_width: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ErrorTable {
    by_idx: HashMap<(u8, u8), ErrorEntry>,
}

#[derive(Clone, Debug)]
pub struct ErrorEntry {
    pub pallet: String,
    pub variant: String,
    pub doc: String,
}

#[derive(Clone, Debug)]
pub struct Metadata {
    pub layout: AccountInfoLayout,
    pub errors: ErrorTable,
}

impl Metadata {
    pub fn from_runtime_metadata(bytes: &[u8]) -> Result<Self, MetadataError> {
        let input = &mut &bytes[..];

        let magic = u32::decode(input).map_err(scale)?;
        if magic != METADATA_MAGIC {
            return Err(MetadataError::Scale(format!(
                "metadata magic mismatch: 0x{magic:08x}"
            )));
        }
        let version = u8::decode(input).map_err(scale)?;
        if version != 14 {
            return Err(MetadataError::Scale(format!(
                "metadata version {version} unsupported (need V14)"
            )));
        }

        let registry = read_registry(input)?;
        let pallets = walk_pallets(input)?;

        let layout = AccountInfoLayout::resolve(&registry, pallets.account_info_ty)?;
        let errors = ErrorTable::build(&registry, &pallets.errors);
        Ok(Self { layout, errors })
    }
}

impl ErrorTable {
    pub fn humanize(&self, pallet_idx: u8, err_idx: u8) -> Option<String> {
        let e = self.by_idx.get(&(pallet_idx, err_idx))?;
        if e.doc.is_empty() {
            Some(format!("{}::{}", e.pallet, e.variant))
        } else {
            Some(format!("{}::{}: {}", e.pallet, e.variant, e.doc))
        }
    }

    pub fn humanize_rpc_error(&self, raw: &str) -> String {
        if let Some(payload) = find_after_any(raw, &["RPC error: ", "transaction failed: "])
            && let Some(json_str) = trim_to_json(payload)
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str)
        {
            if let Some(s) = v.get("data").and_then(|d| d.as_str()) {
                return self
                    .maybe_translate_module(s)
                    .unwrap_or_else(|| s.to_string());
            }
            if let Some(s) = v.get("message").and_then(|m| m.as_str()) {
                return s.to_string();
            }
        }
        if let Some(t) = self.maybe_translate_module(raw) {
            return t;
        }
        raw.to_string()
    }

    fn maybe_translate_module(&self, s: &str) -> Option<String> {
        let start = s.find("Module")?;
        let tail = &s[start..];
        let idx = u8::try_from(parse_after(tail, "index:")?).ok()?;
        let err = u8::try_from(parse_first_byte_after(tail, "error:")?).ok()?;
        self.humanize(idx, err)
    }

    fn build(registry: &[TypeShape], pallets: &[PalletErrorRef]) -> Self {
        let mut by_idx = HashMap::new();
        for p in pallets {
            if let Some(TypeShape::Variant(variants)) = usize::try_from(p.error_ty)
                .ok()
                .and_then(|i| registry.get(i))
            {
                for (variant_idx, variant_name, doc) in variants {
                    by_idx.insert(
                        (p.pallet_idx, *variant_idx),
                        ErrorEntry {
                            pallet: p.pallet_name.clone(),
                            variant: variant_name.clone(),
                            doc: doc.clone(),
                        },
                    );
                }
            }
        }
        Self { by_idx }
    }
}

fn find_after_any<'a>(s: &'a str, needles: &[&str]) -> Option<&'a str> {
    needles
        .iter()
        .find_map(|n| s.find(n).map(|i| &s[i + n.len()..]))
}

fn trim_to_json(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if esc {
            esc = false;
            continue;
        }
        match b {
            b'\\' if in_str => esc = true,
            b'"' => in_str = !in_str,
            b'{' if !in_str => depth += 1,
            b'}' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_after(haystack: &str, needle: &str) -> Option<u32> {
    let after = haystack.find(needle)?;
    let rest = &haystack[after + needle.len()..];
    let digits: String = rest
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn parse_first_byte_after(haystack: &str, needle: &str) -> Option<u32> {
    let after = haystack.find(needle)?;
    let rest = &haystack[after + needle.len()..];
    let bracket = rest.find('[')?;
    let inside = &rest[bracket + 1..];
    let digits: String = inside.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

impl AccountInfoLayout {
    pub fn decode_free(&self, account_info: &[u8]) -> Result<u128, MetadataError> {
        let end = self.free_offset + self.free_width;
        if account_info.len() < end {
            return Err(MetadataError::AccountInfoShort {
                need: end,
                got: account_info.len(),
            });
        }
        let mut buf = [0u8; 16];
        buf[..self.free_width].copy_from_slice(&account_info[self.free_offset..end]);
        Ok(u128::from_le_bytes(buf))
    }

    fn resolve(registry: &[TypeShape], account_info_id: u32) -> Result<Self, MetadataError> {
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
                return Err(MetadataError::StorageNotFound("AccountData.free"));
            }
            offset += byte_size(registry, *ty)?;
        }
        Err(MetadataError::AccountInfoMissing)
    }
}

#[derive(Clone)]
enum TypeShape {
    Primitive {
        width: usize,
        unsigned_int: bool,
    },
    Composite(Vec<(String, u32)>),
    Array {
        len: u32,
        inner: u32,
    },
    Tuple(Vec<u32>),
    /// `(variant_index, variant_name, first_doc_line)` per variant.
    Variant(Vec<(u8, String, String)>),
    Variable,
}

#[derive(Clone, Debug)]
struct PalletErrorRef {
    pallet_name: String,
    pallet_idx: u8,
    error_ty: u32,
}

#[derive(Debug, Default)]
struct PalletWalkResult {
    account_info_ty: u32,
    errors: Vec<PalletErrorRef>,
}

impl TypeShape {
    fn composite(&self, ctx: &'static str) -> Result<&[(String, u32)], MetadataError> {
        match self {
            Self::Composite(fields) => Ok(fields),
            _ => Err(MetadataError::Shape {
                ctx,
                kind: "composite",
            }),
        }
    }

    fn unsigned_int_width(&self, ctx: &'static str) -> Result<usize, MetadataError> {
        match self {
            Self::Primitive {
                width,
                unsigned_int: true,
            } => Ok(*width),
            _ => Err(MetadataError::Shape {
                ctx,
                kind: "unsigned integer primitive",
            }),
        }
    }
}

fn type_at(registry: &[TypeShape], id: u32) -> Result<&TypeShape, MetadataError> {
    let idx = usize::try_from(id).map_err(|_| MetadataError::TypeIdMissing(id))?;
    registry.get(idx).ok_or(MetadataError::TypeIdMissing(id))
}

fn byte_size(registry: &[TypeShape], id: u32) -> Result<usize, MetadataError> {
    match type_at(registry, id)? {
        TypeShape::Primitive { width, .. } => Ok(*width),
        TypeShape::Composite(fields) => fields
            .iter()
            .try_fold(0, |sum, (_, t)| Ok(sum + byte_size(registry, *t)?)),
        TypeShape::Array { len, inner } => {
            // SECURITY: SCALE Array len comes from V14 metadata; bounded by Substrate runtime.
            let len_usize =
                usize::try_from(*len).map_err(|_| MetadataError::TypeIdMissing(*len))?;
            Ok(len_usize * byte_size(registry, *inner)?)
        }
        TypeShape::Tuple(ids) => ids
            .iter()
            .try_fold(0, |sum, t| Ok(sum + byte_size(registry, *t)?)),
        TypeShape::Variant(_) | TypeShape::Variable => Err(MetadataError::VariableWidth(id)),
    }
}

fn read_registry<I: Input>(input: &mut I) -> Result<Vec<TypeShape>, MetadataError> {
    let n = compact(input)?;
    let mut registry = Vec::with_capacity(usize::try_from(n).unwrap_or(0));
    for expected in 0..n {
        let id = compact(input)?;
        if id != expected {
            return Err(MetadataError::NonSequential { got: id, expected });
        }
        // Type { path, type_params, type_def, docs }
        skip_strings(input)?;
        skip_type_params(input)?;
        registry.push(read_type_def(input)?);
        skip_strings(input)?;
    }
    Ok(registry)
}

fn read_type_def<I: Input>(input: &mut I) -> Result<TypeShape, MetadataError> {
    Ok(match u8::decode(input).map_err(scale)? {
        0 => TypeShape::Composite(read_fields(input)?),
        1 => {
            // Variant { variants: Vec<{ name, fields, index, docs }> }
            // We capture (index, name, first_doc_line) for the pallet error path;
            // field shapes are decoded-and-dropped to keep stream sync.
            let n = compact(input)?;
            let mut variants = Vec::with_capacity(usize::try_from(n).unwrap_or(0));
            for _ in 0..n {
                let name = String::decode(input).map_err(scale)?;
                let _ = read_fields(input)?;
                let index = u8::decode(input).map_err(scale)?;
                let docs = <Vec<String>>::decode(input).map_err(scale)?;
                let doc = docs
                    .into_iter()
                    .map(|d| d.trim().to_string())
                    .find(|d| !d.is_empty())
                    .unwrap_or_default();
                variants.push((index, name, doc));
            }
            TypeShape::Variant(variants)
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
            let mut ids = Vec::with_capacity(usize::try_from(n).unwrap_or(0));
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
        tag => return Err(MetadataError::UnknownTypeDef(tag)),
    })
}

fn read_fields<I: Input>(input: &mut I) -> Result<Vec<(String, u32)>, MetadataError> {
    let n = compact(input)?;
    let mut fields = Vec::with_capacity(usize::try_from(n).unwrap_or(0));
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

fn skip_type_params<I: Input>(input: &mut I) -> Result<(), MetadataError> {
    for _ in 0..compact(input)? {
        let _ = String::decode(input).map_err(scale)?;
        match u8::decode(input).map_err(scale)? {
            0 => {}
            1 => {
                compact(input)?;
            }
            tag => return Err(MetadataError::InvalidOptionTag(tag)),
        }
    }
    Ok(())
}

fn primitive_shape(tag: u8) -> Result<TypeShape, MetadataError> {
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
        _ => return Err(MetadataError::UnknownPrimitive(tag)),
    };
    Ok(TypeShape::Primitive {
        width,
        unsigned_int,
    })
}

fn walk_pallets<I: Input>(input: &mut I) -> Result<PalletWalkResult, MetadataError> {
    let mut account_info_ty: Option<u32> = None;
    let mut errors: Vec<PalletErrorRef> = Vec::new();

    for _ in 0..compact(input)? {
        let pallet_name = String::decode(input).map_err(scale)?;
        // storage: Option<{ prefix, entries }>
        if option_tag(input)? {
            let _ = String::decode(input).map_err(scale)?;
            for _ in 0..compact(input)? {
                let entry_name = String::decode(input).map_err(scale)?;
                let _ = u8::decode(input).map_err(scale)?; // modifier
                let value_ty = read_storage_entry_value_type(input)?;
                if pallet_name == "System" && entry_name == "Account" {
                    account_info_ty = Some(value_ty);
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
        // error: Option<{ ty }>
        let error_ty = if option_tag(input)? {
            Some(compact(input)?)
        } else {
            None
        };
        let pallet_index = u8::decode(input).map_err(scale)?;
        if let Some(ty) = error_ty {
            errors.push(PalletErrorRef {
                pallet_name: pallet_name.clone(),
                pallet_idx: pallet_index,
                error_ty: ty,
            });
        }
    }

    Ok(PalletWalkResult {
        account_info_ty: account_info_ty.ok_or(MetadataError::StorageNotFound("System.Account"))?,
        errors,
    })
}

fn read_storage_entry_value_type<I: Input>(input: &mut I) -> Result<u32, MetadataError> {
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
        tag => Err(MetadataError::UnknownStorageEntryType(tag)),
    }
}

fn compact<I: Input>(input: &mut I) -> Result<u32, MetadataError> {
    Ok(<Compact<u32>>::decode(input).map_err(scale)?.0)
}

fn skip_strings<I: Input>(input: &mut I) -> Result<(), MetadataError> {
    let _ = <Vec<String>>::decode(input).map_err(scale)?;
    Ok(())
}

fn option_tag<I: Input>(input: &mut I) -> Result<bool, MetadataError> {
    match u8::decode(input).map_err(scale)? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(MetadataError::InvalidOptionTag(tag)),
    }
}

fn skip_optional_compact<I: Input>(input: &mut I) -> Result<(), MetadataError> {
    if option_tag(input)? {
        compact(input)?;
    }
    Ok(())
}

fn scale(e: CodecError) -> MetadataError {
    MetadataError::Scale(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_table() -> ErrorTable {
        let mut by_idx = HashMap::new();
        by_idx.insert(
            (5, 3),
            ErrorEntry {
                pallet: "Balances".into(),
                variant: "InsufficientBalance".into(),
                doc: "Balance too low to send value.".into(),
            },
        );
        by_idx.insert(
            (12, 0),
            ErrorEntry {
                pallet: "SubtensorModule".into(),
                variant: "NotEnoughBalanceToStake".into(),
                doc: String::new(),
            },
        );
        ErrorTable { by_idx }
    }

    #[test]
    fn humanize_includes_doc_when_present() {
        let t = fixture_table();
        assert_eq!(
            t.humanize(5, 3).as_deref(),
            Some("Balances::InsufficientBalance: Balance too low to send value."),
        );
    }

    #[test]
    fn humanize_omits_doc_when_empty() {
        let t = fixture_table();
        assert_eq!(
            t.humanize(12, 0).as_deref(),
            Some("SubtensorModule::NotEnoughBalanceToStake"),
        );
    }

    #[test]
    fn humanize_returns_none_for_unknown() {
        let t = fixture_table();
        assert!(t.humanize(99, 99).is_none());
    }

    #[test]
    fn humanize_rpc_error_extracts_data_field() {
        let t = ErrorTable::default();
        let raw = r#"RPC error: {"code":1010,"data":"Transaction has a bad signature","message":"Invalid Transaction"}"#;
        assert_eq!(t.humanize_rpc_error(raw), "Transaction has a bad signature");
    }

    #[test]
    fn humanize_rpc_error_falls_back_to_message_field() {
        let t = ErrorTable::default();
        let raw = r#"RPC error: {"code":1010,"message":"Invalid Transaction"}"#;
        assert_eq!(t.humanize_rpc_error(raw), "Invalid Transaction");
    }

    #[test]
    fn humanize_rpc_error_decodes_module_dispatch_error() {
        let t = fixture_table();
        let raw = r#"transaction failed: {"data":"Module(ModuleError { index: 5, error: [3, 0, 0, 0], message: None })"}"#;
        assert_eq!(
            t.humanize_rpc_error(raw),
            "Balances::InsufficientBalance: Balance too low to send value."
        );
    }

    #[test]
    fn humanize_rpc_error_passes_through_unparseable() {
        let t = ErrorTable::default();
        assert_eq!(t.humanize_rpc_error("not json at all"), "not json at all");
    }

    #[test]
    fn humanize_rpc_error_unwraps_send_failed_prefix() {
        let t = ErrorTable::default();
        let raw = r#"Send failed: RPC error: {"code":1010,"data":"Transaction has a bad signature","message":"Invalid Transaction"}"#;
        assert_eq!(t.humanize_rpc_error(raw), "Transaction has a bad signature");
    }
}
