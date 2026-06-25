use std::path::Path;

use serde::Deserialize;

use crate::cli::{DiffArgs, OutputFormat};

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub defaults: Option<Defaults>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Defaults {
    pub format: Option<String>,
    pub fail_on_bytes: Option<u64>,
    pub bin: Option<String>,
}

impl Config {
    pub fn load(repo: &Path) -> Self {
        let path = repo.join(".cargo-regress.toml");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&content).unwrap_or_default()
    }

    /// Apply config defaults to diff args — CLI flags take precedence.
    pub fn apply_to_diff(&self, args: &mut DiffArgs) {
        let Some(ref d) = self.defaults else { return };

        if args.bin.is_none() {
            args.bin.clone_from(&d.bin);
        }

        // Only apply format from config if user didn't pass --format explicitly.
        // We detect "user didn't pass" by checking if it's still the default value.
        if matches!(args.format, OutputFormat::Terminal) {
            if let Some(ref fmt) = d.format {
                args.format = match fmt.as_str() {
                    "json" => OutputFormat::Json,
                    "github" => OutputFormat::Github,
                    "sarif" => OutputFormat::Sarif,
                    "gitlab" => OutputFormat::Gitlab,
                    "html" => OutputFormat::Html,
                    _ => OutputFormat::Terminal,
                };
            }
        }

        if args.fail_on.is_none() {
            if let Some(bytes) = d.fail_on_bytes {
                if bytes > 0 {
                    args.fail_on = Some(format!("{bytes}"));
                }
            }
        }
    }

    #[cfg(test)]
    fn from_str(s: &str) -> Self {
        toml::from_str(s).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_diff_args() -> DiffArgs {
        DiffArgs {
            from: "HEAD~1".to_string(),
            to: "HEAD".to_string(),
            file_from: None,
            file_to: None,
            bin: None,
            lib: false,
            format: OutputFormat::Terminal,
            fail_on: None,
        }
    }

    #[test]
    fn empty_config_applies_no_changes() {
        let cfg = Config::from_str("");
        let mut args = default_diff_args();
        cfg.apply_to_diff(&mut args);
        assert!(args.bin.is_none());
        assert!(args.fail_on.is_none());
        assert!(matches!(args.format, OutputFormat::Terminal));
    }

    #[test]
    fn config_sets_bin() {
        let cfg = Config::from_str(
            r#"[defaults]
bin = "my-app"
"#,
        );
        let mut args = default_diff_args();
        cfg.apply_to_diff(&mut args);
        assert_eq!(args.bin.as_deref(), Some("my-app"));
    }

    #[test]
    fn config_sets_format_json() {
        let cfg = Config::from_str(
            r#"[defaults]
format = "json"
"#,
        );
        let mut args = default_diff_args();
        cfg.apply_to_diff(&mut args);
        assert!(matches!(args.format, OutputFormat::Json));
    }

    #[test]
    fn config_sets_fail_on_bytes() {
        let cfg = Config::from_str(
            r#"[defaults]
fail_on_bytes = 50000
"#,
        );
        let mut args = default_diff_args();
        cfg.apply_to_diff(&mut args);
        assert_eq!(args.fail_on.as_deref(), Some("50000"));
    }

    #[test]
    fn cli_bin_overrides_config() {
        let cfg = Config::from_str(
            r#"[defaults]
bin = "from-config"
"#,
        );
        let mut args = default_diff_args();
        args.bin = Some("from-cli".to_string());
        cfg.apply_to_diff(&mut args);
        assert_eq!(args.bin.as_deref(), Some("from-cli"));
    }

    #[test]
    fn zero_fail_on_bytes_not_applied() {
        let cfg = Config::from_str(
            r#"[defaults]
fail_on_bytes = 0
"#,
        );
        let mut args = default_diff_args();
        cfg.apply_to_diff(&mut args);
        assert!(args.fail_on.is_none());
    }

    #[test]
    fn unknown_format_falls_back_to_terminal() {
        let cfg = Config::from_str(
            r#"[defaults]
format = "unknown_format"
"#,
        );
        let mut args = default_diff_args();
        cfg.apply_to_diff(&mut args);
        assert!(matches!(args.format, OutputFormat::Terminal));
    }
}
