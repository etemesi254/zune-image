use zune_core::log::trace;

use crate::hvec_decoder::bitstream::BitReader;
use crate::hvec_decoder::nal_parser::{NalError, NalUnit};
use crate::hvec_decoder::nal_unit_headers::{ChromaFormat, Pps, ProfileIdc, ProfileTierLevel, SliceHeader, Sps, Vps, Vui, VuiVideoFormat};

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
            limit: VPS_MAX_LAYERS_LIMIT,
            value: max_layers,
            field: "vps_max_layers"
        });
    }

    let max_sub_layers = r.get_bits(3) + 1;
    if max_sub_layers > VPS_MAX_SUBLAYERS_LIMIT {
        return Err(NalError::ParameterOutOfRange {
            limit: VPS_MAX_LAYERS_LIMIT,
            value: max_sub_layers,
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
    trace!("vps: {:?}", vps);

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

    r.skip_bits(44);

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
    const MAX_LUMA_BITDEPTH: u64 = 16;

    let mut r = BitReader::new(nal.payload);
    r.refill();

    let mut sps = Sps::default();

    sps.vps_id = r.get_bits(4);
    sps.max_sub_layers = r.get_bits(3) + 1;

    if sps.max_sub_layers > SPS_MAX_LAYERS_LIMIT {
        return Err(NalError::ParameterOutOfRange {
            limit: SPS_MAX_LAYERS_LIMIT,
            value: sps.max_sub_layers,
            field: "sps_max_sub_layers"
        });
    }

    let _sps_temporal_id_nesting_flag = r.read_flag();

    // Passing true for profile_present_flag, and max_sub_layers - 1
    sps.ptl = decode_profile_data(true, true, &mut r)?;

    let vlc = r.read_ue();
    if vlc >= SPS_MAX_SETS_LIMITS {
        return Err(NalError::ParameterOutOfRange {
            limit: SPS_MAX_SETS_LIMITS,
            value: vlc,
            field: "(sps) vlc"
        });
    }
    sps.sps_id = vlc;

    // --- Decode Chroma type ---
    sps.chroma_format_idc = match r.read_ue() {
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

    sps.separate_color_plane_flag = if sps.chroma_format_idc == ChromaFormat::Yuv444 {
        r.read_flag() as u64
    } else {
        0
    };

    // --- Picture Size ---
    sps.pic_width_in_luma_samples = r.read_ue();
    sps.pic_height_in_luma_samples = r.read_ue();

    if sps.pic_width_in_luma_samples > MAX_PICTURE_WIDTH {
        return Err(NalError::ParameterOutOfRange {
            limit: MAX_PICTURE_WIDTH,
            value: sps.pic_width_in_luma_samples,
            field: "(sps) width"
        });
    }
    if sps.pic_height_in_luma_samples > MAX_PICTURE_HEIGHT {
        return Err(NalError::ParameterOutOfRange {
            limit: MAX_PICTURE_HEIGHT,
            value: sps.pic_height_in_luma_samples,
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
    sps.bit_depth_luma = r.read_ue() + 8;
    if sps.bit_depth_luma > MAX_LUMA_BITDEPTH {
        return Err(NalError::ParameterOutOfRange {
            limit: MAX_LUMA_BITDEPTH,
            value: sps.bit_depth_luma,
            field: "bit_depth (luma)"
        });
    }

    sps.bit_depth_chroma = r.read_ue() + 8;
    if sps.bit_depth_chroma > MAX_LUMA_BITDEPTH {
        return Err(NalError::ParameterOutOfRange {
            limit: MAX_LUMA_BITDEPTH,
            value: sps.bit_depth_chroma,
            field: "bit_depth (chroma)"
        });
    }
    if sps.bit_depth_chroma != sps.bit_depth_luma {
        return Err(NalError::Generic(format!(
            "Luma and Chroma Bit depth differ (luma={}, chroma={})",
            sps.bit_depth_luma, sps.bit_depth_chroma
        )));
    }

    sps.log2_max_pic_order_cnt_lsb = r.read_ue() + 4;

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
    sps.log2_min_luma_coding_block_size = r.read_ue() + 3;
    sps.log2_diff_max_min_luma_coding_block_size = r.read_ue();
    sps.log2_min_transform_block_size = r.read_ue() + 2;
    sps.log2_diff_max_min_transform_block_size = r.read_ue();
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
    let num_short_term_ref_pic_sets = r.read_ue() as usize;
    // Track POC deltas across loop iterations for inter-RPS prediction
    let mut num_delta_pocs = [0usize; 65];

    for i in 0..num_short_term_ref_pic_sets {
        parse_short_term_ref_pic_set(&mut r, i, num_short_term_ref_pic_sets, &mut num_delta_pocs)?;
    }

    let long_term_ref_pics_present_flag = r.read_flag();
    if long_term_ref_pics_present_flag {
        let num_long_term_ref_pics_sps = r.read_ue();
        for _ in 0..num_long_term_ref_pics_sps {
            r.get_bits(sps.log2_max_pic_order_cnt_lsb as u8);
            r.read_flag(); // used_by_curr_pic_lt_sps_flag
        }
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
        sps.log2_min_luma_coding_block_size + sps.log2_diff_max_min_luma_coding_block_size;
    sps.ctb_size_y = 1 << log2_ctb_size_y;

    // Integer math equivalent of ceil(width / ctb_size)
    sps.pic_width_in_ctbs_y = (sps.pic_width_in_luma_samples + sps.ctb_size_y - 1) / sps.ctb_size_y;
    sps.pic_height_in_ctbs_y =
        (sps.pic_height_in_luma_samples + sps.ctb_size_y - 1) / sps.ctb_size_y;

    if (true) {
        println!("{:#?}", sps);
    }

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
    let mut r = BitReader::new(nal.payload);
    r.refill();

    let mut pps = Pps::default();

    pps.pps_id = r.read_ue();
    pps.sps_id = r.read_ue();

    pps.dependent_slice_segments_enabled_flag = r.read_flag();
    pps.output_flag_present_flag = r.read_flag();
    pps.num_extra_slice_header_bits = r.get_bits(3) as u8; // Store this! Slices need it.
    pps.sign_data_hiding_enabled_flag = r.read_flag();
    pps.cabac_init_present_flag = r.read_flag();

    pps.num_ref_idx_l0_default_active = r.read_ue() + 1;
    pps.num_ref_idx_l1_default_active = r.read_ue() + 1;

    pps.init_qp_minus26 = r.read_se();
    pps.constrained_intra_pred_flag = r.read_flag();
    pps.transform_skip_enabled_flag = r.read_flag();

    let cu_qp_delta_enabled_flag = r.read_flag();
    pps.cu_qp_delta_enabled_flag = cu_qp_delta_enabled_flag;
    if cu_qp_delta_enabled_flag {
        pps.diff_cu_qp_delta_depth = r.read_ue();
    }

    pps.cb_qp_offset = r.read_se();
    pps.cr_qp_offset = r.read_se();
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
        }
    }

    pps.pic_scaling_list_data_present_flag = r.read_flag();
    if pps.pic_scaling_list_data_present_flag {
        // We can reuse the same scaling list skipper we wrote for the SPS!
        skip_scaling_list_data(&mut r)?;
    }
    // --- Extensions ---
    pps.lists_modification_present_flag = r.read_flag();
    pps.log2_parallel_merge_level = r.read_ue() + 2;
    pps.slice_segment_header_extension_present_flag = r.read_flag();

    let pps_extension_present_flag = r.read_flag();
    if pps_extension_present_flag {
        // Normally false for basic HEVC. Range extensions go here.
    }

    pps.pic_init_qp = 26 + pps.init_qp_minus26;

    let sps = sps[pps.sps_id as usize]
        .as_ref()
        .ok_or_else(|| NalError::Generic(format!("PPS references missing SPS {}", pps.sps_id)))?;


    let log2_ctb_size_y = sps.log2_min_luma_coding_block_size + sps.log2_diff_max_min_luma_coding_block_size;
    pps.log2_min_cu_qp_delta_size = log2_ctb_size_y - pps.diff_cu_qp_delta_depth;

    if true {
        println!("{:#?}", pps)
    }

    Ok(pps)
}




pub fn decode_slice_header(
    nal: &NalUnit,
    pps_storage: &[Option<Pps>],
    sps_storage: &[Option<Sps>]
) -> Result<SliceHeader, NalError> {
    let mut r = BitReader::new(nal.payload);
    r.refill();

    let mut sh = SliceHeader::default();

    let first_slice_segment_in_pic_flag = r.read_flag();

    // In HEVC, NAL types 16 through 23 are Intra Random Access Point (IRAP) pictures (e.g., IDR)
    let nal_type_u8 = nal.nal_type as u8;
    let is_irap = nal_type_u8 >= 16 && nal_type_u8 <= 23;

    if is_irap {
        let _no_output_of_prior_pics_flag = r.read_flag();
    }

    sh.slice_pic_parameter_set_id = r.read_ue();

    // --- CRITICAL CONTEXT LOOKUP ---
    let pps = pps_storage[sh.slice_pic_parameter_set_id as usize]
        .as_ref()
        .ok_or_else(|| NalError::Generic("Slice references missing PPS".into()))?;

    let sps = sps_storage[pps.sps_id as usize]
        .as_ref()
        .ok_or_else(|| NalError::Generic("Slice references missing SPS".into()))?;

    // --- MAINTAINING BITSTREAM SYNC ---
    // If the PPS defined extra header bits, we MUST skip them now.
    if pps.num_extra_slice_header_bits > 0 {
        trace!("Skipping extra header bits. {}",pps.num_extra_slice_header_bits);
        r.get_bits(pps.num_extra_slice_header_bits as u8);
    }

    // 0 = B (Bi-directional), 1 = P (Predictive), 2 = I (Intra)
    sh.slice_type = r.read_ue();

    // ... The rest of the slice header ...

    Ok(sh)
}