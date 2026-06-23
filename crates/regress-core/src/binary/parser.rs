use std::path::Path;

use anyhow::{Context, Result};
use object::{Object, ObjectSection, ObjectSymbol};

use super::symbol::SymbolEntry;

pub fn parse_symbols(path: &Path) -> Result<Vec<SymbolEntry>> {
    let data = std::fs::read(path)
        .with_context(|| format!("Cannot read binary: {}", path.display()))?;

    let file = object::File::parse(&*data)
        .with_context(|| format!("Cannot parse binary format: {}", path.display()))?;

    let mut symbols = Vec::new();

    for symbol in file.symbols() {
        let name = match symbol.name() {
            Ok(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };

        let size = symbol.size();
        if size == 0 {
            continue;
        }

        let section = match symbol.section() {
            object::SymbolSection::Section(idx) => file
                .section_by_index(idx)
                .ok()
                .and_then(|s| s.name().ok())
                .map(|s| s.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };

        symbols.push(SymbolEntry::new(name, size, section, symbol.address()));
    }

    Ok(symbols)
}
