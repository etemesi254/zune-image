use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::bitstream::BitReader;
use crate::hevc_decoder::nal_parser::{NalError, NalUnit};
use crate::hevc_decoder::nal_unit_headers::{ChromaFormat, Sps};
use crate::hevc_decoder::nal_unit_parsers::{
    decode_profile_data, parse_short_term_ref_pic_set, parse_vui, skip_scaling_list_data
};

pub fn decode_sps(nal: &NalUnit) -> Result<Sps, NalError> {
    const SPS_MAX_LAYERS_LIMIT: u64 = 7;
    const SPS_MAX_SETS_LIMITS: u64 = 16;
    const MAX_PICTURE_WIDTH: u64 = 2 << 15;
    const MAX_PICTURE_HEIGHT: u64 = 2 << 15;
    const MAX_LUMA_BITDEPTH: u8 = 16;

    let mut r = BitReader::new(nal.payload);
    r.refill();

    let mut sps = Sps::default();

    sps.vps_id = r.get_bits(4);
    sps.max_sub_layers = r.get_bits(3) + 1;

    if sps.max_sub_layers > SPS_MAX_LAYERS_LIMIT {
        return Err(NalError::ParameterOutOfRange {
            limit: SPS_MAX_LAYERS_LIMIT as _,
            value: sps.max_sub_layers as _,
            field: "sps_max_sub_layers"
        });
    }

    let _sps_temporal_id_nesting_flag = r.read_flag();

    // Passing true for profile_present_flag, and max_sub_layers - 1
    sps.ptl = decode_profile_data(true, true, &mut r)?;

    let vlc = r.read_ue();
    if vlc >= SPS_MAX_SETS_LIMITS {
        return Err(NalError::ParameterOutOfRange {
            limit: SPS_MAX_SETS_LIMITS as _,
            value: vlc as _,
            field: "(sps) vlc"
        });
    }
    sps.sps_id = vlc;

    // --- Decode Chroma type ---
    sps.chroma_format = match r.read_ue() {
        0 => ChromaFormat::Monochrome,
        1 => ChromaFormat::Yuv420,
        2 => ChromaFormat::Yuv422,
        3 => ChromaFormat::Yuv444,
        r => {
            return Err(NalError::Generic(format!(
                "Invalid Chroma Format (>3) {}",
                r
            )));
        }
    };

    sps.separate_color_plane_flag =
        if sps.chroma_format == ChromaFormat::Yuv444 { r.read_flag() } else { false };

    // --- Picture Size ---
    sps.pic_width_in_luma_samples = r.read_ue();
    sps.pic_height_in_luma_samples = r.read_ue();

    if sps.pic_width_in_luma_samples > MAX_PICTURE_WIDTH {
        return Err(NalError::ParameterOutOfRange {
            limit: MAX_PICTURE_WIDTH as _,
            value: sps.pic_width_in_luma_samples as _,
            field: "(sps) width"
        });
    }
    if sps.pic_height_in_luma_samples > MAX_PICTURE_HEIGHT {
        return Err(NalError::ParameterOutOfRange {
            limit: MAX_PICTURE_HEIGHT as _,
            value: sps.pic_height_in_luma_samples as _,
            field: "(sps) height"
        });
    }

    // --- Conformance Window (Crop Info) ---
    sps.conformance_window_flag = r.read_flag();
    if sps.conformance_window_flag {
        sps.conf_win_left_offset = r.read_ue();
        sps.conf_win_right_offset = r.read_ue();
        sps.conf_win_top_offset = r.read_ue();
        sps.conf_win_bottom_offset = r.read_ue();
    } else {
        sps.conf_win_left_offset = 0;
        sps.conf_win_right_offset = 0;
        sps.conf_win_top_offset = 0;
        sps.conf_win_bottom_offset = 0;
    }

    // --- Bit Depth ---
    sps.bit_depth_luma = r.read_ue() as u8 + 8;
    if sps.bit_depth_luma > MAX_LUMA_BITDEPTH {
        return Err(NalError::ParameterOutOfRange {
            limit: MAX_LUMA_BITDEPTH as _,
            value: sps.bit_depth_luma as _,
            field: "bit_depth (luma)"
        });
    }

    sps.bit_depth_chroma = r.read_ue() as u8 + 8;
    if sps.bit_depth_chroma > MAX_LUMA_BITDEPTH {
        return Err(NalError::ParameterOutOfRange {
            limit: MAX_LUMA_BITDEPTH as _,
            value: sps.bit_depth_chroma as _,
            field: "bit_depth (chroma)"
        });
    }
    if sps.bit_depth_chroma != sps.bit_depth_luma {
        return Err(NalError::Generic(format!(
            "Luma and Chroma Bit depth differ (luma={}, chroma={})",
            sps.bit_depth_luma, sps.bit_depth_chroma
        )));
    }

    sps.log2_max_pic_order_cnt_lsb = (r.read_ue() as u8) + 4;

    // --- Sub-layer ordering info ---
    let sps_sub_layer_ordering_info_present_flag = r.read_flag();
    let first_layer = if sps_sub_layer_ordering_info_present_flag {
        0
    } else {
        sps.max_sub_layers - 1
    };

    for i in first_layer..sps.max_sub_layers {
        let idx = i as usize;
        sps.max_dec_pic_buffering[idx] = r.read_ue() + 1;
        sps.max_num_reorder_pics[idx] = r.read_ue();
        sps.max_latency_increase_plus1[idx] = r.read_ue();
    }

    // Copy info to all layers if only specified once
    if !sps_sub_layer_ordering_info_present_flag && sps.max_sub_layers > 1 {
        let ref_idx = (sps.max_sub_layers - 1) as usize;
        for i in 0..ref_idx {
            sps.max_dec_pic_buffering[i] = sps.max_dec_pic_buffering[ref_idx];
            sps.max_num_reorder_pics[i] = sps.max_num_reorder_pics[ref_idx];
            sps.max_latency_increase_plus1[i] = sps.max_latency_increase_plus1[ref_idx];
        }
    }

    // --- CTB & Transform Sizes ---
    sps.log2_min_luma_coding_block_size = (r.read_ue() + 3) as u8;
    sps.log2_diff_max_min_luma_coding_block_size = r.read_ue() as u8;
    sps.log2_min_transform_block_size = (r.read_ue() + 2) as u8;
    sps.log2_diff_max_min_transform_block_size = (r.read_ue()) as u8;
    sps.max_transform_hierarchy_depth_inter = r.read_ue();
    sps.max_transform_hierarchy_depth_intra = r.read_ue();

    // --- Scaling List ---
    sps.scaling_list_enabled_flag = r.read_flag();
    if sps.scaling_list_enabled_flag {
        let sps_scaling_list_data_present_flag = r.read_flag();
        if sps_scaling_list_data_present_flag {
            skip_scaling_list_data(&mut r)?;
        }
    }

    sps.amp_enabled_flag = r.read_flag();
    sps.sample_adaptive_offset_enabled_flag = r.read_flag();
    sps.pcm_enabled_flag = r.read_flag();

    if sps.pcm_enabled_flag {
        sps.pcm_sample_bit_depth_luma = (r.get_bits(4) + 1) as u8;
        sps.pcm_sample_bit_depth_chroma = (r.get_bits(4) + 1) as u8;
        sps.log2_min_pcm_luma_coding_block_size = (r.read_ue() + 3) as u8;
        sps.log2_diff_max_min_pcm_luma_coding_block_size = r.read_ue() as u8;
        sps.pcm_loop_filter_disable_flag = r.read_flag();
    }

    // --- Reference Picture Sets ---
    sps.num_short_term_ref_pic_sets = r.read_ue();

    // Track POC deltas across loop iterations for inter-RPS prediction
    let mut num_delta_pocs = [0usize; 65];

    for i in 0..(sps.num_short_term_ref_pic_sets as usize) {
        parse_short_term_ref_pic_set(
            &mut r,
            i,
            sps.num_short_term_ref_pic_sets as usize,
            &mut num_delta_pocs
        )?;
    }

    let long_term_ref_pics_present_flag = r.read_flag();
    if long_term_ref_pics_present_flag {
        sps.num_long_term_ref_pics_sps = r.read_ue();
        for _ in 0..(sps.num_long_term_ref_pics_sps) {
            r.get_bits(sps.log2_max_pic_order_cnt_lsb as u8);
            r.read_flag(); // used_by_curr_pic_lt_sps_flag
        }
    } else {
        sps.num_long_term_ref_pics_sps = 0;
    }

    sps.sps_temporal_mvp_enabled_flag = r.read_flag();
    sps.strong_intra_smoothing_enable_flag = r.read_flag();

    // --- VUI Parameters ---
    let vui_parameters_present_flag = r.read_flag();
    if vui_parameters_present_flag {
        sps.vui = Some(parse_vui(&mut r, sps.max_sub_layers - 1)?);
    }

    let _sps_extension_present_flag = r.read_flag();

    // --- Compute Derived Values ---
    sps.min_cb_size_y = 1 << sps.log2_min_luma_coding_block_size;

    let log2_ctb_size_y =
        sps.log2_min_luma_coding_block_size + (sps.log2_diff_max_min_luma_coding_block_size);
    sps.ctb_size_y = 1 << log2_ctb_size_y;

    // Integer math equivalent of ceil(width / ctb_size)
    sps.pic_width_in_ctbs_y = sps.pic_width_in_luma_samples.div_ceil(sps.ctb_size_y);
    sps.pic_height_in_ctbs_y = sps.pic_height_in_luma_samples.div_ceil(sps.ctb_size_y);

    sps.log2_ctb_size_y = log2_ctb_size_y;
    sps.log2_max_transform_block_size =
        sps.log2_min_transform_block_size + sps.log2_diff_max_min_transform_block_size;

    debug_more!(false=>"{:#?}", sps);

    Ok(sps)
}
