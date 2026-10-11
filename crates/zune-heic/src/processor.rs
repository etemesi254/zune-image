use alloc::vec::Vec;

use zune_core::bytestream::ZByteReaderTrait;

use crate::decoder::{HeifDecoder, ImageLayout};
use crate::errors::HeicErrors;
use crate::header_structs::ItemProperty;

pub struct ParameterSets {
    pub vps: Option<Vec<u8>>,
    pub sps: Option<Vec<u8>>,
    pub pps: Option<Vec<u8>>
}
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
    /// The coded pictures of `layout`, in raster order, as zero-copy slices
    /// of the image data plus their parameter sets.
    fn collect_samples(&self, layout: &ImageLayout) -> Result<Vec<HevcSample<'_>>, HeicErrors> {
        let meta = self.meta_section.as_ref().ok_or(HeicErrors::Generic {
            msg: "No meta".into()
        })?;
        let iloc = meta.iloc.as_ref().ok_or(HeicErrors::Generic {
            msg: "No iloc".into()
        })?;
        let mdat = self.mdat_section.as_ref().ok_or(HeicErrors::Generic {
            msg: "No mdat loaded".into()
        })?;
        let iprp = meta.iprp.as_ref().ok_or(HeicErrors::Generic {
            msg: "Missing iprp".into()
        })?;
        let ipma = iprp.ipma.as_ref().ok_or(HeicErrors::Generic {
            msg: "Missing ipma".into()
        })?;
        let ipco = iprp.ipco.as_ref().ok_or(HeicErrors::Generic {
            msg: "Missing ipco".into()
        })?;

        let get_hvcc_for_item = |target_id: u32| -> Option<ParameterSets> {
            let entry = ipma.entries.iter().find(|e| e.item_id == target_id)?;
            entry.associations.iter().find_map(|assoc| {
                let index = usize::from(assoc.property_index).checked_sub(1)?;
                match ipco.properties.get(index) {
                    Some(ItemProperty::HvcC { payload }) => Some(extract_hevc_parameter_sets(payload)),
                    _ => None
                }
            })
        };

        let mut samples = Vec::with_capacity(layout.tile_ids.len());
        for &item_id in &layout.tile_ids {
            let item_type = self.get_item_type(item_id);
            if &item_type != b"hvc1" {
                return Err(HeicErrors::Generic {
                    msg: format!(
                        "Unsupported image item type {:?}",
                        core::str::from_utf8(&item_type).unwrap_or("????")
                    )
                });
            }
            // The item's own parameter sets; tiles without them fall back to
            // the ones of the image (grid) item.
            let parameter_sets = get_hvcc_for_item(item_id)
                .filter(|ps| ps.vps.is_some() || ps.sps.is_some() || ps.pps.is_some())
                .or_else(|| (item_id != layout.item_id).then(|| get_hvcc_for_item(layout.item_id)).flatten())
                .unwrap_or(ParameterSets {
                    vps: None,
                    sps: None,
                    pps: None
                });

            let item_iloc =
                iloc.items
                    .iter()
                    .find(|i| i.item_id == item_id)
                    .ok_or(HeicErrors::Generic {
                        msg: format!("Item {item_id} missing in iloc")
                    })?;

            // zero-copy slices of the item's data
            let mut extents = Vec::with_capacity(item_iloc.extents.len());
            for extent in &item_iloc.extents {
                let absolute_offset = match item_iloc.construction_method {
                    0 => item_iloc.base_offset + extent.offset,
                    1 => {
                        let idat_off = meta.idat.as_ref().ok_or(HeicErrors::Generic {
                            msg: "Missing idat offset".into()
                        })?;
                        idat_off.position + item_iloc.base_offset + extent.offset
                    }
                    _ => {
                        return Err(HeicErrors::Generic {
                            msg: "Unsupported construction method".into()
                        });
                    }
                };
                let start = absolute_offset
                    .checked_sub(mdat.start_offset)
                    .ok_or(HeicErrors::Generic {
                        msg: "Data offset points to before MDAT payload".into()
                    })? as usize;
                let slice = start
                    .checked_add(extent.length as usize)
                    .and_then(|end| mdat.raw_data.get(start..end))
                    .ok_or(HeicErrors::Generic {
                        msg: "Extent goes out of MDAT bounds!".into()
                    })?;
                extents.push(slice);
            }

            samples.push(HevcSample {
                item_id,
                vps: parameter_sets.vps,
                sps: parameter_sets.sps,
                pps: parameter_sets.pps,
                extents
            });
        }
        Ok(samples)
    }

    /// Pass the coded pictures of `layout` to `callback`, one after the other.
    pub(crate) fn process_hevc_samples<F>(
        &self, layout: &ImageLayout, mut callback: F
    ) -> Result<(), HeicErrors>
    where
        F: FnMut(HevcSample<'_>) -> Result<(), HeicErrors>
    {
        for sample in self.collect_samples(layout)? {
            callback(sample)?;
        }
        Ok(())
    }

    /// Pass the coded pictures of `layout` to `callback` from
    /// [`num_threads`](zune_core::options::DecoderOptions::num_threads) threads.
    #[cfg(feature = "std")]
    pub(crate) fn process_hevc_samples_parallel<F>(
        &self, layout: &ImageLayout, callback: F
    ) -> Result<(), HeicErrors>
    where
        F: Fn(HevcSample<'_>) -> Result<(), HeicErrors> + Sync
    {
        use std::sync::Mutex;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let samples = self.collect_samples(layout)?;
        let next_job_idx = AtomicUsize::new(0);
        let first_error = Mutex::new(None);

        // Don't spawn 8 threads if there are only 2 tiles
        let num_threads = (self.options.num_threads() as usize)
            .min(samples.len())
            .max(1);

        std::thread::scope(|scope| {
            for _ in 0..num_threads {
                scope.spawn(|| {
                    loop {
                        // Short-circuit if another thread already hit an error
                        if first_error.lock().unwrap().is_some() {
                            break;
                        }
                        // Grab the next available sample index atomically
                        let idx = next_job_idx.fetch_add(1, Ordering::Relaxed);
                        let Some(sample) = samples.get(idx) else {
                            break; // Queue is empty, thread can exit
                        };
                        // cheap: parameter sets are small, extents are slices
                        let sample = HevcSample {
                            item_id: sample.item_id,
                            vps:     sample.vps.clone(),
                            sps:     sample.sps.clone(),
                            pps:     sample.pps.clone(),
                            extents: sample.extents.clone()
                        };
                        if let Err(e) = callback(sample) {
                            let mut err_guard = first_error.lock().unwrap();
                            if err_guard.is_none() {
                                *err_guard = Some(e);
                            }
                            break;
                        }
                    }
                });
            }
        });

        // If any thread bailed with an error, return it to the caller
        if let Some(err) = first_error.into_inner().unwrap() {
            return Err(err);
        }
        Ok(())
    }
}

/// Extracts the raw VPS, SPS, and PPS NAL units from an HEVC Configuration Record.
pub fn extract_hevc_parameter_sets(hvcc_payload: &[u8]) -> ParameterSets {
    let mut vps = None;
    let mut sps = None;
    let mut pps = None;

    // The hvcC record has exactly 22 bytes of fixed-length configuration
    // flags before the NAL unit arrays begin.
    if hvcc_payload.len() < 23 {
        return ParameterSets { vps, sps, pps };
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
                32
                    if vps.is_none() => {
                        vps = Some(nalu);
                    }
                33
                    if sps.is_none() => {
                        sps = Some(nalu);
                    }
                34
                    if pps.is_none() => {
                        pps = Some(nalu);
                    }
                _ => {} // We ignore SEI messages or other arrays for basic decoding
            }
        }
    }

    ParameterSets { vps, sps, pps }
}
