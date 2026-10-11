use alloc::{string::ToString, vec, vec::Vec};
use alloc::collections::BTreeMap;

use zune_core::bytestream::{ZByteReaderTrait, ZReader, ZSeekFrom};
use zune_core::colorspace::ColorSpace;
use zune_core::log::{trace, warn};
use zune_core::options::DecoderOptions;

use zune_isobmff::{BoxHeader, BoxSize};
use crate::colour::{ColourInfo, Conversion};
use crate::errors::HeicErrors;
use crate::header_structs::{
    ColourInformation, FtypHeader, ItemProperty, MDatSection, MetaSection,
};
use crate::headers::{decode_ftyp, decode_meta};
use crate::hevc_decoder::HevcDecoder;
use crate::hevc_decoder::nal_parser::NalFraming;
use crate::processor::HevcSample;
use crate::transform::{self, CleanAperture, Transform};
use crate::utils::Lock;


/// One coded image (the primary image, or one grid tile) and what zune-heic
/// decoded from it; see [`HeifDecoder::decode_tiles_yuv`].
#[cfg(feature = "dump-tiles")]
pub struct DecodedTile {
    /// The HEIF item id of the coded image
    pub item_id: u32,
    /// Its HEVC bitstream in Annex B form (VPS, SPS, PPS, then the slices),
    /// ready for a stand-alone HEVC decoder
    pub hevc:    Vec<u8>,
    /// The decoded planes, Y then Cb and Cr, each cropped to the conformance
    /// window: what `dec265 -o` writes for the same bitstream
    pub yuv:     Vec<u8>
}

/// A HEIF/HEIC decoder.
///
/// Create one with [`new`](Self::new) or [`new_with_options`](Self::new_with_options)
/// around anything that implements [`ZByteReaderTrait`] (e.g. a
/// [`ZCursor`](zune_core::bytestream::ZCursor) over the file's bytes), then
/// call [`decode`](Self::decode) or [`decode_into`](Self::decode_into).
/// Information about the image (size, colorspace, metadata) is available
/// after [`decode_headers`](Self::decode_headers), which the decode methods
/// call themselves.
///
/// # Example
/// ```no_run
/// use zune_heic::HeifDecoder;
/// use zune_heic::zune_core::bytestream::ZCursor;
///
/// let data = std::fs::read("image.heic").unwrap();
/// let mut decoder = HeifDecoder::new(ZCursor::new(&data));
/// let pixels = decoder.decode().unwrap();
/// ```
pub struct HeifDecoder<T> {
    pub(crate) stream: ZReader<T>,
    pub(crate) ftyp_section: Option<FtypHeader>,
    pub(crate) meta_section: Option<MetaSection>,
    pub(crate) mdat_section: Option<MDatSection>,
    pub(crate) colorspace: Option<ColorSpace>,
    pub(crate) read_headers: bool,
    pub(crate) exif_data: Option<Vec<u8>>,
    pub(crate) icc_data: Option<Vec<u8>>,
    pub(crate) options: DecoderOptions,
    /// The primary image
    pub(crate) primary: Option<ImageLayout>,
    /// Its alpha plane (an auxiliary image), if it has one
    pub(crate) alpha: Option<ImageLayout>,
    /// The colour samples are premultiplied by alpha (`prem` reference)
    pub(crate) alpha_premultiplied: bool,
}

/// How a coded image item is put together: a single HEVC picture, or a grid
/// of them, plus its size and orientation.
#[derive(Clone, Debug)]
pub(crate) struct ImageLayout {
    /// The image item (`grid` or `hvc1`)
    pub item_id:  u32,
    /// The coded pictures (`hvc1` items) in raster order
    pub tile_ids: Vec<u32>,
    /// Columns of the grid (1 for a single picture)
    pub cols:     usize,
    /// Grid tiles are decoded in parallel (std only)
    #[cfg_attr(not(feature = "std"), allow(dead_code))]
    pub is_grid:  bool,
    /// Size of the decoded picture (`ispe`, else the grid's output size)
    pub width:      usize,
    pub height:     usize,
    /// `clap`, `irot` and `imir`, in the order they are applied
    pub transforms: Vec<Transform>,
}

impl ImageLayout {
    /// Size after the transformations
    pub fn output_dimensions(&self) -> (usize, usize) {
        transform::output_size(&self.transforms, (self.width, self.height))
    }
}
impl<T> HeifDecoder<T>
where
    T: ZByteReaderTrait,
{
    /// Create a decoder with the default [`DecoderOptions`].
    ///
    /// Nothing is read until [`decode_headers`](Self::decode_headers) or one
    /// of the decode methods is called.
    ///
    /// # Arguments
    ///  - `stream`: The HEIF/HEIC file, e.g. a [`ZCursor`](zune_core::bytestream::ZCursor)
    ///    over its bytes or a buffered file.
    #[must_use]
    #[allow(clippy::new_without_default)]
    pub fn new(stream: T) -> Self {
        Self::new_with_options(stream, DecoderOptions::default())
    }
    /// Create a decoder with the given options.
    ///
    /// The options used by this decoder are:
    ///  - [`max_width`](DecoderOptions::max_width) / [`max_height`](DecoderOptions::max_height):
    ///    larger images are rejected.
    ///  - [`hevc_max_mdat_size`](DecoderOptions::hevc_max_mdat_size): the
    ///    largest image data (`mdat` box) accepted; it is read into memory.
    ///  - [`num_threads`](DecoderOptions::num_threads): threads used to decode
    ///    the tiles of grid images.
    ///  - [`hvec_use_apple_videotoolbox`](DecoderOptions::hvec_use_apple_videotoolbox):
    ///    decode with Apple's hardware decoder (macOS, `std` feature).
    ///
    /// # Arguments
    ///  - `stream`: The HEIF/HEIC file
    ///  - `options`: Decoding options
    ///
    /// # Example
    /// ```no_run
    /// use zune_heic::HeifDecoder;
    /// use zune_heic::zune_core::bytestream::ZCursor;
    /// use zune_heic::zune_core::options::DecoderOptions;
    ///
    /// let data = std::fs::read("image.heic").unwrap();
    /// // software decoding everywhere, including macOS
    /// let options = DecoderOptions::default().hvec_set_use_videotoolbox(false);
    /// let mut decoder = HeifDecoder::new_with_options(ZCursor::new(&data), options);
    /// let pixels = decoder.decode().unwrap();
    /// ```
    #[allow(clippy::redundant_field_names)]
    pub fn new_with_options(stream: T, options: DecoderOptions) -> Self {
        Self {
            stream: ZReader::new(stream),
            options: options,
            ftyp_section: None,
            mdat_section: None,
            meta_section: None,
            colorspace: None,
            exif_data: None,
            icc_data: None,
            read_headers: false,
            primary: None,
            alpha: None,
            alpha_premultiplied: false,
        }
    }

    /// Parse the file structure: everything except the image data itself.
    ///
    /// Afterwards [`width`](Self::width), [`height`](Self::height),
    /// [`colorspace`](Self::colorspace), [`output_buffer_size`](Self::output_buffer_size),
    /// [`exif_data`](Self::exif_data) and [`icc_data`](Self::icc_data) are
    /// available. Calling it again does nothing; the decode methods call it
    /// themselves.
    ///
    /// # Errors
    /// The file is not a valid HEIF file, uses features this decoder does not
    /// support, is larger than the configured limits, or reading failed.
    ///
    /// # Example
    /// ```no_run
    /// use zune_heic::HeifDecoder;
    /// use zune_heic::zune_core::bytestream::ZCursor;
    ///
    /// let data = std::fs::read("image.heic").unwrap();
    /// let mut decoder = HeifDecoder::new(ZCursor::new(&data));
    /// decoder.decode_headers().unwrap();
    /// println!("{}x{}", decoder.width().unwrap(), decoder.height().unwrap());
    /// ```
    pub fn decode_headers(&mut self) -> Result<(), HeicErrors> {
        if self.read_headers {
            //warn!("Headers already read");
            return Ok(());
        }
        loop {
            let header = BoxHeader::read(&mut self.stream)?;

            match &header.get_box_type().0 {
                b"ftyp" => {
                    let ftype = decode_ftyp(&mut self.stream, &header)?;
                    self.ftyp_section = Some(ftype);
                }
                b"mdat" => {
                    // store mdat option
                    trace!("Found MDAT section :{:?}", header.get_total_size());
                    let payload_start_offset = self.stream.position()?;
                    // skip to the next section
                    match header.get_total_size() {
                        BoxSize::Absolute(total_size) => {
                            let payload_size = total_size.saturating_sub(header.get_header_size());

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
                _ => match header.get_total_size() {
                    BoxSize::Absolute(total_size) => {
                        let payload_size_opt = total_size.checked_sub(header.get_header_size());
                        match payload_size_opt {
                            Some(payload_size) => {
                                self.stream.skip(payload_size as usize)?;
                            }
                            None => {
                                return Err(HeicErrors::Generic {
                                    msg: format!(
                                        "Payload size {total_size} larger than header size {}",
                                        header.get_header_size()
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

        // the primary image: its tiles, size and orientation
        let primary_id = self
            .meta_section
            .as_ref()
            .and_then(|m| m.pitm.as_ref())
            .ok_or(HeicErrors::Generic {
                msg: "no pitm section".to_string(),
            })?
            .item_id;
        let primary = self.read_layout(primary_id)?;
        self.check_dimensions(&primary)?;
        self.primary = Some(primary);
        // its alpha plane, if any
        self.find_alpha(primary_id);
        // Calc colorspace
        self.internal_colorspace()?;
        // Load exif if present
        self.load_exif_data()?;
        // ICC too
        let _ = self.extract_icc_profile_inner();

        self.read_headers = true;

        Ok(())
    }

    /// Four-character type of an item (`hvc1`, `grid`, ...), zeroes if unknown
    pub(crate) fn get_item_type(&self, item_id: u32) -> [u8; 4] {
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
    /// The layout of image item `item_id`: a `grid` of `hvc1` pictures or a
    /// single `hvc1` picture, with its size (`ispe`) and its transformations
    /// (`clap`, `irot`, `imir`, in property order, as libheif applies them).
    fn read_layout(&mut self, item_id: u32) -> Result<ImageLayout, HeicErrors> {
        let meta = self.meta_section.as_ref().ok_or(HeicErrors::Generic {
            msg: "no meta section".to_string(),
        })?;

        // properties: size and transformations
        let (mut width, mut height) = (0, 0);
        // the crop rectangle depends on the size at that point, which is
        // only known once the size is
        let mut steps: Vec<Result<Transform, CleanAperture>> = Vec::new();
        if let Some(iprp) = &meta.iprp
            && let (Some(ipma), Some(ipco)) = (&iprp.ipma, &iprp.ipco)
            && let Some(entry) = ipma.entries.iter().find(|e| e.item_id == item_id)
        {
            // property_index is 1-based, 0 means "none"
            let properties = entry
                .associations
                .iter()
                .filter_map(|a| ipco.properties.get(usize::from(a.property_index).checked_sub(1)?));
            for property in properties {
                match property {
                    ItemProperty::Ispe { width: w, height: h } => {
                        (width, height) = (*w as usize, *h as usize);
                    }
                    ItemProperty::Irot { angle_degrees } => {
                        trace!("Item {item_id} is rotated by {angle_degrees}");
                        if *angle_degrees != 0 {
                            steps.push(Ok(Transform::Rotate(*angle_degrees)));
                        }
                    }
                    ItemProperty::Imir { axis } => {
                        trace!("Item {item_id} is mirrored on axis {axis}");
                        // libheif: axis 0 flips top/bottom, axis 1 left/right
                        steps.push(Ok(if *axis == 0 {
                            Transform::FlipTopBottom
                        } else {
                            Transform::FlipLeftRight
                        }));
                    }
                    ItemProperty::Clap { width, height, horizontal_offset, vertical_offset } => {
                        trace!("Item {item_id} has a clean aperture");
                        steps.push(Err(CleanAperture {
                            width:             *width,
                            height:            *height,
                            horizontal_offset: *horizontal_offset,
                            vertical_offset:   *vertical_offset,
                        }));
                    }
                    _ => {}
                }
            }
        }

        let item_type = self.get_item_type(item_id);
        let layout = match &item_type {
            b"grid" => {
                let meta = self.meta_section.as_ref().unwrap();
                let tile_ids = meta
                    .iref
                    .as_ref()
                    .and_then(|iref| {
                        // 'dimg' links the grid item to its tiles
                        iref.references
                            .iter()
                            .find(|r| &r.reference_type.0 == b"dimg" && r.from_item_id == item_id)
                    })
                    .map(|r| r.to_item_ids.clone())
                    .ok_or(HeicErrors::Generic {
                        msg: "Grid missing 'dimg' relationship".to_string(),
                    })?;

                // the grid descriptor is the item's data
                let grid_item = meta
                    .iloc
                    .as_ref()
                    .ok_or(HeicErrors::Generic {
                        msg: "no iloc section".to_string(),
                    })?
                    .items
                    .iter()
                    .find(|item| item.item_id == item_id)
                    .ok_or(HeicErrors::Generic {
                        msg: "Grid item location not found".into(),
                    })?;
                let extent = grid_item.extents.first().ok_or(HeicErrors::Generic {
                    msg: "Grid item has no data".into(),
                })?;
                let mut offset = grid_item.base_offset + extent.offset;
                // construction method 1: relative to the 'idat' box
                if grid_item.construction_method == 1 {
                    let idat = meta.idat.as_ref().ok_or(HeicErrors::Generic {
                        msg: "Item uses idat construction but idat box not found".into(),
                    })?;
                    offset += idat.position;
                }
                self.stream.seek(ZSeekFrom::Start(offset))?;

                // version, flags, rows - 1, columns - 1, output width and height
                self.stream.skip(1)?;
                let flags = self.stream.read_u8_err()?;
                let rows = usize::from(self.stream.read_u8_err()?) + 1;
                let cols = usize::from(self.stream.read_u8_err()?) + 1;
                let (grid_w, grid_h) = if (flags & 1) == 1 {
                    (
                        self.stream.get_u32_be_err()? as usize,
                        self.stream.get_u32_be_err()? as usize,
                    )
                } else {
                    (
                        usize::from(self.stream.get_u16_be_err()?),
                        usize::from(self.stream.get_u16_be_err()?),
                    )
                };
                trace!("Grid {item_id}: {cols}x{rows} tiles, output {grid_w}x{grid_h}");
                if tile_ids.len() != rows * cols {
                    return Err(HeicErrors::Generic {
                        msg: format!(
                            "Grid has {} tiles but {rows}x{cols} positions",
                            tile_ids.len()
                        ),
                    });
                }
                // `ispe` is mandatory, but fall back to the grid's own size
                if width == 0 || height == 0 {
                    (width, height) = (grid_w, grid_h);
                }
                ImageLayout {
                    item_id,
                    tile_ids,
                    cols,
                    is_grid: true,
                    width,
                    height,
                    transforms: Vec::new(),
                }
            }
            // `hvc1`; other types fail when they are decoded
            _ => ImageLayout {
                item_id,
                tile_ids: vec![item_id],
                cols: 1,
                is_grid: false,
                width,
                height,
                transforms: Vec::new(),
            },
        };
        if layout.width == 0 || layout.height == 0 {
            return Err(HeicErrors::Generic {
                msg: format!("Width or height is zero (w={},h={})", layout.width, layout.height),
            });
        }
        let mut layout = layout;
        let mut size = (layout.width, layout.height);
        for step in steps {
            let t = match step {
                Ok(t) => t,
                Err(clap) => Transform::Crop(clap.crop(size.0, size.1)?),
            };
            size = t.output_size(size);
            layout.transforms.push(t);
        }
        Ok(layout)
    }

    /// Enforce the configured maximum dimensions.
    fn check_dimensions(&self, layout: &ImageLayout) -> Result<(), HeicErrors> {
        if layout.width > self.options.max_width() {
            return Err(HeicErrors::Generic {
                msg: format!(
                    "Width of image {} greater than configured maximum width {}",
                    layout.width,
                    self.options.max_width()
                ),
            });
        }
        if layout.height > self.options.max_height() {
            return Err(HeicErrors::Generic {
                msg: format!(
                    "Height of image {} greater than configured maximum height {}",
                    layout.height,
                    self.options.max_height()
                ),
            });
        }
        trace!("Image size {}x{}", layout.width, layout.height);
        Ok(())
    }

    /// Find the alpha plane of `primary_id`: an auxiliary image (`auxl`
    /// reference) whose `auxC` type is one of the alpha URNs libheif accepts.
    /// An alpha image that cannot be decoded is ignored (with a warning), so
    /// the colour image still decodes.
    fn find_alpha(&mut self, primary_id: u32) {
        const ALPHA_URNS: [&str; 3] = [
            "urn:mpeg:avc:2015:auxid:1",
            "urn:mpeg:hevc:2015:auxid:1",
            "urn:mpeg:mpegB:cicp:systems:auxiliary:alpha",
        ];
        let Some(meta) = self.meta_section.as_ref() else {
            return;
        };
        let (Some(iref), Some(iprp)) = (&meta.iref, &meta.iprp) else {
            return;
        };
        let (Some(ipma), Some(ipco)) = (&iprp.ipma, &iprp.ipco) else {
            return;
        };
        let is_alpha = |item_id: u32| {
            ipma.entries.iter().filter(|e| e.item_id == item_id).any(|entry| {
                entry.associations.iter().any(|a| {
                    let index = usize::from(a.property_index).checked_sub(1);
                    matches!(
                        index.and_then(|i| ipco.properties.get(i)),
                        Some(ItemProperty::AuxC { aux_type, .. })
                            if ALPHA_URNS.contains(&aux_type.trim_end_matches('\0'))
                    )
                })
            })
        };
        let alpha_id = iref
            .references
            .iter()
            .filter(|r| &r.reference_type.0 == b"auxl" && r.to_item_ids.contains(&primary_id))
            .map(|r| r.from_item_id)
            .find(|&id| is_alpha(id));
        // `prem`: the colour image's samples are premultiplied by alpha
        let premultiplied = iref
            .references
            .iter()
            .any(|r| &r.reference_type.0 == b"prem" && r.from_item_id == primary_id);

        let Some(alpha_id) = alpha_id else {
            return;
        };
        let alpha_type = self.get_item_type(alpha_id);
        if &alpha_type != b"hvc1" && &alpha_type != b"grid" {
            warn!("Ignoring alpha image {alpha_id} of unsupported type {alpha_type:?}");
            return;
        }
        match self.read_layout(alpha_id) {
            Ok(layout) => {
                trace!("Alpha plane in item {alpha_id}");
                self.alpha = Some(layout);
                self.alpha_premultiplied = premultiplied;
            }
            Err(e) => warn!("Ignoring alpha image {alpha_id}: {e}"),
        }
    }

    /// Width of the decoded image in pixels, or `None` before
    /// [`decode_headers`](Self::decode_headers).
    ///
    /// This is the width after cropping and rotation, i.e. the width of the pixels
    /// returned by [`decode`](Self::decode).
    pub fn width(&self) -> Option<usize> {
        Some(self.primary.as_ref()?.output_dimensions().0)
    }

    /// Height of the decoded image in pixels, or `None` before
    /// [`decode_headers`](Self::decode_headers).
    ///
    /// This is the height after cropping and rotation, i.e. the height of the pixels
    /// returned by [`decode`](Self::decode).
    pub fn height(&self) -> Option<usize> {
        Some(self.primary.as_ref()?.output_dimensions().1)
    }

    /// Whether the colour samples are premultiplied by alpha (the file has a
    /// `prem` reference), available after [`decode_headers`](Self::decode_headers).
    ///
    /// The decoder returns the samples as stored; divide by alpha to get
    /// straight (unassociated) colour. Always `false` for images without
    /// alpha.
    pub fn is_alpha_premultiplied(&self) -> bool {
        self.alpha.is_some() && self.alpha_premultiplied
    }

    /// Return the number of bytes needed to hold the decoded image,
    /// `width * height * components` of [`colorspace`](Self::colorspace).
    ///
    /// # Returns
    ///  - `Some(usize)`: Minimum size of a buffer for [`decode_into`](Self::decode_into)
    ///  - `None`: Headers were not decoded yet, or the size overflows `usize`
    #[must_use]
    pub fn output_buffer_size(&self) -> Option<usize> {
        let (w, h) = self.primary.as_ref()?.output_dimensions();
        let components = self.colorspace?.num_components();
        w.checked_mul(h)?.checked_mul(components)
    }

    /// Decode the primary image, returning its pixels.
    ///
    /// The pixels are 8-bit and interleaved in the order of
    /// [`colorspace`](Self::colorspace) (usually RGB), row by row,
    /// [`width`](Self::width) × [`height`](Self::height), with any cropping
    /// (`clap`), rotation and mirroring stored in the file already applied. Its size is
    /// [`output_buffer_size`](Self::output_buffer_size).
    ///
    /// To decode into a buffer you already have, use [`decode_into`](Self::decode_into).
    ///
    /// # Errors
    /// Any error of [`decode_headers`](Self::decode_headers), or invalid or
    /// unsupported image data.
    ///
    /// # Example
    /// ```no_run
    /// use zune_heic::HeifDecoder;
    /// use zune_heic::zune_core::bytestream::ZCursor;
    ///
    /// let data = std::fs::read("image.heic").unwrap();
    /// let mut decoder = HeifDecoder::new(ZCursor::new(&data));
    /// let pixels = decoder.decode().unwrap();
    /// let (w, h) = (decoder.width().unwrap(), decoder.height().unwrap());
    /// ```
    pub fn decode(&mut self) -> Result<Vec<u8>, HeicErrors> {
        self.decode_headers()?;
        let size = self.output_buffer_size().ok_or(HeicErrors::Generic {
            msg: "image dimensions overflow usize".to_string(),
        })?;
        let mut output = vec![0; size];
        self.decode_into(&mut output)?;
        Ok(output)
    }

    /// Decode the primary image into `out`.
    ///
    /// `out` must hold at least [`output_buffer_size`](Self::output_buffer_size)
    /// bytes; only that many bytes are written. The pixel layout is the one of
    /// [`decode`](Self::decode).
    ///
    /// # Errors
    /// [`HeicErrors::TooSmallOutput`] if `out` is too small, otherwise as
    /// [`decode`](Self::decode).
    ///
    /// # Example
    /// ```no_run
    /// use zune_heic::HeifDecoder;
    /// use zune_heic::zune_core::bytestream::ZCursor;
    ///
    /// let data = std::fs::read("image.heic").unwrap();
    /// let mut decoder = HeifDecoder::new(ZCursor::new(&data));
    /// decoder.decode_headers().unwrap();
    ///
    /// let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    /// decoder.decode_into(&mut pixels).unwrap();
    /// ```
    pub fn decode_into(&mut self, out: &mut [u8]) -> Result<(), HeicErrors> {
        self.decode_headers()?;

        let expected = self.output_buffer_size().ok_or(HeicErrors::Generic {
            msg: "image dimensions overflow usize".to_string(),
        })?;
        if out.len() < expected {
            return Err(HeicErrors::TooSmallOutput(expected, out.len()));
        }
        let output = &mut out[..expected];

        let primary = self.primary.clone().unwrap();
        let colorspace = self.colorspace.unwrap();
        let channels = colorspace.num_components();

        match (&self.alpha, colorspace) {
            (Some(alpha), ColorSpace::RGBA) => {
                // colour into RGBA (alpha 255), then the alpha plane into every
                // fourth byte
                self.decode_layout(&primary, 4, output, true)?;
                if let Some(plane) = self.decode_alpha(alpha, &primary)? {
                    for (px, a) in output.chunks_exact_mut(4).zip(plane) {
                        px[3] = a;
                    }
                }
            }
            (Some(alpha), ColorSpace::LumaA) => {
                let mut luma = vec![0; expected / 2];
                self.decode_layout(&primary, 1, &mut luma, true)?;
                let plane = self.decode_alpha(alpha, &primary)?;
                let alpha = plane.as_deref().unwrap_or(&[]);
                for (i, (px, y)) in output.chunks_exact_mut(2).zip(luma).enumerate() {
                    px[0] = y;
                    px[1] = alpha.get(i).copied().unwrap_or(255);
                }
            }
            _ => self.decode_layout(&primary, channels, output, true)?,
        }
        Ok(())
    }

    /// Decode the alpha image as a plane (its luma samples, unconverted, as
    /// libheif does) with its own orientation applied. `None` if its size
    /// does not match the colour image's; the alpha is then left opaque.
    fn decode_alpha(
        &self, alpha: &ImageLayout, primary: &ImageLayout,
    ) -> Result<Option<Vec<u8>>, HeicErrors> {
        if alpha.output_dimensions() != primary.output_dimensions() {
            warn!(
                "Alpha image is {:?}, colour image {:?}; ignoring alpha",
                alpha.output_dimensions(),
                primary.output_dimensions()
            );
            return Ok(None);
        }
        let mut plane = vec![0; alpha.width * alpha.height];
        // always in software: hardware decoders return colour, not raw luma
        self.decode_layout(alpha, 1, &mut plane, false)?;
        Ok(Some(plane))
    }

    /// Decode the image described by `layout` into `output`
    /// (`channels` = 1: raw luma samples, 3/4: RGB/RGBA with opaque alpha),
    /// stitching tiles and applying its transformations (`clap`, `irot`,
    /// `imir`).
    fn decode_layout(
        &self, layout: &ImageLayout, channels: usize, output: &mut [u8], allow_hardware: bool,
    ) -> Result<(), HeicErrors> {
        let (w, h) = (layout.width, layout.height);

        #[cfg(all(target_os = "macos", feature = "std"))]
        let use_hardware = allow_hardware && self.options.hvec_use_apple_videotoolbox();
        #[cfg(not(all(target_os = "macos", feature = "std")))]
        let use_hardware = {
            let _ = allow_hardware;
            false
        };

        // Tiles are converted straight into their place on the canvas as soon as
        // they are decoded, so no per-tile RGB copies are kept around. Rotating
        // and cropping need an intermediate canvas; otherwise (no transforms,
        // or only flips, which work in place) the output buffer is the canvas.
        let needs_canvas = layout
            .transforms
            .iter()
            .any(|t| matches!(t, Transform::Rotate(_) | Transform::Crop(_)));
        let mut canvas = if needs_canvas { vec![0u8; w * h * channels] } else { Vec::new() };

        {
            let canvas: &mut [u8] = if needs_canvas { &mut canvas } else { &mut *output };

            // One lock per canvas row: tiles in the same grid row share canvas
            // rows, so they may be written concurrently from different threads.
            let canvas_rows: Vec<Lock<&mut [u8]>> =
                canvas.chunks_mut(w * channels).map(Lock::new).collect();

            let placements = Self::placements(layout);
            let cols = layout.cols.max(1);

            // Apple VideoToolbox: hardware decode, tiles written straight into
            // the same canvas from the decode callback.
            #[cfg(all(target_os = "macos", feature = "std"))]
            if use_hardware {
                trace!("HEVC using apple video toolbox");
                self.decode_hardware_videotoolbox(
                    layout,
                    &canvas_rows,
                    &placements,
                    cols,
                    w,
                    h,
                    channels,
                )?;
            }

            if !use_hardware {
                // colour description from each item's own `colr` (nclx) box
                let item_nclx = self.item_nclx_profiles();
                // conversions built so far, shared by all tiles (usually just one)
                let conversions: Lock<Vec<(ColourInfo, Conversion)>> = Lock::new(Vec::new());
                let row_threads = Self::row_threads(layout);

                self.for_each_picture(layout, |sample: HevcSample| -> Result<(), HeicErrors> {
                    let (frame, tile_w, tile_h, colour) =
                        Self::decode_picture(&sample, row_threads, &item_nclx)?;
                    let conv = conversions.with(|built| {
                        if let Some((_, conv)) = built.iter().find(|(info, _)| *info == colour) {
                            return conv.clone();
                        }
                        let conv = Conversion::new(colour);
                        built.push((colour, conv.clone()));
                        conv
                    });

                    for &index in Self::positions(&placements, sample.item_id)? {
                        let base_x = (index % cols) * tile_w;
                        let base_y = (index / cols) * tile_h;
                        frame.write_into_canvas(&canvas_rows, base_x, base_y, w, h, channels, &conv)?;
                    }
                    Ok(())
                })?;
            }
        }

        transform::apply_interleaved(&layout.transforms, channels, canvas, (w, h), output)
    }

    /// item_id -> grid positions (an item may in theory be referenced more than once)
    fn placements(layout: &ImageLayout) -> BTreeMap<u32, Vec<usize>> {
        let mut placements: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for (index, &item_id) in layout.tile_ids.iter().enumerate() {
            placements.entry(item_id).or_default().push(index);
        }
        placements
    }

    fn positions(placements: &BTreeMap<u32, Vec<usize>>, item_id: u32) -> Result<&[usize], HeicErrors> {
        placements.get(&item_id).map(Vec::as_slice).ok_or(HeicErrors::Generic {
            msg: format!("Decoded tile {item_id} has no grid position"),
        })
    }

    /// Threads for the CTU rows of one picture: grid tiles are already
    /// decoded in parallel (one tile per thread), so only a single picture
    /// spreads its rows over the CPUs.
    fn row_threads(layout: &ImageLayout) -> usize {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        if !layout.is_grid {
            return std::thread::available_parallelism().map_or(1, core::num::NonZero::get);
        }
        let _ = layout;
        1
    }

    /// Run `processor` on every coded picture of `layout`, grid tiles in
    /// parallel when threads are available.
    #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    fn for_each_picture<F>(&self, layout: &ImageLayout, processor: F) -> Result<(), HeicErrors>
    where
        F: Fn(HevcSample) -> Result<(), HeicErrors> + Sync,
    {
        if layout.is_grid {
            trace!("Using parallel sample decoder");
            return self.process_hevc_samples_parallel(layout, processor);
        }
        trace!("Using single threaded sample decoder");
        self.process_hevc_samples(layout, processor)
    }

    /// Run `processor` on every coded picture of `layout` (no threads
    /// without std, or on wasm).
    #[cfg(any(not(feature = "std"), target_arch = "wasm32"))]
    fn for_each_picture<F>(&self, layout: &ImageLayout, processor: F) -> Result<(), HeicErrors>
    where
        F: Fn(HevcSample) -> Result<(), HeicErrors>,
    {
        self.process_hevc_samples(layout, processor)
    }

    /// Decode one coded picture in software. Returns it with its size and
    /// its colour description (like libheif: the item's nclx box, else the
    /// HEVC VUI).
    fn decode_picture(
        sample: &HevcSample, row_threads: usize, item_nclx: &BTreeMap<u32, ColourInfo>,
    ) -> Result<(crate::hevc_decoder::RawFrame, usize, usize, ColourInfo), HeicErrors> {
        fn parameter_set<'a>(set: Option<&'a [u8]>, name: &str) -> Result<&'a [u8], HeicErrors> {
            set.ok_or(HeicErrors::Generic { msg: format!("{name} not found") })
        }

        let mut decoder: HevcDecoder = HevcDecoder::new();
        decoder.set_max_threads(row_threads);
        decoder.parse_extradata(parameter_set(sample.vps.as_deref(), "vps")?, NalFraming::RawBytes)?;
        decoder.parse_extradata(parameter_set(sample.sps.as_deref(), "sps")?, NalFraming::RawBytes)?;
        decoder.parse_extradata(parameter_set(sample.pps.as_deref(), "pps")?, NalFraming::RawBytes)?;

        let frame = decoder.decode(sample)?.ok_or(HeicErrors::Generic {
            msg: "decode failure, no frame found".to_string(),
        })?;
        let colour = item_nclx
            .get(&sample.item_id)
            .copied()
            .unwrap_or_else(|| ColourInfo::from_vui(decoder.vui()));
        Ok((frame, decoder.width(), decoder.height(), colour))
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

        // --- STEP 2: an alpha plane, found by `find_alpha` ---
        let has_alpha_mask = self.alpha.is_some();

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

        // 5. Restore the reader cursor so the rest of the parsing isn't messed up
        self.stream.seek(ZSeekFrom::Start(original_position))?;

        Ok(())
    }

    /// Colorspace of the decoded pixels, or `None` before
    /// [`decode_headers`](Self::decode_headers).
    ///
    /// - [`ColorSpace::RGB`] for colour images,
    /// - [`ColorSpace::Luma`] for greyscale (monochrome) images,
    /// - [`ColorSpace::RGBA`] / [`ColorSpace::LumaA`] when the image has an
    ///   alpha plane. The alpha plane is decoded into the last channel; if
    ///   its size does not match the colour image it is ignored (alpha 255).
    pub fn colorspace(&self) -> Option<ColorSpace> {
        self.colorspace
    }
    /// Decode every coded image of the primary item, and of its alpha image if
    /// it has one, on its own and return its HEVC bitstream together with the
    /// decoded (pre colour conversion) planes, for checking the HEVC decoder
    /// against a reference decoder.
    /// Each image is decoded with as many threads as there are CPUs.
    ///
    /// Only available with the `dump-tiles` feature.
    ///
    /// # Errors
    /// As [`decode`](Self::decode).
    ///
    /// # Example
    /// ```no_run
    /// use zune_heic::HeifDecoder;
    /// use zune_heic::zune_core::bytestream::ZCursor;
    ///
    /// let data = std::fs::read("image.heic").unwrap();
    /// let mut decoder = HeifDecoder::new(ZCursor::new(&data));
    /// for tile in decoder.decode_tiles_yuv().unwrap() {
    ///     // decode `tile.hevc` with e.g. `dec265 -o ref.yuv` and compare with `tile.yuv`
    ///     std::fs::write(format!("{}.hevc", tile.item_id), &tile.hevc).unwrap();
    ///     std::fs::write(format!("{}.yuv", tile.item_id), &tile.yuv).unwrap();
    /// }
    /// ```
    #[cfg(feature = "dump-tiles")]
    pub fn decode_tiles_yuv(&mut self) -> Result<Vec<DecodedTile>, HeicErrors> {
        use crate::hevc_decoder::nal_parser::NalParser;

        self.decode_headers()?;
        // the colour image, then the alpha image (if any)
        let layouts: Vec<ImageLayout> =
            self.primary.iter().chain(self.alpha.iter()).cloned().collect();
        let mut tiles = Vec::new();
        for layout in &layouts {
            self.process_hevc_samples(layout, |sample| {
                let missing = |what: &str| HeicErrors::Generic {
                    msg: format!("{what} not found")
                };
                let parameter_sets = [
                    sample.vps.as_deref().ok_or_else(|| missing("vps"))?,
                    sample.sps.as_deref().ok_or_else(|| missing("sps"))?,
                    sample.pps.as_deref().ok_or_else(|| missing("pps"))?
                ];

                let mut decoder = HevcDecoder::new();
                // all CPUs, so the multi-threaded paths are covered as well
                #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
                decoder.set_max_threads(
                    std::thread::available_parallelism().map_or(1, core::num::NonZero::get)
                );
                let mut hevc = Vec::new();
                for ps in parameter_sets {
                    decoder.parse_extradata(ps, NalFraming::RawBytes)?;
                    hevc.extend_from_slice(&[0, 0, 0, 1]);
                    hevc.extend_from_slice(ps);
                }
                NalParser::new_detect(&sample.extents).for_each_nal(|nal| {
                    // rebuild the 2-byte NAL header in front of the payload
                    hevc.extend_from_slice(&[
                        0,
                        0,
                        0,
                        1,
                        ((nal.nal_type as u8) << 1) | (nal.layer_id >> 5),
                        ((nal.layer_id & 0x1f) << 3) | (nal.temporal_id + 1)
                    ]);
                    hevc.extend_from_slice(nal.payload);
                    Ok(true)
                })?;

                let frame = decoder.decode(&sample)?.ok_or(HeicErrors::Generic {
                    msg: "decode failure, no frame found".to_string()
                })?;
                tiles.push(DecodedTile {
                    item_id: sample.item_id,
                    hevc,
                    yuv: frame.planar_yuv(decoder.conformance_window())
                });
                Ok(())
            })?;
        }
        Ok(tiles)
    }

    /// The `colr` box of type `nclx` of every item that has one.
    fn item_nclx_profiles(&self) -> BTreeMap<u32, ColourInfo> {
        let mut profiles = BTreeMap::new();
        let Some(iprp) = self.meta_section.as_ref().and_then(|m| m.iprp.as_ref()) else {
            return profiles;
        };
        let (Some(ipma), Some(ipco)) = (&iprp.ipma, &iprp.ipco) else {
            return profiles;
        };
        for entry in &ipma.entries {
            // property_index is 1-based, 0 means "none"
            let nclx = entry
                .associations
                .iter()
                .filter_map(|a| ipco.properties.get(usize::from(a.property_index).checked_sub(1)?))
                .find_map(|p| match p {
                    ItemProperty::Colr(c) => ColourInfo::from_colr(c),
                    _ => None,
                });
            if let Some(nclx) = nclx {
                profiles.insert(entry.item_id, nclx);
            }
        }
        profiles
    }

    /// The Exif metadata of the image, if it has any, available after
    /// [`decode_headers`](Self::decode_headers).
    ///
    /// These are the raw TIFF-structured Exif bytes, starting with the byte
    /// order mark (`II` or `MM`), ready for an Exif parser. The HEIF header
    /// in front of them is removed.
    ///
    /// Note that the Exif orientation tag describes the stored image; the
    /// pixels returned by [`decode`](Self::decode) already have the HEIF
    /// cropping, rotation and mirroring applied.
    ///
    /// # Example
    /// ```no_run
    /// use zune_heic::HeifDecoder;
    /// use zune_heic::zune_core::bytestream::ZCursor;
    ///
    /// let data = std::fs::read("image.heic").unwrap();
    /// let mut decoder = HeifDecoder::new(ZCursor::new(&data));
    /// decoder.decode_headers().unwrap();
    /// if let Some(exif) = decoder.exif_data() {
    ///     println!("{} bytes of Exif", exif.len());
    /// }
    /// ```
    pub fn exif_data(&self) -> Option<&Vec<u8>> {
        self.exif_data.as_ref()
    }
    /// The ICC colour profile of the image (a `colr` box of type `prof` or
    /// `rICC`), if it has one, available after
    /// [`decode_headers`](Self::decode_headers).
    ///
    /// The decoded pixels are not converted with this profile; pass it on to
    /// a colour management system or embed it in the output file.
    ///
    /// # Example
    /// ```no_run
    /// use zune_heic::HeifDecoder;
    /// use zune_heic::zune_core::bytestream::ZCursor;
    ///
    /// let data = std::fs::read("image.heic").unwrap();
    /// let mut decoder = HeifDecoder::new(ZCursor::new(&data));
    /// decoder.decode_headers().unwrap();
    /// if let Some(icc) = decoder.icc_data() {
    ///     println!("{} byte ICC profile", icc.len());
    /// }
    /// ```
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
//         crate::dbg_println!("{colorspace:?}");
//         crate::dbg_println!("{:?}", decoder.width().unwrap());
//         crate::dbg_println!("{:?}", decoder.height().unwrap());
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
