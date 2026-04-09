use std::path::PathBuf;
use std::sync::Arc;

use blake2::digest::{Update, VariableOutput};
use samp::extrinsic::ChainParams;
use samp::metadata::{ErrorEntry, ErrorTable, StorageLayout};
use samp::{GenesisHash, SpecVersion, Ss58Prefix, TxVersion};
use serde::{Deserialize, Serialize};

use crate::error::SdkError;
use crate::extrinsic::ChainInfo;
use crate::types::ChainName;

// Cached `spec_version`/`tx_version` are never used at sign time:
// `submit_remark`/`estimate_fee` always call `refresh_signing_params` first.
// Cache tamper at worst makes `Db::open` fail closed (DB key is derived from
// `genesis_hash`).

#[derive(Serialize, Deserialize)]
pub struct ChainSnapshot {
    pub chain_name: String,
    pub ss58_prefix: u16,
    pub genesis_hash: [u8; 32],
    pub spec_version: u32,
    pub tx_version: u32,
    pub account_storage_offset: usize,
    pub account_storage_width: usize,
    pub errors: Vec<ErrorRecord>,
    pub token_symbol: String,
    pub token_decimals: u32,
}

#[derive(Serialize, Deserialize)]
pub struct ErrorRecord {
    pub pallet_idx: u8,
    pub error_idx: u8,
    pub pallet: String,
    pub variant: String,
    pub doc: String,
}

impl ChainSnapshot {
    pub fn from_chain_info(info: &ChainInfo, token_symbol: &str, token_decimals: u32) -> Self {
        let errors = info
            .errors
            .iter()
            .map(|((p, e), entry)| ErrorRecord {
                pallet_idx: p,
                error_idx: e,
                pallet: entry.pallet.clone(),
                variant: entry.variant.clone(),
                doc: entry.doc.clone(),
            })
            .collect();
        Self {
            chain_name: info.name.as_str().to_string(),
            ss58_prefix: info.ss58_prefix.get(),
            genesis_hash: *info.chain_params.genesis_hash().as_bytes(),
            spec_version: info.chain_params.spec_version().get(),
            tx_version: info.chain_params.tx_version().get(),
            account_storage_offset: info.account_storage.offset,
            account_storage_width: info.account_storage.width,
            errors,
            token_symbol: token_symbol.to_string(),
            token_decimals,
        }
    }

    pub fn into_chain_info(self) -> Result<(ChainInfo, String, u32), SdkError> {
        let name = ChainName::parse(self.chain_name)
            .map_err(|e| SdkError::Other(format!("cached chain name invalid: {e}")))?;
        let ss58 = Ss58Prefix::new(self.ss58_prefix)
            .map_err(|e| SdkError::Other(format!("cached ss58 prefix invalid: {e}")))?;
        let errors = ErrorTable::from_entries(self.errors.into_iter().map(|r| {
            (
                (r.pallet_idx, r.error_idx),
                ErrorEntry {
                    pallet: r.pallet,
                    variant: r.variant,
                    doc: r.doc,
                },
            )
        }));
        let info = ChainInfo {
            name,
            ss58_prefix: ss58,
            chain_params: ChainParams::new(
                GenesisHash::from_bytes(self.genesis_hash),
                SpecVersion::new(self.spec_version),
                TxVersion::new(self.tx_version),
            ),
            account_storage: StorageLayout {
                offset: self.account_storage_offset,
                width: self.account_storage_width,
            },
            errors: Arc::new(errors),
        };
        Ok((info, self.token_symbol, self.token_decimals))
    }
}

fn cache_path(node_url: &str) -> Option<PathBuf> {
    let mut hasher = blake2::Blake2bVar::new(8).ok()?;
    hasher.update(node_url.as_bytes());
    let mut digest = [0u8; 8];
    hasher.finalize_variable(&mut digest).ok()?;
    let key = hex::encode(digest);
    Some(
        dirs::config_dir()?
            .join("taolk")
            .join("chains")
            .join(format!("{key}.json")),
    )
}

pub fn load(node_url: &str) -> Option<ChainSnapshot> {
    let path = cache_path(node_url)?;
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save(node_url: &str, snap: &ChainSnapshot) -> Result<(), SdkError> {
    let path = cache_path(node_url)
        .ok_or_else(|| SdkError::Other("could not resolve cache path".into()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| SdkError::Other(format!("create cache dir: {e}")))?;
    }
    let bytes =
        serde_json::to_vec(snap).map_err(|e| SdkError::Other(format!("encode snapshot: {e}")))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| SdkError::Other(format!("write tmp: {e}")))?;
    std::fs::rename(&tmp, &path).map_err(|e| SdkError::Other(format!("rename: {e}")))?;
    Ok(())
}
