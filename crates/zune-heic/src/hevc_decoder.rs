use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use zune_core::log::trace;

use crate::hevc_decoder::nal_parser::{NalError, NalParser, NalUnitType};
use crate::hevc_decoder::nal_unit_headers::{Pps, Sps, Vps};
use crate::hevc_decoder::nal_unit_parsers::{decode_pps, decode_sps, decode_vps};
use crate::hevc_decoder::neighbor_tracker::NeighborTracker;
use crate::hevc_decoder::quadtree::decode_slice;
use crate::hevc_decoder::raw_frame::RawFrame;
use crate::processor::HevcSample;

pub(crate) static DEBUG_MORE: AtomicBool = AtomicBool::new(false);
mod bitstream;
mod cabac;
mod cabac_tables;
mod constants;
pub(crate) mod ctx;
mod deblocker;
mod macros;
mod nal_parser;
mod nal_unit_headers;
mod nal_unit_parsers;
mod neighbor_tracker;
mod quadtree;
mod raw_frame;
mod utils;

mod idct;

pub struct HevcDecoder {
    vps_storage: Vec<Option<Vps>>,
    sps_storage: Vec<Option<Sps>>,
    pps_storage: Vec<Option<Pps>>,
    width: usize,
    height: usize,
    pub(crate) neighbor_tracker: Option<NeighborTracker>,
    pub(crate) dependent_slice_contexts: Option<Vec<u8>>
}

impl HevcDecoder {
    fn new() -> Self {
        Self {
            vps_storage:              vec![None; 16],
            sps_storage:              vec![None; 16],
            pps_storage:              vec![None; 16],
            width:                    0,
            height:                   0,
            neighbor_tracker:         None,
            dependent_slice_contexts: None
        }
    }

    pub fn decode(&mut self, sample: HevcSample) -> Result<Option<Arc<RawFrame>>, NalError> {
        let nal_parser = NalParser::new_detect(&sample.extents);

        let mut raw_frame = None;
        nal_parser.for_each_nal(|nal| {
            match nal.nal_type {
                // --- Metadata NALs ---
                NalUnitType::VpsNut => {
                    trace!("Decoding vps nal unit");
                    let vps = decode_vps(&nal)?;
                    let vps_id = vps.vps_id as usize;
                    self.vps_storage[vps_id] = Some(vps);
                }
                NalUnitType::SpsNut => {
                    trace!("Decoding sps nal unit");
                    let sps = decode_sps(&nal)?;
                    let sps_id = sps.sps_id as usize;
                    self.width = sps.pic_width_in_luma_samples as usize;
                    self.height = sps.pic_height_in_luma_samples as usize;
                    self.sps_storage[sps_id] = Some(sps);
                }
                NalUnitType::PpsNut => {
                    trace!("Decoding pps nal unit");
                    let pps = decode_pps(&nal, &self.sps_storage)?;
                    let pps_id = pps.pps_id as usize;
                    self.pps_storage[pps_id] = Some(pps);
                }

                // --- Video Coding Layer (VCL) NALs (The actual frames) ---
                // HEVC VCL NAL types are 0 to 31. We can catch all of them here.
                nal_type if (nal_type as u8) <= 31 => {
                    if raw_frame.is_none() {
                        // We need the SPS to know the resolution.
                        // TODO: Get the raw sps_id
                        let active_sps =
                            self.sps_storage.iter().find_map(|sps| sps.as_ref()).expect(
                                "Stream Error: VCL NAL encountered before any SPS was loaded!"
                            );

                        let units_w = active_sps.pic_width_in_luma_samples as usize;
                        let units_h = active_sps.pic_height_in_luma_samples as usize;

                        // Spin up the fresh pixel buffer and tracker for this frame!
                        raw_frame = Some(RawFrame::from_sps(active_sps));

                        self.neighbor_tracker = Some(NeighborTracker::new(units_w, units_h, 2));
                    }

                    trace!("Decoding NAL {:?}", nal_type);
                    if let Some(f) = raw_frame.clone() {
                        decode_slice(&nal, self, f)?;
                    } else {
                        panic!("No Raw frame allocated");
                    }
                }

                _ => {
                    trace!("Skipping NAL section {:?}", nal.nal_type);
                }
            }

            Ok(true)
        })?;
        // apply deblocking

        Ok(raw_frame)
    }

    /// Call this if the container format (like HEIC or MP4) provides global
    /// parameter sets in its header (e.g., the `hvcC` box) before video samples.
    pub fn parse_extradata(&mut self, extradata: &[u8]) -> Result<(), NalError> {
        let binding = [extradata];

        let nal_parser = NalParser::new_detect(&binding);

        nal_parser.for_each_nal(|nal| {
            match nal.nal_type {
                NalUnitType::VpsNut => {
                    let vps = decode_vps(&nal)?;
                    let vps_id = vps.vps_id as usize;
                    self.vps_storage[vps_id as usize] = Some(vps);
                }
                NalUnitType::SpsNut => {
                    let sps = decode_sps(&nal)?;
                    let sps_id = sps.sps_id as usize;
                    self.sps_storage[sps_id] = Some(sps);
                }
                NalUnitType::PpsNut => {
                    let pps = decode_pps(&nal, &self.sps_storage)?;
                    let pps_id = pps.pps_id as usize;
                    self.pps_storage[pps_id] = Some(pps);
                }
                _ => {
                    trace!("Ignoring non-metadata NAL in extradata: {:?}", nal.nal_type);
                }
            }
            Ok(true)
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs::read;

    use crate::hevc_decoder::HevcDecoder;
    use crate::processor::HevcSample;

    #[test]
    fn tests_load_hvec() {
        let data = read("/Users/etemesi/rust/zune-image/output_dirs/item_0001.hvc").unwrap();

        let sample = HevcSample {
            item_id: 0,
            vps:     None,
            sps:     None,
            pps:     None,
            extents: vec![&data]
        };
        let mut decoder = HevcDecoder::new();

        let frame = decoder.decode(sample).unwrap();

        frame.unwrap().dump_ppm("item_0002.ppm").unwrap();
    }
}
