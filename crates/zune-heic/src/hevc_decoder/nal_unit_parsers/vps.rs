use crate::hevc_decoder::DEBUG_MORE;
use crate::debug_more;
use crate::hevc_decoder::bitstream::BitReader;
use crate::hevc_decoder::nal_parser::{NalError, NalUnit};
use crate::hevc_decoder::nal_unit_headers::Vps;
use crate::hevc_decoder::nal_unit_parsers::decode_profile_data;

pub fn decode_vps(nal: &NalUnit) -> Result<Vps, NalError> {
    const VPS_MAX_LAYERS_LIMIT: u64 = 64;
    const VPS_MAX_SUBLAYERS_LIMIT: u64 = 8;

    let mut r = BitReader::new(nal.payload);
    r.refill();

    let vps_id = r.get_bits(4) ?as usize;
    r.skip_bits(2);
    let max_layers = r.get_bits(6)? + 1;

    if max_layers > VPS_MAX_LAYERS_LIMIT {
        return Err(NalError::ParameterOutOfRange {
            limit: VPS_MAX_LAYERS_LIMIT as _,
            value: max_layers as _,
            field: "vps_max_layers"
        });
    }

    let max_sub_layers = r.get_bits(3)? + 1;
    if max_sub_layers > VPS_MAX_SUBLAYERS_LIMIT {
        return Err(NalError::ParameterOutOfRange {
            limit: VPS_MAX_LAYERS_LIMIT as _,
            value: max_sub_layers as _,
            field: "vps_max_sub_layers"
        });
    }
    r.skip_bits(1);

    r.get_bits(16)?; // reserved_ffff

    // --- Profile Tier Level ---
    let ptl_info = decode_profile_data(true, true, &mut r)?;

    // --- Sub-layer ordering info ---
    let mut max_dec_pic_buffering = Vec::with_capacity(max_sub_layers as usize);
    let mut max_num_reorder_pics = Vec::with_capacity(max_sub_layers as usize);

    let sub_layer_ordering_info_present = r.read_flag()?;
    for _ in 0..max_sub_layers as usize {
        if sub_layer_ordering_info_present {
            max_dec_pic_buffering.push(r.read_ue()? as u32);
            max_num_reorder_pics.push(r.read_ue()? as u32);
            let _max_latency_increase = r.read_ue()?;
        } else {
            // Derive from index 0 if not present for this sub-layer
            let prev_dec = *max_dec_pic_buffering.first().unwrap_or(&0);
            let prev_reorder = *max_num_reorder_pics.first().unwrap_or(&0);
            max_dec_pic_buffering.push(prev_dec);
            max_num_reorder_pics.push(prev_reorder);
        }
    }

    // Wrap it up and store it
    let vps = Vps {
        vps_id: vps_id as u8,
        max_layers: max_layers as u8,
        max_sub_layers: max_sub_layers as u8,
        profile_info: ptl_info.expect("NO Profile Info"),
        max_dec_pic_buffering,
        max_num_reorder_pics
    };
    debug_more!(false=>"vps: {:#?}", vps);

    return Ok(vps);
}