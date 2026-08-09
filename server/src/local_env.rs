use std::{env, fs, path::Path};

pub fn load_local_env_files() {
    // 优先级：命令行环境变量 > .env.local > .env。
    // 两类文件都不覆盖已存在的环境变量；.env.local 先加载，使其优先于 .env 生效。
    for path in [".env.local", "../.env.local"] {
        load_env_file(path);
    }
    for path in [".env", "../.env"] {
        load_env_file(path);
    }
}

fn load_env_file(path: impl AsRef<Path>) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || key.starts_with('#') {
            continue;
        }
        if env::var_os(key).is_some() {
            continue;
        }

        let value = clean_env_value(value.trim());
        // The server is still single-threaded at process bootstrap here; load local
        // development secrets before Loco initializes workers and HTTP state.
        unsafe {
            env::set_var(key, value);
        }
    }
}

fn clean_env_value(value: &str) -> String {
    let without_comment = value
        .split_once(" #")
        .map(|(value, _)| value)
        .unwrap_or(value)
        .trim();
    without_comment
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            without_comment
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(without_comment)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::clean_env_value;
    use std::env;

    #[test]
    fn env_value_cleaner_trims_quotes_and_comments() {
        assert_eq!(clean_env_value(" value # comment"), "value");
        assert_eq!(clean_env_value("\"quoted value\""), "quoted value");
        assert_eq!(clean_env_value("'quoted value'"), "quoted value");
    }

    #[test]
    fn env_file_does_not_override_existing_process_vars() {
        let key = "KINDLEAF_TEST_NO_OVERRIDE";
        unsafe { env::set_var(key, "from-process") };
        let dir = env::temp_dir().join(format!("kindleaf-env-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(".env.local");
        std::fs::write(&file, format!("{key}=from-file\n")).unwrap();

        super::load_env_file(&file);

        assert_eq!(env::var(key).unwrap(), "from-process");
        unsafe { env::remove_var(key) };
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn env_file_sets_missing_vars() {
        let key = "KINDLEAF_TEST_SET_MISSING";
        unsafe { env::remove_var(key) };
        let dir = env::temp_dir().join(format!("kindleaf-env-test-set-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(".env");
        std::fs::write(&file, format!("{key}=\"from-file\"\n")).unwrap();

        super::load_env_file(&file);

        assert_eq!(env::var(key).unwrap(), "from-file");
        unsafe { env::remove_var(key) };
        let _ = std::fs::remove_dir_all(&dir);
    }
}
