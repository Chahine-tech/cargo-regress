use std::path::Path;

use anyhow::{Context, Result};
use object::{Object, ObjectSection, ObjectSymbol};

use super::symbol::SymbolEntry;

pub fn parse_symbols(path: &Path) -> Result<Vec<SymbolEntry>> {
    let data = std::fs::read(path)
        .with_context(|| format!("Cannot read binary: {}", path.display()))?;

    let file = object::File::parse(&*data)
        .with_context(|| format!("Cannot parse binary format: {}", path.display()))?;

    // Collect (address, name, section_name) for all named symbols.
    let mut raw: Vec<(u64, String, String)> = Vec::new();

    for symbol in file.symbols() {
        let name = match symbol.name() {
            Ok(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };

        let section_name = match symbol.section() {
            object::SymbolSection::Section(idx) => file
                .section_by_index(idx)
                .ok()
                .and_then(|s| s.name().ok())
                .map(|s| s.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };

        raw.push((symbol.address(), name, section_name));
    }

    if raw.is_empty() {
        return Ok(Vec::new());
    }

    // Sort by address so we can compute sizes from address differences.
    raw.sort_unstable_by_key(|(addr, _, _)| *addr);

    let mut symbols: Vec<SymbolEntry> = Vec::with_capacity(raw.len());

    for i in 0..raw.len() {
        let (addr, ref name, ref section) = raw[i];

        // Size = distance to the next symbol (or 0 for the last one).
        // This approximation matches what bloaty / cargo-bloat use for Mach-O.
        let size = if i + 1 < raw.len() {
            raw[i + 1].0.saturating_sub(addr)
        } else {
            0
        };

        if size == 0 {
            continue;
        }

        symbols.push(SymbolEntry::new(name.clone(), size, section.clone(), addr));
    }

    Ok(symbols)
}
