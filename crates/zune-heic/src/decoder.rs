use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zune_core::bytestream::{ZByteReaderTrait, ZReader, ZSeekFrom};
use zune_core::colorspace::ColorSpace;
use zune_core::log::trace;
use zune_core::options::DecoderOptions;

use zune_isobmff::{BoxHeader, BoxSize};
use crate::errors::HeicErrors;
use crate::header_structs::{
    ColourInformation, FtypHeader, ItemProperty, MDatSection, MetaSection,
};
use crate::headers::{decode_ftyp, decode_meta};
use crate::hevc_decoder::HevcDecoder;
use crate::hevc_decoder::nal_parser::NalFraming;
use crate::processor::HevcSample;

pub(crate) struct SingleDecodedTile {
    pub pixels: Vec<u8>,
    pub width: usize,
    pub height: usize,
}
pub(crate) type TileMap = Arc<Mutex<HashMap<u32, Result<SingleDecodedTile, HeicErrors>>>>;

/// A HEIF/Heic Decoder Instance
pub struct HeifDecoder<T> {
    pub(crate) stream: ZReader<T>,
    pub(crate) ftyp_section: Option<FtypHeader>,
    pub(crate) meta_section: Option<MetaSection>,
    pub(crate) mdat_section: Option<MDatSection>,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) colorspace: Option<ColorSpace>,
    // Derived from itemproperty::irot
    pub(crate) rotation: Option<u16>,
    // Derived from itemproperty::imir
    pub(crate) mirror: Option<u8>,
    pub(crate) read_headers: bool,
    pub(crate) exif_data: Option<Vec<u8>>,
    pub(crate) icc_data: Option<Vec<u8>>,
    pub(crate) ordered_tile_ids: Vec<u32>,
    pub(crate) rows: u32,
    pub(crate) cols: u32,
    pub(crate) options: DecoderOptions,
    // whether image is grid type, if true, we use parallel decoders
    pub(crate) is_grid: bool,
}
impl<T> HeifDecoder<T>
where
    T: ZByteReaderTrait,
{
    /// Create a new Decoder instance
    ///
    /// # Arguments
    ///  - `stream`: The raw bytes of a heif/heic file.
    #[must_use]
    #[allow(clippy::new_without_default)]
    pub fn new(stream: T) -> Self {
        Self::new_with_options(stream, DecoderOptions::default())
    }
    /// Create a new decoder instance with the ability to define decode options
    ///
    /// # Arguments
    ///  - `options`: Decoding options
    ///  - `stream`: The actual data stream with raw heif/heic file
    #[allow(clippy::redundant_field_names)]
    pub fn new_with_options(stream: T, options: DecoderOptions) -> Self {
        Self {
            stream: ZReader::new(stream),
            options: options,
            ftyp_section: None,
            mdat_section: None,
            meta_section: None,
            width: None,
            height: None,
            rotation: None,
            mirror: None,
            colorspace: None,
            exif_data: None,
            icc_data: None,
            ordered_tile_ids: Vec::new(),
            read_headers: false,
            is_grid: false,
            rows: 0,
            cols: 0,
        }
    }

    pub fn decode_headers(&mut self) -> Result<(), HeicErrors> {
        if self.read_headers {
            //warn!("Headers already read");
            return Ok(());
        }
        loop {
            let header = BoxHeader::read(&mut self.stream)?;

            match &header.box_type.0 {
                b"ftyp" => {
                    let ftype = decode_ftyp(&mut self.stream, &header)?;
                    self.ftyp_section = Some(ftype);
                }
                b"mdat" => {
                    // store mdat option
                    trace!("Found MDAT section :{:?}", header.total_size);
                    let payload_start_offset = self.stream.position()?;
                    // skip to the next section
                    match header.total_size {
                        BoxSize::Absolute(total_size) => {
                            let payload_size = total_size.saturating_sub(header.header_size);

                            if payload_size > self.options.hevc_max_mdat_size() as u64 {
                                return Err(HeicErrors::Generic {
                                    msg: format!(
                                        "MDAT {} size exceeds HEIC max configured size {}",
                                        payload_size,
                                        self.options.hevc_max_mdat_size()
                                    ),
                                });
                            }
                            let mut output = vec![0; payload_size as usize];

                            self.stream.read_exact_bytes(&mut output)?;
                            self.mdat_section = Some(MDatSection {
                                raw_data: output,
                                start_offset: payload_start_offset,
                            });
                        }
                        BoxSize::ToEnd => {
                            // read to end of file/stream
                            let mut output = Vec::with_capacity(1024);
                            self.stream.read_all(&mut output)?;
                            self.mdat_section = Some(MDatSection {
                                raw_data: output,
                                start_offset: payload_start_offset,
                            });
                        }
                    }
                }
                b"meta" => {
                    let meta = decode_meta(&mut self.stream, &header)?;
                    self.meta_section = Some(meta);
                }
                // skip to another section
                _ => match header.total_size {
                    BoxSize::Absolute(total_size) => {
                        let payload_size_opt = total_size.checked_sub(header.header_size);
                        match payload_size_opt {
                            Some(payload_size) => {
                                self.stream.skip(payload_size as usize)?;
                            }
                            None => {
                                return Err(HeicErrors::Generic {
                                    msg: format!(
                                        "Payload size {total_size} larger than header size {}",
                                        header.header_size
                                    ),
                                });
                            }
                        }
                    }
                    BoxSize::ToEnd => {
                        trace!("Skipping to end, found a section with skip_to_end flag");
                        break;
                    }
                },
            }

            if self.stream.eof()? {
                break;
            }
        }
        // at these point we expect some fields to be present otherwise its an
        // invalid file
        if self.meta_section.is_none() {
            return Err(HeicErrors::Generic {
                msg: "no meta section".to_string(),
            });
        }
        if self.mdat_section.is_none() {
            return Err(HeicErrors::Generic {
                msg: "no mdat section".to_string(),
            });
        }

        // load extra items
        self.handle_grid_items()?;
        // Calc width and height
        self.calc_internal_dims_via_ispe()?;
        // Calc colorspace
        self.internal_colorspace()?;
        // Load exif if present
        self.load_exif_data()?;
        // ICC too
        let _ = self.extract_icc_profile_inner();

        self.read_headers = true;

        Ok(())
    }

    pub fn get_item_type(&self, item_id: u32) -> [u8; 4] {
        self.meta_section
            .as_ref()
            .and_then(|meta| meta.iinf.as_ref())
            .and_then(|iinf| {
                iinf.entries
                    .iter()
                    .find(|entry| entry.item_id == item_id)
                    .map(|entry| entry.item_type.0) // .0 because it's usually a FourCC wrapper
            })
            .unwrap_or([0, 0, 0, 0]) // Return null if not found
    }
    fn handle_grid_items(&mut self) -> Result<(), HeicErrors> {
        let meta = self.meta_section.as_ref().ok_or(HeicErrors::Generic {
            msg: "no meta section".to_string(),
        })?;

        let pitm = meta.pitm.as_ref().ok_or(HeicErrors::Generic {
            msg: "no pitm section".to_string(),
        })?;

        if let Some(iref) = &meta.iref {
            for rel in &iref.references {
                // 'dimg' links the Grid item to its constituent Tiles
                if &rel.reference_type.0 == b"dimg" && rel.from_item_id == pitm.item_id {
                    self.ordered_tile_ids = rel.to_item_ids.clone();
                }
            }
        }
        // 1. Find the location of the grid data using iloc
        let grid_item = meta
            .iloc
            .as_ref()
            .ok_or(HeicErrors::Generic {
                msg: "no iloc section".to_string(),
            })?
            .items
            .iter()
            .find(|item| item.item_id == pitm.item_id)
            .ok_or(HeicErrors::Generic {
                msg: "Grid item location not found".into(),
            })?;

        let item_type = self.get_item_type(pitm.item_id);
        if &item_type == b"grid" {
            self.is_grid = true;
            let extent = &grid_item.extents[0];
            let mut final_offset = grid_item.base_offset + extent.offset;

            // Construction Method 1 means the offset is relative to the 'idat' box
            if grid_item.construction_method == 1 {
                let idat_offset = meta.idat.as_ref().ok_or(HeicErrors::Generic {
                    msg: "Item uses idat construction but idat box not found".into(),
                })?;
                final_offset += idat_offset.position;
            }

            self.stream.seek(ZSeekFrom::Start(final_offset))?;

            // 3. Parse the 8-byte Grid Descriptor
            self.stream.skip(1)?;
            let flags = self.stream.read_u8_err()?;

            // The next two bytes are rows and columns (stored as N-1)
            self.rows = u32::from(self.stream.read_u8_err()?) + 1;
            self.cols = u32::from(self.stream.read_u8_err()?) + 1;

            // 4. Parse output width and height
            if (flags & 1) == 1 {
                // 32-bit dimensions
                self.width = Some(self.stream.get_u32_be_err()?);
                self.height = Some(self.stream.get_u32_be_err()?);
            } else {
                // 16-bit dimensions (most common)
                self.width = Some(u32::from(self.stream.get_u16_be_err()?));
                self.height = Some(u32::from(self.stream.get_u16_be_err()?));
            }

            trace!(
                "Grid initialized: {}x{} tiles, target resolution: {}x{}",
                self.cols,
                self.rows,
                self.width.unwrap(),
                self.height.unwrap()
            );
        } else {
            self.rows = 1;
            self.cols = 1;
            self.ordered_tile_ids = vec![pitm.item_id];
        }
        Ok(())
    }
    /// Return the width of the image
    ///
    pub fn width(&self) -> Option<usize> {
        let w = self.width?;
        let h = self.height?;
        let rot = self.rotation.unwrap_or(0);
        if rot == 90 || rot == 270 { Some(h as usize) } else { Some(w as usize) }
    }

    pub fn height(&self) -> Option<usize> {
        let w = self.width?;
        let h = self.height?;
        let rot = self.rotation.unwrap_or(0);
        if rot == 90 || rot == 270 { Some(w as usize) } else { Some(h as usize) }
    }

    pub(crate) fn calc_internal_dims_via_ispe(&mut self) -> Result<(), HeicErrors> {
        let meta = self.meta_section.as_ref().ok_or(HeicErrors::Generic {
            msg: "No meta section parsed".to_string(),
        })?;
        // now try extracting width and height.
        // 1. Ensure the properties sections exist
        let iprp = meta.iprp.as_ref().ok_or(HeicErrors::Generic {
            msg: "no iprp section found".to_string(),
        })?;
        let ipma = iprp.ipma.as_ref().ok_or(HeicErrors::Generic {
            msg: "no ipma section found".to_string(),
        })?;
        let ipco = iprp.ipco.as_ref().ok_or(HeicErrors::Generic {
            msg: "no ipco section found".to_string(),
        })?;
        let pitm = meta.pitm.as_ref().ok_or(HeicErrors::Generic {
            msg: "no pitm section found".to_string(),
        })?;

        let mut final_width = 0;
        let mut final_height = 0;

        // 2. Find the property associations for the primary item
        let primary_ipma_entry = ipma.entries.iter().find(|e| e.item_id == pitm.item_id);

        if let Some(entry) = primary_ipma_entry {
            // 3. Loop through the traits assigned to this item
            for assoc in &entry.associations {
                // CRITICAL: property_index is 1-based in the ISOBMFF spec!
                // We must subtract 1 to safely access our 0-based Rust Vec.
                if assoc.property_index == 0 {
                    continue; // Invalid index per spec, skip it
                }
                let array_index = (assoc.property_index - 1) as usize;

                // 4. Look up the physical property in the ipco container
                if let Some(property) = ipco.properties.get(array_index) {
                    // 5. Check if it's the Image Spatial Extents (ispe)
                    match property {
                        // 1. Grab the Encoded Dimensions
                        ItemProperty::Ispe { width, height } => {
                            final_width = *width;
                            final_height = *height;
                        }
                        // 2. Grab the Rotation (if it exists)
                        ItemProperty::Irot { angle_degrees } => {
                            trace!("Image is rotated on {angle_degrees}");
                            self.rotation = Some(*angle_degrees);
                        }
                        // (Optional) Grab Mirroring
                        ItemProperty::Imir { axis } => {
                            trace!("Image is mirrored on axis: {axis}");
                            self.mirror = Some(*axis);
                        }
                        _ => {}
                    }
                }
            }
        }

        if final_height == 0 || final_width == 0 {
            return Err(HeicErrors::Generic {
                msg: format!("Width or height is zero (w={final_width},h={final_height})"),
            });
        }
        if final_width as usize > self.options.max_width() {
            return Err(HeicErrors::Generic {
                msg: format!(
                    "Width of image {} greater than configured maximum width {}",
                    final_width,
                    self.options.max_width()
                ),
            });
        }
        if final_height as usize > self.options.max_height() {
            return Err(HeicErrors::Generic {
                msg: format!(
                    "Height of image {} greater than configured maximum height {}",
                    final_height,
                    self.options.max_height()
                ),
            });
        }
        self.width = Some(final_width);
        self.height = Some(final_height);

        trace!("Width and height from ispe {final_width}x{final_height}");
        Ok(())
    }

    pub fn decode(&mut self) -> Result<Vec<u8>, HeicErrors> {
        self.decode_headers()?;

        let w = self.width.unwrap() as usize;
        let h = self.height.unwrap() as usize;
        let colors = self.colorspace().unwrap().num_components();

        let mut output = vec![0; w * h * colors];

        #[cfg(target_os = "macos")]
        {
            if self.options.hvec_use_apple_videotoolbox() {
                trace!("HEVC using apple video toolbox");
                // --- APPLE SILICON PATH ---
                let tile_map = self.decode_hardware_videotoolbox()?;

                self.stitch(&tile_map, &mut output)?;
                return Ok(output);
            }
        }
        let tile_map: TileMap = Arc::new(Mutex::new(HashMap::new()));

        let processor = |sample: HevcSample| -> Result<(), HeicErrors> {
            let mut software_decoder: HevcDecoder = HevcDecoder::new();

            let vps = sample.vps.as_deref().ok_or(HeicErrors::Generic {
                msg: "vps not found".to_owned(),
            })?;
            let sps = sample.sps.as_deref().ok_or(HeicErrors::Generic {
                msg: "sps not found".to_owned(),
            })?;
            let pps = sample.pps.as_deref().ok_or(HeicErrors::Generic {
                msg: "pps not found".to_owned(),
            })?;

            software_decoder.parse_extradata(vps, NalFraming::RawBytes)?;
            software_decoder.parse_extradata(sps, NalFraming::RawBytes)?;
            software_decoder.parse_extradata(pps, NalFraming::RawBytes)?;

            let sample_id = sample.item_id;

            // Dynamically grab the REAL tile dimensions
            let tile_w = software_decoder.width();
            let tile_h = software_decoder.height();

            // then decode
            let result = software_decoder.decode(&sample)?;

            // allocate the necessary width and height
            let mut out = vec![0; software_decoder.height() * software_decoder.width() * colors];
            match result {
                Some(frame) => {
                    frame.write_rgb_420(&mut out)?;
                    let tile = SingleDecodedTile {
                        pixels: out,
                        width: tile_w,
                        height: tile_h,
                    };

                    tile_map.lock().unwrap().insert(sample_id, Ok(tile));
                    Ok(())
                }
                None => {
                    return Err(HeicErrors::Generic {
                        msg: "decode failure, no frame found".to_string(),
                    });
                }
            }
        };
        // wasm threads not a thing
        #[cfg(target_arch = "wasm32")]
        {
            trace!("Using single threaded sample processor");

            self.process_hevc_samples(processor)?;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.is_grid {
                trace!("Using parallel sample decoder");
                self.process_hevc_samples_parallel(processor)?;
            } else {
                trace!("Using standard sample decoder, image is not grid so no need for parallel");
                self.process_hevc_samples(processor)?;
            }
        }
        self.stitch(&tile_map, &mut output)?;
        return Ok(output);
    }

    /// Determines the final output color space of the primary image,
    /// accounting for grayscale encoded images and separate Alpha mask channels.
    pub(crate) fn internal_colorspace(&mut self) -> Result<(), HeicErrors> {
        let meta = self.meta_section.as_ref().ok_or(HeicErrors::Generic {
            msg: "No meta section parsed".to_string(),
        })?;

        let pitm = meta.pitm.as_ref().ok_or(HeicErrors::Generic {
            msg: "No primary item (pitm) found".to_string(),
        })?;

        let primary_id = pitm.item_id;

        // Default assumptions
        let mut base_channels = 3; // Assume RGB if no pixi box is present
        let mut has_alpha_mask = false;

        // --- STEP 1: Find base channels via `pixi` on the primary item ---
        if let Some(iprp) = &meta.iprp
            && let (Some(ipma), Some(ipco)) = (&iprp.ipma, &iprp.ipco)
        {
            // Find properties assigned to the primary item
            if let Some(entry) = ipma.entries.iter().find(|e| e.item_id == primary_id) {
                for assoc in &entry.associations {
                    if assoc.property_index == 0 {
                        continue;
                    }
                    let array_index = (assoc.property_index - 1) as usize;

                    if let Some(ItemProperty::Pixi { channels, .. }) =
                        ipco.properties.get(array_index)
                    {
                        base_channels = *channels;
                    }
                }
            }
        }

        // --- STEP 2: Find Alpha channel via `iref` and `auxC` ---
        if let Some(iref) = &meta.iref {
            for ref_entry in &iref.references {
                // Look for an Auxiliary Link pointing TO our primary image
                if &ref_entry.reference_type.0 == b"auxl"
                    && ref_entry.to_item_ids.contains(&primary_id)
                {
                    let auxiliary_item_id = ref_entry.from_item_id;

                    // We found an auxiliary item. Now check if it's an Alpha Mask (auxid:1)
                    if let Some(iprp) = &meta.iprp
                        && let (Some(ipma), Some(ipco)) = (&iprp.ipma, &iprp.ipco)
                        && let Some(entry) =
                            ipma.entries.iter().find(|e| e.item_id == auxiliary_item_id)
                    {
                        for assoc in &entry.associations {
                            if assoc.property_index == 0 {
                                continue;
                            }
                            let array_index = (assoc.property_index - 1) as usize;

                            if let Some(ItemProperty::AuxC { aux_type, .. }) =
                                ipco.properties.get(array_index)
                            {
                                // Trim null terminators just in case they were captured in the string
                                let clean_type = aux_type.trim_end_matches('\0');
                                if clean_type == "urn:mpeg:hevc:2015:auxid:1" {
                                    has_alpha_mask = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        // --- STEP 3: Resolve the final ColorSpace enum ---
        let result = match (base_channels, has_alpha_mask) {
            (1, false) => ColorSpace::Luma,
            (1, true) => ColorSpace::LumaA,
            (3, false) => ColorSpace::RGB,
            (3, true) | (4, _) => ColorSpace::RGBA, // 4 native channels is usually RGBA
            (n, _) => {
                if let Some(nz) = core::num::NonZeroU32::new(u32::from(n)) {
                    ColorSpace::MultiBand(nz)
                } else {
                    ColorSpace::Unknown
                }
            }
        };
        self.colorspace = Some(result);
        Ok(())
    }

    /// Internal method to find and load EXIF data into the struct.
    /// Should be called immediately after the main headers are parsed.
    pub(crate) fn load_exif_data(&mut self) -> Result<(), HeicErrors> {
        let Some(meta) = &self.meta_section else {
            return Ok(());
        };

        let Some(iinf) = &meta.iinf else {
            return Ok(());
        };

        // 1. Find the EXIF item ID
        let exif_item_id = match iinf.entries.iter().find(|e| &e.item_type.0 == b"Exif") {
            Some(infe) => infe.item_id,
            None => return Ok(()), // No Exif in this file
        };

        let Some(iloc) = &meta.iloc else {
            return Ok(());
        };

        // 2. Find the physical location
        let Some(exif_iloc) = iloc.items.iter().find(|i| i.item_id == exif_item_id) else {
            return Ok(());
        };

        if exif_iloc.extents.is_empty() {
            return Ok(());
        }
        let position = &exif_iloc.extents[0];

        // 3. Calculate absolute file offset
        let absolute_offset = match exif_iloc.construction_method {
            0 => exif_iloc.base_offset + position.offset,
            1 => {
                let idat_off = meta.idat.as_ref().ok_or(HeicErrors::Generic {
                    msg: "idat offset required for EXIF but not found".into(),
                })?;
                idat_off.position + exif_iloc.base_offset + position.offset
            }
            _ => {
                return Err(HeicErrors::Generic {
                    msg: "Unsupported construction method".into(),
                });
            }
        };

        // Remember where the reader is currently so we can put it back!
        let original_position = self.stream.position()?;

        // 4. Seek, skip garbage, and read
        self.stream.seek(ZSeekFrom::Start(absolute_offset))?;

        let tiff_header_offset = self.stream.get_u32_be_err()? as usize;
        if tiff_header_offset > 0 {
            self.stream.skip(tiff_header_offset)?;
        }

        let exif_data_length = (position.length as usize)
            .saturating_sub(4)
            .saturating_sub(tiff_header_offset);

        if exif_data_length > 0 {
            trace!("Exif data length: ({exif_data_length} bytes)");
            let mut exif_bytes = vec![0; exif_data_length];
            for i in 0..exif_data_length {
                exif_bytes[i] = self.stream.read_fixed_bytes_or_error::<1>()?[0];
            }
            // STORE IT IN THE STRUCT!
            self.exif_data = Some(exif_bytes);
        }

        // 5. Restore the reader cursor so the rest of your parsing isn't messed up
        self.stream.seek(ZSeekFrom::Start(original_position))?;

        Ok(())
    }

    /// Get the image colorspace
    pub fn colorspace(&self) -> Option<ColorSpace> {
        self.colorspace
    }
    /// Return the image exif data if present
    pub fn exif_data(&self) -> Option<&Vec<u8>> {
        self.exif_data.as_ref()
    }
    pub fn icc_data(&self) -> Option<&Vec<u8>> {
        self.icc_data.as_ref()
    }
    #[allow(clippy::collapsible_if,clippy::collapsible_match)]
    fn extract_icc_profile_inner(&mut self) -> Option<()> {
        let iprp = self.meta_section.as_ref()?.iprp.as_ref()?;

        // 2. Iterate through the properties in 'ipco'
        for item in &iprp.ipco.as_ref().unwrap().properties {
            if let ItemProperty::Colr(c) = item {
                if let ColourInformation::IccProfile {
                    profile_type: _,
                    profile_data,
                } = c {
                    trace!("Icc profile ({} bytes)", profile_data.len());
                    self.icc_data = Some(profile_data.clone());
                    break;
                }
            }
        }
        Some(())
    }
}

// #[cfg(test)]
// mod tests {
//     use std::fs::{File, read};
//     use std::io::Write;
//     use zune_core::bytestream::ZCursor;
//     use zune_core::options::DecoderOptions;
//
//     use crate::decoder::HeifDecoder;
//
//     pub(crate) fn write_ppm(name: &str, data: &[u8], w: usize, h: usize) {
//         // 4. Final Write
//         let mut file = File::create(name).unwrap();
//         file.write_all(format!("P6\n{} {}\n255\n", w, h).as_bytes())
//             .unwrap();
//         file.write_all(&data).unwrap();
//     }
//     #[test]
//     fn test_decoding() {
//         let file = read("/Users/etemesi/Downloads/IMG_4909.HEIC").unwrap();
//         let data = ZCursor::new(file);
//         let opt = DecoderOptions::default().hvec_set_use_videotoolbox(false);
//         let mut decoder = HeifDecoder::new_with_options(data, opt);
//         decoder.decode_headers().unwrap();
//         let colorspace = decoder.colorspace().unwrap();
//         println!("{colorspace:?}");
//         println!("{:?}", decoder.width().unwrap());
//         println!("{:?}", decoder.height().unwrap());
//         let data = decoder.decode().unwrap();
//
//         // 4. Final Write
//         write_ppm(
//             "final_stitched_px.ppm",
//             data.as_ref(),
//             decoder.width().unwrap(),
//             decoder.height().unwrap()
//         );
//     }
// }
