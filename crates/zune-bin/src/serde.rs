/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::ffi::OsString;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use zune_image::metadata::ImageMetadata;

pub struct Metadata<'a> {
    file:     OsString,
    size:     u64,
    metadata: &'a ImageMetadata
}

impl<'a> Metadata<'a> {
    pub fn new(file: OsString, size: u64, metadata: &ImageMetadata) -> Metadata {
        Metadata {
            file,
            size,
            metadata
        }
    }
}

impl<'a> Serialize for Metadata<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        let mut state = serializer.serialize_struct("ImageMetadata", 2)?;

        state.serialize_field("file", &self.file.to_string_lossy())?;
        state.serialize_field("length", &self.size)?;

        state.serialize_field("metadata", &self.metadata)?;

        state.end()
    }
}
