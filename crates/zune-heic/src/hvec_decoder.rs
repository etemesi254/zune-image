use zune_core::log::trace;

use crate::hvec_decoder::bitstream::BitReader;
use crate::hvec_decoder::nal_parser::{NalError, NalFraming, NalParser, NalUnit, NalUnitType};
use crate::hvec_decoder::nal_unit_headers::{Pps, Sps, Vps};
use crate::hvec_decoder::nal_unit_parsers::{
    decode_pps, decode_slice_header, decode_sps, decode_vps
};
use crate::processor::HevcSample;

mod bitstream;
mod nal_parser;
mod nal_unit_headers;
mod nal_unit_parsers;

struct HVecDecoder<'a> {
    hevc_sample: HevcSample<'a>,
    vps_storage: Vec<Option<Vps>>,
    sps_storage: Vec<Option<Sps>>,
    pps_storage: Vec<Option<Pps>>
}
impl<'a> HVecDecoder<'a> {
    fn new(sample: HevcSample<'a>) -> Self {
        Self {
            hevc_sample: sample,
            vps_storage: vec![None; 16],
            sps_storage: vec![None; 16],
            pps_storage: vec![None; 16]
        }
    }

    pub fn decode(&mut self) {
        let nal_parser = NalParser::new_detect(&self.hevc_sample.extents);

        nal_parser
            .for_each_nal(|nal| {
                match nal.nal_type {
                    // --- Metadata NALs ---
                    NalUnitType::VpsNut => {
                        let vps = decode_vps(&nal)?;
                        let vps_id = vps.vps_id as usize;
                        self.vps_storage[vps_id] = Some(vps);
                    }
                    NalUnitType::SpsNut => {
                        let sps = decode_sps(&nal)?;
                        let sps_id = sps.sps_id as usize;
                        self.sps_storage[sps_id] = Some(sps);
                    }
                    NalUnitType::PpsNut => {
                        // Pass the sps_storage so decode_pps can look up the referenced SPS
                        let pps = decode_pps(&nal, &self.sps_storage)?;
                        let pps_id = pps.pps_id as usize;
                        self.pps_storage[pps_id] = Some(pps);
                    }

                    // --- Video Coding Layer (VCL) NALs (The actual frames) ---
                    // HEVC VCL NAL types are 0 to 31. We can catch all of them here.
                    nal_type if (nal_type as u8) <= 31 => {
                        // Pass the storages so the slice header can resolve its context
                        let slice_header =
                            decode_slice_header(&nal, &self.pps_storage, &self.sps_storage)?;

                        // The BitReader inside decode_slice_header stopped exactly
                        // where the CABAC entropy data begins!
                        println!(
                            "Successfully parsed slice! Type: {}, POC: {}",
                            slice_header.slice_type, slice_header.slice_pic_order_cnt_lsb
                        );
                    }

                    // --- SEI, AUD, and everything else ---
                    _ => {}
                }

                Ok(true)
            })
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use std::fs::read;

    use crate::hvec_decoder::HVecDecoder;
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
        let mut decoder = HVecDecoder::new(sample);
        decoder.decode();
    }
}
