use kindergarten_storybook_server::app::App;
use loco_rs::cli;
use migration::Migrator;
use std::{env, fs, path::Path};

#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    load_dotenv();
    cli::main::<App, Migrator>().await
}

fn load_dotenv() {
    for path in [Path::new(".env"), Path::new("../.env")] {
        let Ok(contents) = fs::read_to_string(path) else {
            continue;
        };
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key.is_empty() || env::var_os(key).is_some() {
                continue;
            }
            let value = unquote_env_value(value.trim());
            // Runs before Tokio starts work; no other threads are reading env yet.
            unsafe {
                env::set_var(key, value);
            }
        }
    }
}

fn unquote_env_value(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}
