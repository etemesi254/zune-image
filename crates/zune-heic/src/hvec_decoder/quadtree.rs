use crate::hvec_decoder::binarizer::Binarizer;
use crate::hvec_decoder::cabac::CabacEngine;
use crate::hvec_decoder::nal_parser::{NalError, NalUnit};
use crate::hvec_decoder::nal_unit_headers::{Pps, Sps};
use crate::hvec_decoder::nal_unit_parsers::decode_slice_header;
use crate::hvec_decoder::utils::extract_rbsp;

pub fn decode_slice(
    nal: &NalUnit, pps_storage: &[Option<Pps>], sps_storage: &[Option<Sps>]
) -> Result<(), NalError> {
    // 1. Clean the entire NAL unit first! Skip the 2-byte NAL header.
    let clean_rbsp = extract_rbsp(&nal.payload[..]);

    // 2. Pass the clean bytes to the slice header parser
    let slice_header = decode_slice_header(&nal, &pps_storage, &sps_storage, &clean_rbsp)?;
    // 1. Resolve Active Parameter Sets
    let pps = pps_storage[slice_header.slice_pic_parameter_set_id as usize]
        .as_ref()
        .expect("Stream error: PPS missing!");
    let sps = sps_storage[pps.sps_id as usize]
        .as_ref()
        .expect("Stream error: SPS missing!");

    // 2. Extract the raw CABAC payload
    let payload_start = slice_header.cabac_start_position;
    let slice_qp = 26 + pps.init_qp_minus26 + slice_header.slice_qp_delta;

    // 2. Extract the raw CABAC payload
    let payload_start = slice_header.cabac_start_position;

    // 3. Calculate Slice QP for Context Initialization
    let slice_qp = 26 + pps.init_qp_minus26 + slice_header.slice_qp_delta;

    // 4. Boot up the Entropy Pipeline
    let mut cabac = CabacEngine::new(
        &clean_rbsp[payload_start..],
        slice_header.slice_type,
        slice_qp
    );
    let mut binarizer = Binarizer::new(&mut cabac);

    // 5. The CTU Raster Scan Loop
    let ctu_size = sps.ctb_size_y as usize;
    let width_in_ctus = sps.pic_width_in_ctbs_y as usize;
    let height_in_ctus = sps.pic_height_in_ctbs_y as usize;
    let total_ctus = width_in_ctus * height_in_ctus;

    for ctu_idx in 0..total_ctus {}
    Ok(())
}
