use zune_core::bytestream::ZByteReaderTrait;

use crate::apple_videotoolbox::TileMap;
use crate::decoder::HeifDecoder;
use crate::errors::BmfErrors;
use crate::header_structs::ItemProperty;

pub struct HevcSample<'a> {
    pub item_id: u32,

    // Parameter Sets (Usually extracted from the hvcC property)
    // We use Vec<u8> here because they are tiny and usually need to
    // be reconstructed with Annex B start codes (00 00 00 01) anyway.
    pub vps: Option<Vec<u8>>,
    pub sps: Option<Vec<u8>>,
    pub pps: Option<Vec<u8>>,

    // The zero-copy slices pointing directly into the MDAT buffer!
    // If the item wasn't fragmented, this will just have len() == 1.
    pub extents: Vec<&'a [u8]>
}
impl<T: ZByteReaderTrait> HeifDecoder<T> {
    /// Iterates over HEVC items and passes zero-copy slices to a provided closure.
    pub fn process_hevc_samples<F>(&self, mut callback: F) -> Result<(), BmfErrors>
    where
        F: FnMut(HevcSample<'_>) -> Result<(), BmfErrors>
    {
        let meta = self.meta_section.as_ref().ok_or(BmfErrors::Generic {
            msg: "No meta".into()
        })?;
        let pitm = meta.pitm.as_ref().ok_or(BmfErrors::Generic {
            msg: "No pitm".into()
        })?;
        let iinf = meta.iinf.as_ref().ok_or(BmfErrors::Generic {
            msg: "No iinf".into()
        })?;
        let iloc = meta.iloc.as_ref().ok_or(BmfErrors::Generic {
            msg: "No iloc".into()
        })?;
        let mdat = self.mdat_section.as_ref().ok_or(BmfErrors::Generic {
            msg: "No mdat loaded".into()
        })?;

        let iprp = meta.iprp.as_ref().ok_or(BmfErrors::Generic {
            msg: "Missing iprp".into()
        })?;
        let ipma = iprp.ipma.as_ref().ok_or(BmfErrors::Generic {
            msg: "Missing ipma".into()
        })?;
        let ipco = iprp.ipco.as_ref().ok_or(BmfErrors::Generic {
            msg: "Missing ipco".into()
        })?;

        let mut target_item_ids = Vec::new();

        // 1. Determine which items we need to extract (Grid tiles or Single photo)
        let primary_infe = iinf
            .entries
            .iter()
            .find(|e| e.item_id == pitm.item_id)
            .ok_or(BmfErrors::Generic {
                msg: "Primary item not in iinf".into()
            })?;

        if &primary_infe.item_type.0 == b"grid" {
            let iref = meta.iref.as_ref().ok_or(BmfErrors::Generic {
                msg: "Grid missing iref links".into()
            })?;
            let dimg_ref = iref
                .references
                .iter()
                .find(|r| &r.reference_type.0 == b"dimg" && r.from_item_id == pitm.item_id)
                .ok_or(BmfErrors::Generic {
                    msg: "Grid missing 'dimg' relationship".into()
                })?;

            target_item_ids.extend(&dimg_ref.to_item_ids);
        } else if &primary_infe.item_type.0 == b"hvc1" {
            target_item_ids.push(pitm.item_id);
        } else {
            return Err(BmfErrors::Generic {
                msg: format!(
                    "Primary item is an unsupported type: {:?}",
                    primary_infe.item_type.0
                )
            });
        }

        let get_hvcc_for_item =
            |target_id: u32| -> (Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>) {
                if let Some(entry) = ipma.entries.iter().find(|e| e.item_id == target_id) {
                    for assoc in &entry.associations {
                        if assoc.property_index == 0 {
                            continue;
                        }
                        let array_index = (assoc.property_index - 1) as usize;

                        if let Some(ItemProperty::HvcC { payload }) =
                            ipco.properties.get(array_index)
                        {
                            return extract_hevc_parameter_sets(payload);
                        }
                    }
                }
                (None, None, None)
            };

        // 2. Loop through the targets, slice the data, and trigger the callback
        for item_id in target_item_ids {
            // First, try to get the parameter sets directly from the current item
            let (mut vps, mut sps, mut pps) = get_hvcc_for_item(item_id);

            // --- THE FALLBACK ---
            // If the item doesn't have its own parameter sets, and it is NOT the
            // primary item itself, fall back to checking the primary item's properties.
            if vps.is_none() && sps.is_none() && pps.is_none() && item_id != pitm.item_id {
                let fallback = get_hvcc_for_item(pitm.item_id);
                vps = fallback.0;
                sps = fallback.1;
                pps = fallback.2;
            }

            let item_iloc =
                iloc.items
                    .iter()
                    .find(|i| i.item_id == item_id)
                    .ok_or(BmfErrors::Generic {
                        msg: format!("Item {} missing in iloc", item_id)
                    })?;

            // Prepare our zero-copy vector of slices
            let mut zero_copy_extents = Vec::with_capacity(item_iloc.extents.len());

            for extent in &item_iloc.extents {
                let absolute_offset = match item_iloc.construction_method {
                    0 => item_iloc.base_offset + extent.extent_offset,
                    1 => {
                        let idat_off = meta.idat.as_ref().ok_or(BmfErrors::Generic {
                            msg: "Missing idat offset".into()
                        })?;
                        idat_off.position + item_iloc.base_offset + extent.extent_offset
                    }
                    _ => {
                        return Err(BmfErrors::Generic {
                            msg: "Unsupported construction method".into()
                        });
                    }
                };

                if absolute_offset < mdat.start_offset {
                    return Err(BmfErrors::Generic {
                        msg: "Data offset points to before MDAT payload".into()
                    });
                }

                let buffer_index = (absolute_offset - mdat.start_offset) as usize;
                let length = extent.extent_length as usize;

                if buffer_index + length > mdat.raw_data.len() {
                    return Err(BmfErrors::Generic {
                        msg: "Extent goes out of MDAT bounds!".into()
                    });
                }

                // We take a slice of the existing memory instead of copying!
                let slice = &mdat.raw_data[buffer_index..(buffer_index + length)];
                zero_copy_extents.push(slice);
            }

            let sample = HevcSample {
                item_id,
                vps: vps,
                sps: sps,
                pps: pps,
                extents: zero_copy_extents
            };

            // 3. Yield to the closure
            callback(sample)?;
        }

        Ok(())
    }

    pub fn stitch(&self, tile_map: TileMap, output: &mut [u8]) {
        let final_w = self.width.unwrap();
        let final_h = self.height.unwrap();

        let channels = 3;
        let canvas_stride = final_w * channels;
        let tile_w = 512;
        let tile_h = 512;
        let tile_stride = tile_w * channels;

        let canvas = &mut output[0..(final_w * final_h * channels) as usize];
        let tiles = tile_map.lock().unwrap();

        if self.ordered_tile_ids.len() == 1 {
            // no grid stitching needed. so copy to output directly
            if let Some(tile) = tiles.get(&self.ordered_tile_ids[0]) {
                canvas.copy_from_slice(tile.as_slice());
            }
        } else {
            for (index, &item_id) in self.ordered_tile_ids.iter().enumerate() {
                if let Some(tile_data) = tiles.get(&item_id) {
                    let col = (index as u32) % self.cols;
                    let row = (index as u32) / self.cols;

                    let base_x = col * tile_w;
                    let base_y = row * tile_h;

                    for ty in 0..tile_h {
                        let canvas_y = base_y + ty;
                        if canvas_y >= final_h {
                            break;
                        }

                        if base_x >= final_w {
                            continue;
                        }

                        let copy_width = tile_w.min(final_w - base_x);
                        let len = (copy_width * channels) as usize;

                        let src = (ty * tile_stride) as usize;
                        let dst = (canvas_y * canvas_stride + base_x * channels) as usize;

                        canvas[dst..dst + len].copy_from_slice(&tile_data[src..src + len]);
                    }
                }
            }
        }
    }
}
/// Extracts the raw VPS, SPS, and PPS NAL units from an HEVC Configuration Record.
pub fn extract_hevc_parameter_sets(
    hvcc_payload: &[u8]
) -> (Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>) {
    let mut vps = None;
    let mut sps = None;
    let mut pps = None;

    // The hvcC record has exactly 22 bytes of fixed-length configuration
    // flags before the NAL unit arrays begin.
    if hvcc_payload.len() < 23 {
        return (vps, sps, pps);
    }

    let mut offset = 22;
    let num_arrays = hvcc_payload[offset];
    offset += 1;

    for _ in 0..num_arrays {
        if offset >= hvcc_payload.len() {
            break;
        }

        // The NAL unit type is stored in the lower 6 bits
        let nal_unit_type = hvcc_payload[offset] & 0x3F;
        offset += 1;

        if offset + 2 > hvcc_payload.len() {
            break;
        }
        let num_nalus = u16::from_be_bytes([hvcc_payload[offset], hvcc_payload[offset + 1]]);
        offset += 2;

        for _ in 0..num_nalus {
            if offset + 2 > hvcc_payload.len() {
                break;
            }
            let nalu_length =
                u16::from_be_bytes([hvcc_payload[offset], hvcc_payload[offset + 1]]) as usize;
            offset += 2;

            if offset + nalu_length > hvcc_payload.len() {
                break;
            }

            // Extract the pure NAL unit bytes!
            let nalu = hvcc_payload[offset..offset + nalu_length].to_vec();
            offset += nalu_length;

            // HEVC NAL Unit Types: 32 = VPS, 33 = SPS, 34 = PPS
            match nal_unit_type {
                32 => {
                    if vps.is_none() {
                        vps = Some(nalu);
                    }
                }
                33 => {
                    if sps.is_none() {
                        sps = Some(nalu);
                    }
                }
                34 => {
                    if pps.is_none() {
                        pps = Some(nalu);
                    }
                }
                _ => {} // We ignore SEI messages or other arrays for basic decoding
            }
        }
    }

    (vps, sps, pps)
}
