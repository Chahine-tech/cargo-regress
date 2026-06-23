pub mod parser;
pub mod symbol;

pub use parser::parse_symbols;
pub use symbol::SymbolEntry;
pub(crate) use symbol::crate_from_demangled;
