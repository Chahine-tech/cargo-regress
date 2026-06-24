use std::path::Path;

use anyhow::{Context, Result};
use object::{Object, ObjectSection, ObjectSymbol};

use super::symbol::SymbolEntry;

pub fn parse_symbols(path: &Path) -> Result<Vec<SymbolEntry>> {
    let data =
        std::fs::read(path).with_context(|| format!("Cannot read binary: {}", path.display()))?;

    let file = object::File::parse(&*data)
        .with_context(|| format!("Cannot parse binary format: {}", path.display()))?;

    // Collect (address, size_hint, name, section_name) for all named symbols.
    // size_hint is the explicit size from the symbol table (ELF/PE COFF);
    // it is 0 for Mach-O, which has no size field.
    let mut raw: Vec<(u64, u64, String, String)> = Vec::new();

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

        raw.push((symbol.address(), symbol.size(), name, section_name));
    }

    if raw.is_empty() {
        // PE release builds strip COFF symbols by default (they go to .pdb).
        if file.format() == object::BinaryFormat::Pe {
            eprintln!(
                "⚠ No symbols found in PE binary. MSVC release builds strip COFF \
                 symbols by default. Rebuild with /debugtype:cv,pdata or use the \
                 GNU/MinGW toolchain for embedded symbols."
            );
        }
        return Ok(Vec::new());
    }

    // Sort by address so we can fall back to address-difference sizing.
    raw.sort_unstable_by_key(|(addr, _, _, _)| *addr);

    let mut symbols: Vec<SymbolEntry> = Vec::with_capacity(raw.len());

    for i in 0..raw.len() {
        let (addr, size_hint, ref name, ref section) = raw[i];

        // Prefer the explicit size when the format provides it (ELF, some PE COFF).
        // Fall back to address-difference for Mach-O and unsized entries.
        let size = if size_hint > 0 {
            size_hint
        } else if i + 1 < raw.len() {
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
