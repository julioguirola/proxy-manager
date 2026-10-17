use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};

use crate::proxy::{ProxyConfig, build_url};

/// Entrada del historial. Conserva el mismo formato JSON que la versión Go
/// para poder leer las configuraciones ya guardadas.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HistoryEntry {
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub pass: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub no_proxy: String,
    #[serde(default = "default_scheme")]
    pub scheme: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub files: String,
}

fn default_scheme() -> String {
    "http".to_string()
}

impl HistoryEntry {
    /// Crea una entrada a partir de la configuración actual del formulario.
    pub fn from_proxy_config(pc: &ProxyConfig) -> Self {
        Self {
            timestamp: chrono::Local::now().to_rfc3339_opts(SecondsFormat::Secs, false),
            user: pc.user.clone(),
            pass: pc.pass.clone(),
            url: pc.host.clone(),
            port: pc.port,
            no_proxy: pc.no_proxy.clone(),
            scheme: pc.scheme.clone(),
            action: String::new(),
            files: String::new(),
        }
    }

    pub fn to_proxy_config(&self) -> ProxyConfig {
        ProxyConfig {
            scheme: if self.scheme.is_empty() {
                "http".to_string()
            } else {
                self.scheme.clone()
            },
            user: self.user.clone(),
            pass: self.pass.clone(),
            host: self.url.clone(),
            port: self.port,
            no_proxy: self.no_proxy.clone(),
        }
    }

    /// Texto que se muestra en la lista del historial.
    pub fn display_label(&self) -> String {
        let host = if self.port != 0 {
            format!("{}:{}", self.url, self.port)
        } else {
            self.url.clone()
        };
        if self.user.is_empty() {
            host
        } else {
            format!("{} | {}", self.user, host)
        }
    }

    pub fn url_string(&self) -> String {
        build_url(&self.to_proxy_config())
    }
}

fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| {
        let user = std::env::var("USER").unwrap_or_default();
        format!("/home/{user}")
    })
}

pub fn history_file_path() -> PathBuf {
    Path::new(&home_dir()).join(".proxy-manager-history.json")
}

pub fn settings_file_path() -> PathBuf {
    Path::new(&home_dir()).join(".proxy-manager-settings.json")
}

pub fn load_history() -> Vec<HistoryEntry> {
    let Ok(data) = fs::read(history_file_path()) else {
        return Vec::new();
    };
    serde_json::from_slice(&data).unwrap_or_default()
}

pub fn save_history(history: &[HistoryEntry]) -> io::Result<()> {
    let data = serde_json::to_string_pretty(history).map_err(io::Error::other)?;
    fs::write(history_file_path(), data)
}

pub fn add_history_entry(entry: &HistoryEntry) -> io::Result<()> {
    let mut history = load_history();
    history.push(entry.clone());
    save_history(&history)
}

/// Ajustes de la aplicación. Se conservan las mismas claves JSON que la
/// versión Go (window_width/window_height no se usan en la TUI).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_window_width")]
    pub window_width: f64,
    #[serde(default = "default_window_height")]
    pub window_height: f64,
    #[serde(default = "default_split_offset")]
    pub split_offset: f64,
}

fn default_window_width() -> f64 {
    800.0
}

fn default_window_height() -> f64 {
    520.0
}

fn default_split_offset() -> f64 {
    0.35
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            window_width: default_window_width(),
            window_height: default_window_height(),
            split_offset: default_split_offset(),
        }
    }
}

pub fn load_settings() -> Settings {
    let Ok(data) = fs::read(settings_file_path()) else {
        return Settings::default();
    };
    serde_json::from_slice(&data).unwrap_or_default()
}

pub fn save_settings(settings: &Settings) -> io::Result<()> {
    let data = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    fs::write(settings_file_path(), data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formato_compatible_con_version_go() {
        let json = r#"[
          {
            "timestamp": "2026-09-25T10:15:39-04:00",
            "user": "jguilora",
            "pass": "03090660960",
            "url": "10.12.0.205",
            "port": 3128,
            "no_proxy": ".xetid.cu,localhost",
            "action": "",
            "files": ""
          }
        ]"#;
        let history: Vec<HistoryEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(history.len(), 1);
        let entry = &history[0];
        assert_eq!(
            entry.url_string(),
            "http://jguilora:03090660960@10.12.0.205:3128"
        );

        // Al reserializar se mantienen las claves originales.
        let out = serde_json::to_string(&entry).unwrap();
        for key in [
            "timestamp",
            "user",
            "pass",
            "url",
            "port",
            "no_proxy",
            "action",
            "files",
        ] {
            assert!(out.contains(&format!("\"{key}\"")), "falta la clave {key}");
        }
    }

    #[test]
    fn ajustes_por_defecto() {
        let s = Settings::default();
        assert_eq!(s.split_offset, 0.35);
    }
}
