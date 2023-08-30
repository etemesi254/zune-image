/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::ops::Deref;

use log::info;
use memmap2::Mmap;
use zune_core::options::DecoderOptions;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::IntoImage;

pub struct ZuneFile {
    file_path: OsString,
    use_mmap:  bool,
    options:   DecoderOptions
}

impl ZuneFile {
    pub fn new(file_path: OsString, use_mmap: bool, options: DecoderOptions) -> ZuneFile {
        ZuneFile {
            file_path,
            use_mmap,
            options
        }
    }
}

impl IntoImage for ZuneFile {
    fn into_image(self) -> Result<Image, ImageErrors> {
        // read file
        let mut fd = File::open(self.file_path)?;
        let mmap = unsafe { Mmap::map(&fd)? };

        let mut buf = Vec::with_capacity((1 << 20) * usize::from(!self.use_mmap));

        // Decide how we are reading files
        // this has to be here due to Rust ownership rules, etc etc
        let data = {
            if self.use_mmap {
                info!("Reading file via memory maps");
                mmap.deref()
            } else {
                info!("Reading file to memory");
                fd.read_to_end(&mut buf)?;
                &buf
            }
        };

        Image::read(data, self.options)
    }
}
