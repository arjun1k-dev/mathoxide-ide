//! .moab — MathOxide compiled asset bundle.
//!
//! Layout: 4-byte magic "MOAB", u32 version, then length-prefixed postcard
//! sections. Unknown sections are skipped on read (forward compat).

use mo_core::scene::Timeline;
use thiserror::Error;

pub const MAGIC: &[u8; 4] = b"MOAB";
pub const VERSION: u32 = 1;

#[derive(Error, Debug)]
pub enum MoabError {
    #[error("bad magic (not a .moab file)")]
    BadMagic,
    #[error("unsupported version {0}")]
    Version(u32),
    #[error("corrupt section: {0}")]
    Corrupt(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Bundle {
    /// The timeline IR — the editable command stream.
    timeline: Timeline,
    /// Baked geometry per object name at the end state (inspectable without
    /// re-running anything).
    final_state: std::collections::HashMap<String, std::sync::Arc<mo_core::vgroup::VGroup>>,
    meta: Meta,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Meta {
    pub source_hash: u64,
    pub created_unix: u64,
    pub generator: String,
}

/// Compile a .moa source into .moab bytes.
pub fn compile(src: &str) -> Result<Vec<u8>, MoabError> {
    let timeline = mo_dsl::parse(src).map_err(|d| {
        MoabError::Corrupt(
            d.iter().map(|x| format!("{}:{}: {}", x.line, x.col, x.message)).collect::<Vec<_>>().join("; "),
        )
    })?;
    let final_state = mo_core::scene::Player::state_at(&timeline, f32::INFINITY);
    let bundle = Bundle {
        timeline,
        final_state,
        meta: Meta {
            source_hash: fxhash(src),
            created_unix: 0, // stamped by CLI
            generator: concat!("mathoxide ", env!("CARGO_PKG_VERSION")).into(),
        },
    };
    let body = postcard::to_allocvec(&bundle).map_err(|e| MoabError::Corrupt(e.to_string()))?;
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Open .moab bytes back into an editable bundle.
pub fn open(bytes: &[u8]) -> Result<(Timeline, std::collections::HashMap<String, std::sync::Arc<mo_core::vgroup::VGroup>>, Meta), MoabError> {
    if bytes.len() < 8 || &bytes[..4] != MAGIC {
        return Err(MoabError::BadMagic);
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version > VERSION {
        return Err(MoabError::Version(version));
    }
    let bundle: Bundle =
        postcard::from_bytes(&bytes[8..]).map_err(|e| MoabError::Corrupt(e.to_string()))?;
    Ok((bundle.timeline, bundle.final_state, bundle.meta))
}

/// Re-serialize after edits (re-export path).
pub fn save(
    timeline: &Timeline,
    final_state: &std::collections::HashMap<String, std::sync::Arc<mo_core::vgroup::VGroup>>,
    meta: &Meta,
) -> Result<Vec<u8>, MoabError> {
    let body = postcard::to_allocvec(&Bundle {
        timeline: timeline.clone(),
        final_state: final_state.clone(),
        meta: meta.clone(),
    })
    .map_err(|e| MoabError::Corrupt(e.to_string()))?;
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

fn fxhash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let src = "circle a r=2; rect b at (3,0); play showCreation(a) 1s;";
        let bytes = compile(src).unwrap();
        let (tl, state, meta) = open(&bytes).unwrap();
        assert_eq!(tl.stmts.len(), 3);
        assert!(state.contains_key("b"));
        assert!(meta.generator.starts_with("mathoxide"));
        // edited re-export
        let bytes2 = save(&tl, &state, &meta).unwrap();
        assert!(open(&bytes2).is_ok());
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(open(b"nope"), Err(MoabError::BadMagic)));
    }
}
