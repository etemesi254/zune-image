/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufReader, Cursor};
use zune_core::options::DecoderOptions;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::IntoImage;

pub struct ZuneFile {
    file_path: OsString,
    options:   DecoderOptions
}

impl ZuneFile {
    pub fn new(file_path: OsString, options: DecoderOptions) -> ZuneFile {
        ZuneFile { file_path, options }
    }
}

impl IntoImage for ZuneFile {
    fn into_image(&mut self) -> Result<Image, ImageErrors> {
        // read file
        let fd = BufReader::new(File::open(self.file_path.clone())?);

        Image::read(fd, self.options)
    }
}

pub struct ZuneMem<T: AsRef<[u8]>> {
    source:  T,
    options: DecoderOptions
}
impl<T: AsRef<[u8]>> ZuneMem<T> {
    pub fn new(source: T, options: DecoderOptions) -> ZuneMem<T> {
        ZuneMem { source, options }
    }
}
impl<T: AsRef<[u8]>> IntoImage for ZuneMem<T> {
    fn into_image(&mut self) -> Result<Image, ImageErrors> {
        Image::read(Cursor::new(self.source.as_ref()), self.options)
    }
}
