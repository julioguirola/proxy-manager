use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use url::Url;

/// Ruta del archivo de entorno del sistema.
pub const ETC_ENV: &str = "/etc/environment";

/// Variables de proxy soportadas (minúsculas y mayúsculas).
pub const PROXY_VAR_NAMES: [&str; 6] = [
    "http_proxy",
    "HTTP_PROXY",
    "https_proxy",
    "HTTPS_PROXY",
    "no_proxy",
    "NO_PROXY",
];

/// Archivos sobre los que actúa el programa (config del shell y /etc/environment).
#[derive(Clone, Debug)]
pub struct Targets {
    pub term: PathBuf,
    pub etc: PathBuf,
}

impl Targets {
    pub fn new(term: PathBuf) -> Self {
        Self {
            term,
            etc: PathBuf::from(ETC_ENV),
        }
    }

    pub fn files(&self) -> [PathBuf; 2] {
        [self.term.clone(), self.etc.clone()]
    }
}

/// Devuelve la ruta de configuración del shell según $SHELL, igual que la versión original.
pub fn shell_config_path() -> PathBuf {
    let shell = std::env::var("SHELL").unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    if shell.contains("zsh") {
        Path::new(&home).join(".zshrc")
    } else if shell.contains("fish") {
        Path::new(&home).join(".config/fish/config.fish")
    } else {
        // bash y cualquier otro shell: .bashrc
        Path::new(&home).join(".bashrc")
    }
}

/// Configuración de proxy (mismos datos que la versión Go, más el esquema).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyConfig {
    pub scheme: String,
    pub user: String,
    pub pass: String,
    pub host: String,
    pub port: u16,
    pub no_proxy: String,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            scheme: "http".to_string(),
            user: String::new(),
            pass: String::new(),
            host: String::new(),
            port: 0,
            no_proxy: String::new(),
        }
    }
}

/// Devuelve el nombre de la variable de proxy que define la línea (sin el
/// prefijo 'export '), o None si la línea no define una variable de proxy.
pub fn proxy_var_name(clean_line: &str) -> Option<&'static str> {
    PROXY_VAR_NAMES
        .iter()
        .copied()
        .find(|name| clean_line.starts_with(&format!("{name}=")))
}

fn is_proxy_url_var(name: &str) -> bool {
    matches!(
        name,
        "http_proxy" | "https_proxy" | "HTTP_PROXY" | "HTTPS_PROXY"
    )
}

fn is_no_proxy_var(name: &str) -> bool {
    matches!(name, "no_proxy" | "NO_PROXY")
}

/// Codifica caracteres especiales para la parte userinfo de una URL.
fn encode_userinfo(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        let c = byte as char;
        if c.is_ascii_alphanumeric() || "-._~!$&'()*+,;=:".contains(c) {
            out.push(c);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Construye la URL completa del proxy tal y como se escribe en los archivos.
pub fn build_url(pc: &ProxyConfig) -> String {
    let scheme = if pc.scheme.is_empty() {
        "http"
    } else {
        pc.scheme.as_str()
    };
    let mut out = format!("{scheme}://");
    if !pc.user.is_empty() || !pc.pass.is_empty() {
        out.push_str(&encode_userinfo(&pc.user));
        if !pc.pass.is_empty() {
            out.push(':');
            out.push_str(&encode_userinfo(&pc.pass));
        }
        out.push('@');
    }
    out.push_str(&pc.host);
    if pc.port != 0 {
        out.push(':');
        out.push_str(&pc.port.to_string());
    }
    out
}

/// Parsea una URL de proxy. Si no trae esquema se asume `http://`.
pub fn parse_url(raw: &str) -> Result<ProxyConfig, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("la URL del proxy está vacía".to_string());
    }
    let invalid = |e: url::ParseError| format!("URL de proxy inválida: {e}");
    let parsed = if raw.contains("://") {
        Url::parse(raw).map_err(invalid)?
    } else {
        match Url::parse(raw) {
            Ok(u) if u.host_str().is_some() => u,
            _ => Url::parse(&format!("http://{raw}")).map_err(invalid)?,
        }
    };
    let host = parsed
        .host_str()
        .filter(|h| !h.is_empty())
        .ok_or_else(|| "la URL del proxy no tiene host".to_string())?
        .to_string();
    Ok(ProxyConfig {
        scheme: parsed.scheme().to_string(),
        user: parsed.username().to_string(),
        pass: parsed.password().unwrap_or_default().to_string(),
        host,
        port: parsed.port().unwrap_or(0),
        no_proxy: String::new(),
    })
}

/// Lee una línea `export VAR=...` o `VAR=...`, quitando comillas del valor.
fn line_value(clean_line: &str) -> Option<String> {
    clean_line
        .split_once('=')
        .map(|(_, value)| value.trim_matches('"').to_string())
}

/// Parsea la configuración de proxy de un archivo. Devuelve None si no hay
/// una URL de proxy válida (misma semántica que parseConfigFromFile de Go).
pub fn parse_config_from_file(path: &Path) -> Option<ProxyConfig> {
    let data = fs::read_to_string(path).ok()?;
    let mut proxy_url: Option<String> = None;
    let mut no_proxy = String::new();

    for line in data.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let clean = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        match proxy_var_name(clean) {
            Some(name) if is_proxy_url_var(name) => {
                proxy_url = line_value(clean);
            }
            Some(name) if is_no_proxy_var(name) => {
                if let Some(value) = line_value(clean) {
                    no_proxy = value;
                }
            }
            _ => {}
        }
    }

    let raw_url = proxy_url?;
    if raw_url.is_empty() {
        return None;
    }
    let mut pc = parse_url(&raw_url).ok()?;
    pc.no_proxy = no_proxy;
    Some(pc)
}

/// Revisa si el proxy está habilitado en todos los archivos indicados.
/// Un archivo cuenta como habilitado si tiene un http(s)_proxy sin comentar
/// y con valor no vacío.
pub fn is_proxy_enabled(files: &[PathBuf]) -> bool {
    let mut both_enabled = true;
    for path in files {
        let Ok(data) = fs::read_to_string(path) else {
            both_enabled = false;
            continue;
        };
        let mut file_has_enabled = false;
        for line in data.split('\n') {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let clean = trimmed.strip_prefix("export ").unwrap_or(trimmed);
            if let Some(name) = proxy_var_name(clean)
                && is_proxy_url_var(name)
                && let Some((_, value)) = clean.split_once('=')
                && !value.is_empty()
            {
                file_has_enabled = true;
                break;
            }
        }
        if !file_has_enabled {
            both_enabled = false;
        }
    }
    both_enabled
}

/// Habilita o deshabilita el proxy en un archivo, reescribiendo las variables
/// de proxy existentes y eliminando otras líneas relacionadas con proxies.
pub fn change_proxy_config(pc: &ProxyConfig, path: &Path, enable: bool) -> io::Result<()> {
    let data = fs::read_to_string(path)?;
    let url_value = format!("\"{}\"", build_url(pc));
    let no_proxy_value = format!("\"{}\"", pc.no_proxy);
    let comment = if enable { "" } else { "# " };

    let mut lines: Vec<String> = data.split('\n').map(str::to_string).collect();
    for line in lines.iter_mut() {
        let mut matched = false;
        for name in PROXY_VAR_NAMES {
            for prefix in [format!("export {name}="), format!("{name}=")] {
                if line.contains(&prefix) {
                    let value = if is_no_proxy_var(name) {
                        &no_proxy_value
                    } else {
                        &url_value
                    };
                    *line = format!("{comment}{prefix}{value}");
                    matched = true;
                    break;
                }
            }
            if matched {
                break;
            }
        }
        // Cualquier otra línea relacionada con proxies se elimina.
        if !matched && (line.contains("proxy") || line.contains("PROXY")) {
            line.clear();
        }
    }

    fs::write(path, lines.join("\n"))
}

/// Añade stubs comentados para las variables de proxy que falten en los
/// archivos, igual que ensureProxyVarsExist de la versión original.
pub fn ensure_proxy_vars_exist(targets: &Targets) {
    for file in targets.files() {
        let Ok(data) = fs::read_to_string(&file) else {
            continue;
        };
        let lines: Vec<&str> = data.split('\n').collect();

        let mut existing: HashSet<&'static str> = HashSet::new();
        for line in &lines {
            let trimmed = line.trim();
            let uncommented = trimmed.strip_prefix("# ").unwrap_or(trimmed);
            let uncommented = uncommented.strip_prefix('#').unwrap_or(uncommented);
            let clean = uncommented.strip_prefix("export ").unwrap_or(uncommented);
            if let Some(name) = proxy_var_name(clean) {
                existing.insert(name);
            }
        }

        let mut new_lines = Vec::new();
        for name in PROXY_VAR_NAMES {
            if existing.contains(name) {
                continue;
            }
            if file == targets.term {
                new_lines.push(format!("# export {name}=\"\""));
            } else {
                new_lines.push(format!("# {name}=\"\""));
            }
        }

        if new_lines.is_empty() {
            continue;
        }

        let mut content = lines.join("\n");
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&new_lines.join("\n"));
        content.push('\n');

        if file == targets.etc {
            // /etc/environment requiere permisos: se copia con pkexec.
            let mut tmp = file.clone().into_os_string();
            tmp.push(".tmp");
            let tmp = PathBuf::from(tmp);
            if fs::write(&tmp, content).is_err() {
                continue;
            }
            let _ = Command::new("pkexec")
                .arg("cp")
                .arg(&tmp)
                .arg(&file)
                .status();
            let _ = fs::remove_file(&tmp);
        } else {
            let _ = fs::write(&file, content);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let bashrc = dir.join(".bashrc");
        let etc_env = dir.join("environment");
        let missing = dir.join("missing");
        (bashrc, etc_env, missing)
    }

    #[test]
    fn test_uppercase_proxy_vars() {
        let dir = tempfile::tempdir().unwrap();
        let (bashrc, etc_env, missing) = setup(dir.path());

        fs::write(
            &bashrc,
            "# mi bashrc\nexport HTTP_PROXY=\"http://old:p@oldproxy:3128\"\n# export HTTPS_PROXY=\"\"\n",
        )
        .unwrap();

        fs::write(
            &etc_env,
            "http_proxy=\"http://old:p@oldproxy:3128\"\n# https_proxy=\"\"\n# HTTPS_PROXY=\"\"\n# no_proxy=\"\"\nNO_PROXY=\"localhost\"\n# HTTP_PROXY=\"\"\n",
        )
        .unwrap();

        // 1) ensure solo sobre bashrc (el segundo path no existe)
        let targets = Targets {
            term: bashrc.clone(),
            etc: missing.clone(),
        };
        ensure_proxy_vars_exist(&targets);

        let out = fs::read_to_string(&bashrc).unwrap();
        for want in [
            "# export http_proxy=\"\"",
            "# export https_proxy=\"\"",
            "# export no_proxy=\"\"",
            "# export NO_PROXY=\"\"",
        ] {
            assert!(out.contains(want), "falta stub {want:?} tras ensure");
        }

        // 2) parse desde mayúsculas
        let pc_bash = parse_config_from_file(&bashrc).expect("no parseó el bashrc");
        assert_eq!(pc_bash.host, "oldproxy");
        assert_eq!(pc_bash.port, 3128);

        let pc_etc = parse_config_from_file(&etc_env).expect("no parseó etcEnv");
        assert_eq!(pc_etc.no_proxy, "localhost");

        // 3) habilitar en ambos archivos
        let pc = ProxyConfig {
            user: "u".to_string(),
            pass: "p".to_string(),
            host: "proxy.test.com".to_string(),
            port: 8080,
            no_proxy: "localhost,127.0.0.1".to_string(),
            ..ProxyConfig::default()
        };
        change_proxy_config(&pc, &bashrc, true).unwrap();
        change_proxy_config(&pc, &etc_env, true).unwrap();

        let data_b = fs::read_to_string(&bashrc).unwrap();
        let data_e = fs::read_to_string(&etc_env).unwrap();
        assert!(data_b.contains("export HTTP_PROXY=\"http://u:p@proxy.test.com:8080\""));
        assert!(data_b.contains("export HTTPS_PROXY=\"http://u:p@proxy.test.com:8080\""));
        assert!(data_e.contains("NO_PROXY=\"localhost,127.0.0.1\""));
        assert!(data_e.contains("HTTP_PROXY=\"http://u:p@proxy.test.com:8080\""));

        assert!(is_proxy_enabled(&[bashrc.clone(), etc_env.clone()]));

        // 4) deshabilitar
        change_proxy_config(&pc, &bashrc, false).unwrap();
        change_proxy_config(&pc, &etc_env, false).unwrap();

        let data_b2 = fs::read_to_string(&bashrc).unwrap();
        let data_e2 = fs::read_to_string(&etc_env).unwrap();
        assert!(!data_b2.contains("\nexport HTTP_PROXY="));
        assert!(data_b2.contains("# export HTTP_PROXY=\"http://u:p@proxy.test.com:8080\""));
        assert!(!data_e2.contains("\nNO_PROXY="));
        assert!(!is_proxy_enabled(&[bashrc, etc_env]));
    }

    #[test]
    fn test_parse_url_sin_esquema() {
        let pc = parse_url("10.12.0.205:3128").unwrap();
        assert_eq!(pc.host, "10.12.0.205");
        assert_eq!(pc.port, 3128);
        assert_eq!(pc.scheme, "http");

        let pc = parse_url("http://jguilora:03090660960@10.12.0.205:3128").unwrap();
        assert_eq!(pc.user, "jguilora");
        assert_eq!(pc.pass, "03090660960");
        assert_eq!(
            build_url(&pc),
            "http://jguilora:03090660960@10.12.0.205:3128"
        );
    }

    #[test]
    fn test_parse_url_invalida() {
        assert!(parse_url("").is_err());
        assert!(parse_url("http://").is_err());
    }

    #[test]
    fn test_archivo_inexistente_no_habilitado() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        assert!(!is_proxy_enabled(&[missing]));
    }
}
