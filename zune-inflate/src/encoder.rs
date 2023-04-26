/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::vec;
use alloc::vec::Vec;

use crate::constants::DEFLATE_BLOCKTYPE_UNCOMPRESSED;
use crate::utils::calc_adler_hash;

#[derive(Debug, Copy, Clone)]
pub enum DeflateEncodingStrategy
{
    NoCompression
}

impl DeflateEncodingStrategy
{
    fn to_level(self) -> u8
    {
        match self
        {
            Self::NoCompression => 0
        }
    }
}

pub struct DeflateEncodingOptions
{
    strategy: DeflateEncodingStrategy
}

impl Default for DeflateEncodingOptions
{
    fn default() -> Self
    {
        DeflateEncodingOptions {
            strategy: DeflateEncodingStrategy::NoCompression
        }
    }
}

pub struct DeflateEncoder<'a>
{
    data:            &'a [u8],
    options:         DeflateEncodingOptions,
    output_position: usize,
    input_position:  usize,
    output:          Vec<u8>
}

impl<'a> DeflateEncoder<'a>
{
    /// Create a new deflate encoder.
    ///
    /// The
    pub fn new(data: &'a [u8]) -> DeflateEncoder<'a>
    {
        DeflateEncoder::new_with_options(data, DeflateEncodingOptions::default())
    }
    pub fn new_with_options(data: &'a [u8], options: DeflateEncodingOptions) -> DeflateEncoder<'a>
    {
        let length = data.len() + 1024;
        let out_array = vec![0; length];

        DeflateEncoder {
            data,
            options,
            output_position: 0,
            input_position: 0,
            output: out_array
        }
    }

    fn write_zlib_header(&mut self)
    {
        const ZLIB_CM_DEFLATE: u16 = 8;
        const ZLIB_CINFO_32K_WINDOW: u16 = 7;

        let level_hint = self.options.strategy.to_level();

        let mut hdr = (ZLIB_CM_DEFLATE << 8) | (ZLIB_CINFO_32K_WINDOW << 12);

        hdr |= u16::from(level_hint) << 6;
        hdr |= 31 - (hdr % 31);

        self.output[self.output_position..self.output_position + 2]
            .copy_from_slice(&hdr.to_be_bytes());
    }
    /// Encode a deflate data block with no compression
    fn encode_no_compression(&mut self)
    {
        /*
         * If the input is zero-length, we still must output a block in order
         * for the output to be a valid DEFLATE stream.  Handle this case
         * specially to avoid potentially passing NULL to memcpy() below.
         */
        if self.data.is_empty()
        {
            /* BFINAL and BTYPE */
            self.output[self.output_position] = (1 | (DEFLATE_BLOCKTYPE_UNCOMPRESSED << 1)) as u8;
            self.output_position += 1;
            /* LEN and NLEN */
            let num: u32 = 0xFFFF0000;
            self.output[self.output_position..self.output_position + 4]
                .copy_from_slice(&num.to_le_bytes());
            self.output_position += 4;
            return;
        }
        loop
        {
            let mut bfinal = 0;
            let mut len = usize::from(u16::MAX);

            if self.data.len() - self.input_position <= usize::from(u16::MAX)
            {
                bfinal = 1;
                len = self.data.len() - self.input_position;
            }
            /*
             * Output BFINAL and BTYPE.  The stream is already byte-aligned
             * here, so this step always requires outputting exactly 1 byte.
             */
            self.output[self.output_position] =
                (bfinal | (DEFLATE_BLOCKTYPE_UNCOMPRESSED << 1)) as u8;

            self.output_position += 1;
            // output len and nlen
            let len_u16 = len as u16;

            self.output[self.output_position..self.output_position + 2]
                .copy_from_slice(&len_u16.to_le_bytes());
            self.output_position += 2;

            self.output[self.output_position..self.output_position + 2]
                .copy_from_slice(&(!len_u16).to_le_bytes());
            self.output_position += 2;

            // copy from input to output
            self.output[self.output_position..self.output_position + len]
                .copy_from_slice(&self.data[self.input_position..self.input_position + len]);
            self.output_position += len;
            self.input_position += len;

            if self.input_position == self.data.len()
            {
                break;
            }
        }
    }

    pub fn encode_zlib(&mut self) -> Vec<u8>
    {
        let extra = 40 * ((self.data.len() + 41) / 40);
        self.output = vec![0_u8; self.data.len() + extra];
        self.write_zlib_header();
        self.output_position = 2;

        match self.options.strategy
        {
            DeflateEncodingStrategy::NoCompression =>
            {
                self.encode_no_compression();
            }
        }
        // add adler hash
        let hash = calc_adler_hash(self.data);
        self.output[self.output_position..self.output_position + 4]
            .copy_from_slice(&hash.to_be_bytes());
        self.output_position += 4;

        self.output.truncate(self.output_position);

        core::mem::take(&mut self.output)
    }
}
