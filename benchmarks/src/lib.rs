/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::path::{Path, PathBuf};

/// Get the parent directory from which this
/// crate is compiled from
pub fn sample_path() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"));
    // get parent path
    path.parent().unwrap().to_owned()
}
