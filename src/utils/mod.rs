use std::{env, path::PathBuf};

pub fn get_cwd() -> PathBuf {
    return env::current_dir().unwrap();
}
