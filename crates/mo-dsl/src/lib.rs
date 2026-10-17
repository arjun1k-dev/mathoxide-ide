//! The .moa DSL: declarative scene files.
//!
//! ```text
//! circle c at (0,0) r=2 fill=blue stroke=#58C4DD sw=4;
//! play fadeIn(c) 2s smooth;
//! wait 0.5s;
//! play transform(c, c2);
//! ```
//!
//! Parsed with winnow into a mo_core::Timeline. Errors carry (line, col).

mod parser;

pub use parser::{parse, parse_with_origins, Diag, PlayOrigin};

/// Convenience: parse + report human-friendly diagnostics.
pub fn parse_or_report(src: &str) -> Result<mo_core::scene::Timeline, String> {
    parse(src).map_err(|diags| {
        diags
            .iter()
            .map(|d| format!("{}:{}: {}", d.line, d.col, d.message))
            .collect::<Vec<_>>()
            .join("\n")
    })
}
