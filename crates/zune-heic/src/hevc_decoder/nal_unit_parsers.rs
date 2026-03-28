use zune_core::log::{trace, warn};

use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::bitstream::BitReader;
use crate::hevc_decoder::nal_parser::{NalError, NalUnit};
use crate::hevc_decoder::nal_unit_headers::{
    ChromaFormat, Pps, PpsRangeExtension, ProfileIdc, ProfileTierLevel, SliceHeader, SliceType,
    Sps, Vps, Vui, VuiVideoFormat
};

pub fn decode_vps(nal: &NalUnit) -> Result<Vps, NalError> {
    const VPS_MAX_LAYERS_LIMIT: u64 = 64;
    const VPS_MAX_SUBLAYERS_LIMIT: u64 = 8;

    let mut r = BitReader::new(nal.payload);
    r.refill();

    let vps_id = r.get_bits(4) as usize;
    r.skip_bits(2);
    let max_layers = r.get_bits(6) + 1;

    if max_layers > VPS_MAX_LAYERS_LIMIT {
        return Err(NalError::ParameterOutOfRange {
            limit: VPS_MAX_LAYERS_LIMIT as _,
            value: max_layers as _,
            field: "vps_max_layers"
        });
    }

    let max_sub_layers = r.get_bits(3) + 1;
    if max_sub_layers > VPS_MAX_SUBLAYERS_LIMIT {
        return Err(NalError::ParameterOutOfRange {
            limit: VPS_MAX_LAYERS_LIMIT as _,
            value: max_sub_layers as _,
            field: "vps_max_sub_layers"
        });
    }
    r.skip_bits(1);

    r.get_bits(16); // reserved_ffff

    // --- Profile Tier Level ---
    let ptl_info = decode_profile_data(true, true, &mut r)?;

    // --- Sub-layer ordering info ---
    let mut max_dec_pic_buffering = Vec::with_capacity(max_sub_layers as usize);
    let mut max_num_reorder_pics = Vec::with_capacity(max_sub_layers as usize);

    let sub_layer_ordering_info_present = r.read_flag();
    for _ in 0..max_sub_layers as usize {
        if sub_layer_ordering_info_present {
            max_dec_pic_buffering.push(r.read_ue() as u32);
            max_num_reorder_pics.push(r.read_ue() as u32);
            let _max_latency_increase = r.read_ue();
        } else {
            // Derive from index 0 if not present for this sub-layer
            let prev_dec = *max_dec_pic_buffering.get(0).unwrap_or(&0);
            let prev_reorder = *max_num_reorder_pics.get(0).unwrap_or(&0);
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

fn decode_profile_data(
    profile_present: bool, level_present: bool, r: &mut BitReader
) -> Result<Option<ProfileTierLevel>, NalError> {
    if !profile_present {
        trace!("Profile skipped");
        return Ok(None);
    }
    let profile_space = r.get_bits(2) as u8;
    let tier_flag = r.get_bits(1) as u8;

    let profile_idc = match r.get_bits(5) {
        1 => ProfileIdc::Main,
        2 => ProfileIdc::Main10,
        3 => ProfileIdc::MainStillPicture,
        4 => ProfileIdc::FormatRangeExtensions,
        _ => return Err(NalError::ForbiddenBitSet)
    };
    let mut profile_compatibility_flags = [false; 32];

    for item in profile_compatibility_flags.iter_mut() {
        *item = r.read_flag()
    }
    let progressive_source_flag = r.get_bits(1) as u8;
    let interlaced_source_flag = r.get_bits(1) as u8;

    let non_packed_constraint_flag = r.get_bits(1) as u8;
    let frame_only_constraint_flag = r.get_bits(1) as u8;

    // a lot of bits to skip, so skip in parts
    // we are to skip 44 bits but the bit-reader can refill 32 bits at a time
    // so break it down into modest skips
    r.skip_bits(22);
    r.skip_bits(22);

    let level_idc = if level_present { r.get_bits(8) as u8 } else { 0 };

    let item = ProfileTierLevel {
        profile_space,
        tier_flag,
        profile_idc,
        profile_compatibility_flags,
        progressive_source_flag,
        interlaced_source_flag,
        non_packed_constraint_flag,
        frame_only_constraint_flag,
        level_idc
    };

    Ok(Some(item))
}

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
        let _pcm_sample_bit_depth_luma = r.get_bits(4) + 1;
        let _pcm_sample_bit_depth_chroma = r.get_bits(4) + 1;
        let _log2_min_pcm_luma_coding_block_size = r.read_ue() + 3;
        let _log2_diff_max_min_pcm_luma_coding_block_size = r.read_ue();
        let _pcm_loop_filter_disable_flag = r.read_flag();
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

    sps.log2_ctb_size_y = sps.log2_min_luma_coding_block_size + sps.log2_diff_max_min_luma_coding_block_size;

    debug_more!(false=>"{:#?}", sps);

    Ok(sps)
}

// ============================================================================
// Helper Functions
// ============================================================================

fn skip_scaling_list_data(r: &mut BitReader) -> Result<(), NalError> {
    for size_id in 0..4 {
        let num_matrices = if size_id == 3 { 2 } else { 6 };

        for _ in 0..num_matrices {
            let scaling_list_pred_mode_flag = r.read_flag();
            if !scaling_list_pred_mode_flag {
                r.read_ue(); // scaling_list_pred_matrix_id_delta
            } else {
                let coef_num = std::cmp::min(64, 1 << (4 + (size_id << 1)));
                if size_id > 1 {
                    r.read_se(); // scaling_list_dc_coef_minus8
                }
                for _ in 0..coef_num {
                    r.read_se(); // scaling_list_delta_coef
                }
            }
        }
    }
    Ok(())
}

fn parse_short_term_ref_pic_set(
    r: &mut BitReader, st_rps_idx: usize, num_short_term_ref_pic_sets: usize,
    num_delta_pocs: &mut [usize]
) -> Result<(), NalError> {
    let mut inter_ref_pic_set_prediction_flag = false;

    if st_rps_idx != 0 {
        inter_ref_pic_set_prediction_flag = r.read_flag();
    }

    if inter_ref_pic_set_prediction_flag {
        let mut delta_idx_minus1 = 0;
        if st_rps_idx == num_short_term_ref_pic_sets {
            delta_idx_minus1 = r.read_ue() as usize;
        }
        let _sign_data_rps_minus1 = r.read_flag();
        let _abs_delta_rps_minus1 = r.read_ue();

        let ref_rps_idx = st_rps_idx - 1 - delta_idx_minus1;
        let mut current_delta_pocs = 0;

        for _ in 0..=num_delta_pocs[ref_rps_idx] {
            let used_by_curr_pic_flag = r.read_flag();
            let mut use_delta_flag = false;
            if !used_by_curr_pic_flag {
                use_delta_flag = r.read_flag();
            }
            if used_by_curr_pic_flag || use_delta_flag {
                current_delta_pocs += 1;
            }
        }
        num_delta_pocs[st_rps_idx] = current_delta_pocs;
    } else {
        let num_negative_pics = r.read_ue() as usize;
        let num_positive_pics = r.read_ue() as usize;

        for _ in 0..num_negative_pics {
            r.read_ue(); // delta_poc_s0_minus1
            r.read_flag(); // used_by_curr_pic_s0_flag
        }
        for _ in 0..num_positive_pics {
            r.read_ue(); // delta_poc_s1_minus1
            r.read_flag(); // used_by_curr_pic_s1_flag
        }
        num_delta_pocs[st_rps_idx] = num_negative_pics + num_positive_pics;
    }

    Ok(())
}

fn parse_vui(r: &mut BitReader, max_sub_layers_minus1: u64) -> Result<Vui, NalError> {
    let mut v = Vui::default();

    if r.read_flag() {
        // aspect_ratio_info_present_flag
        v.aspect_ratio_idc = r.get_bits(8) as u8;
        if v.aspect_ratio_idc == 255 {
            // Extended_SAR
            v.sar_width = r.get_bits(16) as u16;
            v.sar_height = r.get_bits(16) as u16;
        }
    }

    if r.read_flag() {
        // overscan_info_present_flag
        r.read_flag(); // overscan_appropriate_flag
    }

    if r.read_flag() {
        // video_signal_type_present_flag
        v.video_signal_type_present_flag = true;
        v.video_format = match r.get_bits(3) {
            0 => VuiVideoFormat::Component,
            1 => VuiVideoFormat::PAL,
            2 => VuiVideoFormat::NTSC,
            3 => VuiVideoFormat::SECAM,
            4 => VuiVideoFormat::MAC,
            _ => VuiVideoFormat::Unspecified
        };
        v.video_full_range_flag = r.read_flag();
        if r.read_flag() {
            // colour_description_present_flag
            v.colour_description_present_flag = true;
            v.colour_primaries = r.get_bits(8) as u8;
            v.transfer_characteristics = r.get_bits(8) as u8;
            v.matrix_coeffs = r.get_bits(8) as u8;
        }
    }

    if r.read_flag() {
        // chroma_loc_info_present_flag
        v.chroma_loc_info_present_flag = true;
        v.chroma_sample_loc_type_top_field = r.read_ue();
        v.chroma_sample_loc_type_bottom_field = r.read_ue();
    }

    r.read_flag(); // neutral_chroma_indication_flag
    r.read_flag(); // field_seq_flag
    r.read_flag(); // frame_field_info_present_flag

    if r.read_flag() {
        // default_display_window_flag
        r.read_ue();
        r.read_ue();
        r.read_ue();
        r.read_ue();
    }

    if r.read_flag() {
        // vui_timing_info_present_flag
        r.get_bits(32); // num_units_in_tick
        r.get_bits(32); // time_scale
        if r.read_flag() {
            // poc_proportional_to_timing_flag
            r.read_ue(); // num_ticks_poc_diff_one_minus1
        }

        if r.read_flag() {
            // hrd_parameters_present_flag
            skip_hrd_parameters(r, true, max_sub_layers_minus1)?;
        }
    }

    if r.read_flag() {
        // bitstream_restriction_flag
        v.tiles_fixed_structure_flag = r.read_flag();
        v.motion_vectors_over_pic_boundaries_flag = r.read_flag();
        r.read_flag(); // restricted_ref_pic_lists_flag
        v.min_spatial_segmentation_idc = r.read_ue();
        v.max_bytes_per_pic_denom = r.read_ue();
        v.max_bits_per_min_cu_denom = r.read_ue();
        v.log2_max_mv_length_horizontal = r.read_ue();
        v.log2_max_mv_length_vertical = r.read_ue();
    }

    Ok(v)
}

fn skip_hrd_parameters(
    r: &mut BitReader, common_inf_present_flag: bool, max_num_sub_layers_minus1: u64
) -> Result<(), NalError> {
    let mut nal_hrd_present = false;
    let mut vcl_hrd_present = false;
    let mut sub_pic_hrd_present = false;

    if common_inf_present_flag {
        nal_hrd_present = r.read_flag();
        vcl_hrd_present = r.read_flag();

        if nal_hrd_present || vcl_hrd_present {
            sub_pic_hrd_present = r.read_flag();
            if sub_pic_hrd_present {
                r.get_bits(8); // tick_divisor_minus2
                r.get_bits(5); // du_cpb_removal_delay_increment_length_minus1
                r.read_flag(); // sub_pic_cpb_params_in_pic_timing_sei_flag
                r.get_bits(5); // dpb_output_delay_du_length_minus1
            }
            r.get_bits(4); // bit_rate_scale
            r.get_bits(4); // cpb_size_scale
            if sub_pic_hrd_present {
                r.get_bits(4); // cpb_size_du_scale
            }
            r.get_bits(5); // initial_cpb_removal_delay_length_minus1
            r.get_bits(5); // au_cpb_removal_delay_length_minus1
            r.get_bits(5); // dpb_output_delay_length_minus1
        }
    }

    for _ in 0..=max_num_sub_layers_minus1 {
        let fixed_pic_rate_general_flag = r.read_flag();
        let mut fixed_pic_rate_within_cvs_flag = true;

        if !fixed_pic_rate_general_flag {
            fixed_pic_rate_within_cvs_flag = r.read_flag();
        }

        let mut low_delay_hrd_flag = false;
        if fixed_pic_rate_within_cvs_flag {
            r.read_ue(); // elemental_duration_in_tc_minus1
        } else {
            low_delay_hrd_flag = r.read_flag();
        }

        let mut cpb_cnt_minus1 = 0;
        if !low_delay_hrd_flag {
            cpb_cnt_minus1 = r.read_ue();
        }

        if nal_hrd_present {
            for _ in 0..=cpb_cnt_minus1 {
                r.read_ue();
                r.read_ue();
                if sub_pic_hrd_present {
                    r.read_ue();
                    r.read_ue();
                }
                r.read_flag();
            }
        }
        if vcl_hrd_present {
            for _ in 0..=cpb_cnt_minus1 {
                r.read_ue();
                r.read_ue();
                if sub_pic_hrd_present {
                    r.read_ue();
                    r.read_ue();
                }
                r.read_flag();
            }
        }
    }
    Ok(())
}

pub fn decode_pps(nal: &NalUnit, sps: &[Option<Sps>]) -> Result<Pps, NalError> {
    // 7.4.3.3.1: pps_pic_parameter_set_id is in [0, 63].
    const HEVC_MAX_PPS_COUNT: u8 = 64;
    // 7.4.3.2.1: sps_seq_parameter_set_id is in [0, 15].
    const HEVC_MAX_SPS_COUNT: u8 = 16;
    // 7.4.3.1: vps_max_dec_pic_buffering_minus1[i] is in [0, MaxDpbSize - 1].
    const HEVC_MAX_REFS: u8 = 16;

    let mut r = BitReader::new(nal.payload);
    r.refill();

    let mut pps = Pps::default();

    pps.pps_id = r.read_ue_u8()?;

    if pps.pps_id >= HEVC_MAX_PPS_COUNT {
        return Err(NalError::ParameterOutOfRange {
            limit: HEVC_MAX_PPS_COUNT as _,
            value: pps.pps_id as _,
            field: "pps.pps_id"
        });
    }

    pps.sps_id = r.read_ue_u8()?;

    if pps.sps_id >= HEVC_MAX_SPS_COUNT {
        return Err(NalError::ParameterOutOfRange {
            limit: HEVC_MAX_PPS_COUNT as _,
            value: pps.sps_id as _,
            field: "pps.sps_id"
        });
    }

    let sps = sps[pps.sps_id as usize]
        .as_ref()
        .ok_or_else(|| NalError::Generic(format!("PPS references missing SPS {}", pps.sps_id)))?;

    pps.dependent_slice_segments_enabled_flag = r.read_flag();
    pps.output_flag_present_flag = r.read_flag();
    pps.num_extra_slice_header_bits = r.get_bits(3) as u8;
    pps.sign_data_hiding_enabled_flag = r.read_flag();
    pps.cabac_init_present_flag = r.read_flag();

    pps.num_ref_idx_l0_default_active = r.read_ue_u8()? + 1;
    pps.num_ref_idx_l1_default_active = r.read_ue_u8()? + 1;

    if pps.num_ref_idx_l0_default_active >= HEVC_MAX_REFS
        || pps.num_ref_idx_l1_default_active >= HEVC_MAX_REFS
    {
        return Err(NalError::ParameterOutOfRange {
            limit: HEVC_MAX_REFS as _,
            value: pps.num_ref_idx_l0_default_active as _,
            field: "pps.num_ref_idx_l0_default_active"
        });
    }

    pps.init_qp_minus26 = r.read_se();
    pps.constrained_intra_pred_flag = r.read_flag();
    pps.transform_skip_enabled_flag = r.read_flag();
    pps.cu_qp_delta_enabled_flag = r.read_flag();

    if pps.cu_qp_delta_enabled_flag {
        pps.diff_cu_qp_delta_depth = r.read_ue() as u8;
    }

    if pps.diff_cu_qp_delta_depth > sps.log2_diff_max_min_luma_coding_block_size {
        return Err(NalError::ParameterOutOfRange {
            limit: sps.log2_diff_max_min_luma_coding_block_size as _,
            value: pps.diff_cu_qp_delta_depth as _,
            field: "pps.diff_cu_qp_delta_depth"
        });
    }

    pps.cb_qp_offset = r.read_se();

    if pps.cb_qp_offset < -12 || pps.cb_qp_offset > 12 {
        return Err(NalError::Generic(format!(
            "pps.cb_qp_offset must be between -12 and 12 but is {} ",
            pps.cb_qp_offset
        )));
    }

    pps.cr_qp_offset = r.read_se();

    if pps.cr_qp_offset < -12 || pps.cr_qp_offset > 12 {
        return Err(NalError::Generic(format!(
            "pps.cb_qp_offset must be between -12 and 12 but is {} ",
            pps.cr_qp_offset
        )));
    }

    pps.slice_chroma_qp_offsets_present_flag = r.read_flag();
    pps.weighted_pred_flag = r.read_flag();
    pps.weighted_bipred_flag = r.read_flag();
    pps.transquant_bypass_enabled_flag = r.read_flag();

    // --- Tiles Setup ---
    pps.tiles_enabled_flag = r.read_flag();
    pps.entropy_coding_sync_enabled_flag = r.read_flag();

    if pps.tiles_enabled_flag {
        pps.num_tile_columns = r.read_ue() + 1;
        pps.num_tile_rows = r.read_ue() + 1;
        pps.uniform_spacing_flag = r.read_flag();

        if !pps.uniform_spacing_flag {
            pps.column_width.reserve(pps.num_tile_columns as _);
            pps.row_height.reserve(pps.num_tile_rows as _);

            for _ in 0..(pps.num_tile_columns - 1) {
                pps.column_width.push(r.read_ue() + 1);
            }
            for _ in 0..(pps.num_tile_rows - 1) {
                pps.row_height.push(r.read_ue() + 1);
            }
        }
        pps.loop_filter_across_tiles_enabled_flag = r.read_flag();
    } else {
        // Defaults when tiles are disabled
        pps.num_tile_columns = 1;
        pps.num_tile_rows = 1;
        pps.loop_filter_across_tiles_enabled_flag = true;
    }

    // --- Deblocking & Loop Filter ---
    pps.loop_filter_across_slices_enabled_flag = r.read_flag();
    pps.deblocking_filter_control_present_flag = r.read_flag();

    if pps.deblocking_filter_control_present_flag {
        pps.deblocking_filter_override_enabled_flag = r.read_flag();
        pps.deblocking_filter_disabled_flag = r.read_flag();
        if !pps.deblocking_filter_disabled_flag {
            pps.beta_offset_div2 = r.read_se();
            pps.tc_offset_div2 = r.read_se();

            if pps.beta_offset_div2 < -6 || pps.beta_offset_div2 > 6 {
                return Err(NalError::Generic(format!(
                    "pps.beta_offset_div2 must be between -6 and 6 but is {} ",
                    pps.cr_qp_offset
                )));
            }
            if pps.tc_offset_div2 < -6 || pps.tc_offset_div2 > 6 {
                return Err(NalError::Generic(format!(
                    "pps.tc_offset_div2 must be between -6 and 6 but is {} ",
                    pps.cr_qp_offset
                )));
            }
        }
    }

    pps.pic_scaling_list_data_present_flag = r.read_flag();
    if pps.pic_scaling_list_data_present_flag {
        skip_scaling_list_data(&mut r)?;
    }
    // --- Extensions ---
    pps.lists_modification_present_flag = r.read_flag();

    let log2_parallel_merge_level_minus2 = r.read_ue_u8()?;
    if log2_parallel_merge_level_minus2 > sps.log2_ctb_size_y {
        return Err(NalError::ParameterOutOfRange {
            limit: sps.log2_ctb_size_y as _,
            value: log2_parallel_merge_level_minus2 as _,
            field: "log2_parallel_merge_level_minus2"
        });
    }
    pps.log2_parallel_merge_level = log2_parallel_merge_level_minus2 + 2;

    pps.slice_segment_header_extension_present_flag = r.read_flag();

    let pps_extension_present_flag = r.read_flag();

    if pps_extension_present_flag {
        let range_extension_flag = r.read_flag();
        let multilayer_extension_flag = r.read_flag();
        // skips
        // 3d extension flag => 1 bit
        // scc extension flag => 1 bit
        let extension_6_bits = r.get_bits(6);

        if range_extension_flag {
            let mut log2_max_transform_skip_block_size = 2;
            let mut diff_cu_chroma_qp_offset_depth = 0;
            let mut chroma_qp_offset_list_len = 0;
            let mut cr_qp_offset_list = [0; 6];
            let mut cb_qp_offset_list = [0; 6];

            let log2_sao_offset_scale_luma;
            let log2_sao_offset_scale_chroma;

            if pps.transform_skip_enabled_flag {
                let v = r.read_ue() as u8;
                let log_2_max_transform_size = (sps.log2_min_transform_block_size
                    + sps.log2_diff_max_min_transform_block_size)
                    - 2;

                if v > log_2_max_transform_size {
                    return Err(NalError::Generic(
                        "Invalid PPS Header range-extension".to_string()
                    ));
                }
                log2_max_transform_skip_block_size = v + 2;
            }

            let cross_component_prediction_enabled_flag = r.read_flag();

            if sps.chroma_format == ChromaFormat::Yuv444 && cross_component_prediction_enabled_flag
            {
                warn!(
                    "Invalid PPS header(range_component), chross_component_prediction={} and format=Yuv444 ",
                    cross_component_prediction_enabled_flag
                );
            }

            let chroma_qp_offset_list_enabled_flag = r.read_flag();

            if sps.chroma_format == ChromaFormat::Monochrome && chroma_qp_offset_list_enabled_flag {
                warn!(
                    "Invalid PPS header(range_component) chroma_qp_offset_list_enabled_flag =true and format=Monochrome ",
                );
            }

            if chroma_qp_offset_list_enabled_flag {
                let max_v = sps.log2_diff_max_min_luma_coding_block_size;
                let v = r.read_ue() as u8;

                if v > max_v {
                    return Err(NalError::Generic(format!(
                        "PPS header invalid diff_cu_chroma_qp_offset_depth={} should be greater than {}",
                        max_v, v
                    )));
                }

                diff_cu_chroma_qp_offset_depth = v;

                let v = r.read_ue() as u8;

                if v > 5 {
                    return Err(NalError::Generic(format!(
                        "PPS header invalid chroma_qp_offset_list_len = {} should be less than 5",
                        v
                    )));
                }
                chroma_qp_offset_list_len = v + 1;

                for i in 0..chroma_qp_offset_list_len {
                    let s_v = r.read_se();

                    if s_v < -12 || s_v > 12 {
                        return Err(NalError::Generic(format!(
                            "SVLC value cb {} not in range of -12 = 12",
                            s_v
                        )));
                    }
                    cb_qp_offset_list[i as usize] = s_v as i8;

                    let s_v = r.read_se();

                    if s_v < -12 || s_v > 12 {
                        return Err(NalError::Generic(format!(
                            "SVLC value cr {} not in range of -12 = 12",
                            s_v
                        )));
                    }
                    cr_qp_offset_list[i as usize] = s_v as i8;
                }
            }

            let u = r.read_ue() as u8;

            if u > sps.bit_depth_luma.saturating_sub(10) {
                return Err(NalError::Generic(format!(
                    "INVALID PPS header bit_depth_luma {} > {}",
                    sps.bit_depth_luma.saturating_sub(10),
                    u
                )));
            }
            log2_sao_offset_scale_luma = u;

            let u = r.read_ue() as u8;

            if u > sps.bit_depth_chroma.saturating_sub(10) {
                return Err(NalError::Generic(format!(
                    "INVALID PPS header bit_depth_chroma {} > {}",
                    sps.bit_depth_luma.saturating_sub(10),
                    u
                )));
            }

            log2_sao_offset_scale_chroma = u;

            let range_ext = PpsRangeExtension {
                cb_qp_offset_list,
                cr_qp_offset_list,
                log2_sao_offset_scale_luma,
                log2_sao_offset_scale_chroma,
                log2_max_transform_skip_block_size,
                diff_cu_chroma_qp_offset_depth,
                chroma_qp_offset_list_len
            };
            pps.range_extension = Some(range_ext);
        }
    }

    pps.pic_init_qp = 26 + pps.init_qp_minus26;

    let log2_ctb_size_y =
        sps.log2_min_luma_coding_block_size + (sps.log2_diff_max_min_luma_coding_block_size);
    pps.log2_min_cu_qp_delta_size = log2_ctb_size_y - pps.diff_cu_qp_delta_depth;

    debug_more!(false=>"{:#?}", pps);

    Ok(pps)
}
pub fn decode_slice_header(
    nal: &NalUnit,
    pps_storage: &[Option<Pps>],
    sps_storage: &[Option<Sps>],
    clean_payload: &[u8]
) -> Result<SliceHeader, NalError> {
    // 1. Clean the RBSP first to handle 0x03 Emulation Prevention Bytes
    let mut r = BitReader::new(&clean_payload);

    let mut sh = SliceHeader::default();

    // 2. Initial Flags
    sh.first_slice_segment_in_pic_flag = r.read_flag();

    let nal_unit_type = nal.nal_type as u8;
    let is_irap = nal_unit_type >= 16 && nal_unit_type <= 23;
    let is_idr = nal_unit_type == 19 || nal_unit_type == 20;

    if is_irap {
        let _no_output_of_prior_pics_flag = r.read_flag();
    }

    // 3. PPS/SPS Lookup (The most critical part for bit-alignment)
    sh.slice_pic_parameter_set_id = r.read_ue();

    let pps = pps_storage[sh.slice_pic_parameter_set_id as usize]
        .as_ref()
        .ok_or_else(|| NalError::Generic("PPS not found".into()))?;

    let sps = sps_storage[pps.sps_id as usize]
        .as_ref()
        .ok_or_else(|| NalError::Generic("SPS not found".into()))?;

    // 4. Dependent Slice Logic
    if !sh.first_slice_segment_in_pic_flag {
        if pps.dependent_slice_segments_enabled_flag {
            sh.dependent_slice_segment_flag = r.read_flag();
        }
        let pic_size_in_ctbs_y = sps.pic_width_in_ctbs_y * sps.pic_height_in_ctbs_y;
        let address_length = (pic_size_in_ctbs_y as f64).log2().ceil() as u8;
        sh.slice_segment_address = r.get_bits(address_length);
    }

    if !sh.dependent_slice_segment_flag {
        if pps.num_extra_slice_header_bits > 0 {
            r.get_bits(pps.num_extra_slice_header_bits as u8);
        }

        sh.slice_type = SliceType::try_from(r.read_ue())?;

        if pps.output_flag_present_flag {
            let _pic_output_flag = r.read_flag();
        }

        if sps.separate_color_plane_flag {
            let _colour_plane_id = r.get_bits(2);
        }

        // 5. POC LSB (Skipped for IDR, present for CRA/Trailing)
        if !is_idr {
            sh.slice_pic_order_cnt_lsb = r.get_bits(sps.log2_max_pic_order_cnt_lsb as u8);

            let short_term_ref_pic_set_sps_flag = r.read_flag();
            if !short_term_ref_pic_set_sps_flag {
                // Inline RPS parsing would go here
            } else if sps.num_short_term_ref_pic_sets > 1 {
                let num_bits = (sps.num_short_term_ref_pic_sets as f64).log2().ceil() as u8;
                r.get_bits(num_bits);
            }

            if sps.sps_temporal_mvp_enabled_flag {
                let _slice_temporal_mvp_enabled_flag = r.read_flag();
            }
        }

        // 6. SAO Flags
        if sps.sample_adaptive_offset_enabled_flag {
            sh.slice_sao_luma_flag = r.read_flag();
            sh.slice_sao_chroma_flag = r.read_flag();
        }

        // 7. Temporal/Inter Logic (Skipped for I-Slices)
        if sh.slice_type == SliceType::P || sh.slice_type == SliceType::B {
            let num_ref_idx_active_override_flag = r.read_flag();
            if num_ref_idx_active_override_flag {
                r.read_ue(); // l0
                if sh.slice_type == SliceType::B {
                    r.read_ue(); // l1
                }
            }
        }

        // 8. QP Delta (se(v))
        sh.slice_qp_delta = r.read_se();

        // 9. Deblocking Filter
        if pps.deblocking_filter_control_present_flag {
            if pps.deblocking_filter_override_enabled_flag {
                let deblocking_filter_override_flag = r.read_flag();
                if deblocking_filter_override_flag {
                    let _slice_deblocking_filter_disabled_flag = r.read_flag();
                    if !_slice_deblocking_filter_disabled_flag {
                        r.read_se(); // beta_offset_div2
                        r.read_se(); // tc_offset_div2
                    }
                }
            }
        }

        // 10. Loop Filter Across Slices (The 1-bit drift culprit)
        let is_sao_enabled = sps.sample_adaptive_offset_enabled_flag
            && (sh.slice_sao_luma_flag || sh.slice_sao_chroma_flag);
        let is_dbf_enabled = !pps.deblocking_filter_disabled_flag;

        if pps.loop_filter_across_slices_enabled_flag && (is_sao_enabled || is_dbf_enabled) {
            let _slice_loop_filter_across_slices_enabled_flag = r.read_flag();
        }
    }

    // 11. WPP Entry Point Offsets (The "28 Bytes" Culprit)
    if pps.tiles_enabled_flag || pps.entropy_coding_sync_enabled_flag {
        let num_entry_point_offsets = r.read_ue();
        if num_entry_point_offsets > 0 {
            let offset_len_minus1 = r.read_ue();
            for _ in 0..num_entry_point_offsets {
                r.get_bits((offset_len_minus1 + 1) as u8);
            }
        }
    }

    debug_more!("Before byte alignment, start position :{}",r.byte_position());
    // 12. Final Alignment
    r.byte_align();

    // sh.cabac_start_position = (r.position * 8 - r.bits_left) / 8
    sh.cabac_start_position = r.byte_position();
    debug_more!("cabac start position: {}", sh.cabac_start_position);

    debug_more!(false=>"{:#?}",sh);
    Ok(sh)
}
