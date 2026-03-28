use zune_core::log::trace;

use crate::hevc_decoder::nal_parser::{NalParser, NalUnitType};
use crate::hevc_decoder::nal_unit_headers::{Pps, SliceHeader, Sps, Vps};
use crate::hevc_decoder::nal_unit_parsers::{decode_pps, decode_sps, decode_vps};
use crate::hevc_decoder::quadtree::decode_slice;
use crate::processor::HevcSample;

pub const DEBUG_MORE: bool = true;
mod binarizer;
mod bitstream;
mod cabac;
mod cabac_tables;
mod neighbor_tracker;
mod macros;
mod nal_parser;
mod nal_unit_headers;
mod nal_unit_parsers;
mod quadtree;
mod quadtree_vb;
mod utils;
mod constants;
pub struct HevcDecoder {
    vps_storage:      Vec<Option<Vps>>,
    sps_storage:      Vec<Option<Sps>>,
    pps_storage:      Vec<Option<Pps>>,
    last_size_header: Box<Option<SliceHeader>>
}

impl HevcDecoder {
    fn new() -> Self {
        Self {
            vps_storage:      vec![None; 16],
            sps_storage:      vec![None; 16],
            pps_storage:      vec![None; 16],
            last_size_header: Box::new(None)
        }
    }

    pub fn decode(&mut self, sample: HevcSample) {
        let nal_parser = NalParser::new_detect(&sample.extents);

        nal_parser
            .for_each_nal(|nal| {
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
                        trace!("Decoding NAL {:?}", nal_type);
                        decode_slice(&nal, self)?;
                    }

                    _ => {
                        trace!("Skipping NAL section {:?}", nal.nal_type);
                    }
                }

                Ok(true)
            })
            .unwrap();
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
        decoder.decode(sample);
    }
}
