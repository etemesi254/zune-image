/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use std::fs::File;
use std::io::{BufReader, Cursor, Seek, SeekFrom};
use zune_core::options::DecoderOptions;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::IntoImage;

pub struct ZuneFile {
    file: Option<File>,
    options: DecoderOptions,
}

impl ZuneFile {
    pub fn new(file: File, options: DecoderOptions) -> ZuneFile {
        ZuneFile {
            file: Some(file),
            options,
        }
    }
}

impl IntoImage for ZuneFile {
    fn into_image(&mut self) -> Result<Image, ImageErrors> {
        let mut file = self.file.take().ok_or_else(|| {
            ImageErrors::GenericString("File stream already consumed".to_string())
        })?;

        // Rewind the file back to the beginning!
        // This takes microseconds and avoids a filesystem re-open.
        file.seek(SeekFrom::Start(0))?;

        let fd = BufReader::new(file);
        Image::read(fd, self.options)
    }
}

pub struct ZuneWeb {
    buffer: Option<zune_image::web::WebBuffer>,
    options: DecoderOptions,
}

impl ZuneWeb {
    pub fn new(buffer: zune_image::web::WebBuffer, options: DecoderOptions) -> Self {
        Self {
            buffer: Some(buffer),
            options,
        }
    }
}

impl IntoImage for ZuneWeb {
    fn into_image(&mut self) -> Result<Image, ImageErrors> {
        // Extract the buffer, leaving None in its place
        let buffer = self
            .buffer
            .take()
            .ok_or_else(|| ImageErrors::GenericString("Web stream already consumed".to_string()))?;

        // Pass the live WebBuffer directly to your generic decoder
        Image::read(buffer, self.options)
    }
}
pub struct ZuneMem<T: AsRef<[u8]>> {
    source: T,
    options: DecoderOptions,
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
