use std::sync::atomic::AtomicBool;

use zune_core::log::trace;

use crate::hevc_decoder::nal_parser::{NalError, NalParser, NalUnitType};
use crate::hevc_decoder::nal_unit_headers::{Pps, SliceHeader, Sps, Vps};
use crate::hevc_decoder::nal_unit_parsers::{decode_pps, decode_sps, decode_vps};
use crate::hevc_decoder::neighbor_tracker::NeighborTracker;
use crate::hevc_decoder::quadtree::decode_slice;
use crate::hevc_decoder::raw_frame::RawFrame;
use crate::processor::HevcSample;

pub(crate) static  DEBUG_MORE: AtomicBool = AtomicBool::new(false);
mod binarizer;
mod bitstream;
mod cabac;
mod cabac_tables;
mod constants;
pub(crate) mod ctx;
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
    vps_storage:                  Vec<Option<Vps>>,
    sps_storage:                  Vec<Option<Sps>>,
    pps_storage:                  Vec<Option<Pps>>,
    pub(crate) neighbor_tracker:  Option<NeighborTracker>,
    last_size_header:             Box<Option<SliceHeader>>,
    pub dependent_slice_contexts: Option<Vec<u8>>
}

impl HevcDecoder {
    fn new() -> Self {
        Self {
            vps_storage:              vec![None; 16],
            sps_storage:              vec![None; 16],
            pps_storage:              vec![None; 16],
            neighbor_tracker:         None,
            last_size_header:         Box::new(None),
            dependent_slice_contexts: None
        }
    }

    pub fn decode(&mut self, sample: HevcSample) -> Result<(), NalError> {
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

                    let units_w = sps.pic_width_in_luma_samples as usize;
                    let units_h = sps.pic_height_in_luma_samples as usize;

                    raw_frame = Some(RawFrame::from_sps(&sps));
                    self.sps_storage[sps_id] = Some(sps);

                    self.neighbor_tracker = Some(NeighborTracker::new(units_w, units_h, 2))
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
                    trace!("Decoding NAL {:?}", nal_type);
                    if let Some(f) = raw_frame.clone() {
                        decode_slice(&nal, self, f)?;
                    }
                }

                _ => {
                    trace!("Skipping NAL section {:?}", nal.nal_type);
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
        let data = read("/Users/etemesi/rust/zune-image/output_dirs/item_0002.hvc").unwrap();

        let sample = HevcSample {
            item_id: 0,
            vps:     None,
            sps:     None,
            pps:     None,
            extents: vec![&data]
        };
        let mut decoder = HevcDecoder::new();
        decoder.decode(sample).unwrap();
    }
}
