//! senda-agent-parser — read/write canonical agent documents and transpile
//! them to each CLI's native format.
//!
//! Phase 0 ships only the canonical reader/writer plus the [`Transpiler`]
//! trait, with stub implementations for the three CLIs. Phase 1 fills in the
//! actual conversions and the warning system.

pub mod canonical;
pub mod native;
pub mod transpilers;
pub mod warnings;

pub use canonical::{parse_canonical, serialize_canonical, ParseError};
pub use native::{detect_cli, parse_from_file, parse_native, NativeAgent};
pub use senda_core::CanonicalAgent;
pub use transpilers::Transpiler;
pub use warnings::Warning;
