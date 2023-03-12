use std::path::{Path, PathBuf};

pub fn sample_path() -> PathBuf
{
    let path = Path::new(env!("CARGO_MANIFEST_DIR"));
    // get parent path
    path.parent().unwrap().to_owned()
}
