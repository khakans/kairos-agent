//! Read provider configuration without mutating the process environment after threads start.
use std::{collections::HashMap, path::Path};
use zeroize::Zeroizing;

type Config = HashMap<String, Zeroizing<String>>;

pub fn load() -> Result<Config, String> {
    let cwd = std::env::current_dir().map_err(|_| "Cannot locate configuration directory")?;
    let path = cwd
        .ancestors()
        .map(|dir| dir.join(".env"))
        .find(|path| path.is_file())
        .or_else(|| {
            std::env::current_exe()
                .ok()?
                .parent()
                .map(|dir| dir.join(".env"))
                .filter(|path| path.is_file())
        });
    match path {
        Some(path) => read(&path),
        None => Ok(Config::new()),
    }
}

fn read(path: &Path) -> Result<Config, String> {
    let source = Zeroizing::new(std::fs::read_to_string(path).map_err(|_| "Cannot read .env")?);
    dotenvy::from_read_iter(source.trim_start_matches('\u{feff}').as_bytes())
        .map(|item| {
            item.map(|(key, value)| (key, Zeroizing::new(value)))
                // Parser errors can contain the original line, including credentials.
                .map_err(|_| "Invalid .env syntax; check KEY=value entries and quotes".into())
        })
        .collect()
}

pub fn value(config: &Config, key: &str) -> String {
    std::env::var(key)
        .ok()
        .or_else(|| config.get(key).map(|v| v.to_string()))
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_windows_dotenv_and_redacts_parse_errors() {
        let path = std::env::temp_dir().join(format!("kairos-env-{}", uuid::Uuid::new_v4()));
        std::fs::write(
            &path,
            "\u{feff}KAIROS_CONFIG_TEST='quoted # value'\r\nEMPTY=\r\nPATH='file-value'\r\n",
        )
        .unwrap();
        let config = read(&path).unwrap();
        assert_eq!(value(&config, "KAIROS_CONFIG_TEST"), "quoted # value");
        assert_eq!(value(&config, "EMPTY"), "");
        assert_eq!(
            value(&config, "PATH"),
            std::env::var("PATH").unwrap().trim()
        );
        std::fs::write(&path, "SECRET='private-unclosed-value").unwrap();
        assert_eq!(
            read(&path).unwrap_err(),
            "Invalid .env syntax; check KEY=value entries and quotes"
        );
        std::fs::remove_file(path).unwrap();
    }
}
