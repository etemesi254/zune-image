use zune_core::log::{trace};

use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::bitstream::BitReader;
use crate::hevc_decoder::nal_parser::{NalError, NalUnit};
use crate::hevc_decoder::nal_unit_headers::{Pps, ProfileIdc, ProfileTierLevel, ScalingLists, SliceHeader, SliceType, Sps, Vui, VuiVideoFormat};

mod pps;
mod sps;
mod vps;

pub use pps::decode_pps;
pub use sps::decode_sps;
pub use vps::decode_vps;

// ============================================================================
// Helper Functions
// ============================================================================
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

    for item in &mut profile_compatibility_flags {
        *item = r.read_flag();
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


#[rustfmt::skip]
    const SCAN_4X4: [usize; 16] = [
        0, 4, 1, 8, 5, 2, 12, 9, 6, 3, 13, 10, 7, 14, 11, 15
    ];
#[rustfmt::skip]
    const SCAN_8X8: [usize; 64] = [
        0,  8,  1, 16,  9,  2, 24, 17, 10,  3, 32, 25, 18, 11,  4, 40,
        33, 26, 19, 12,  5, 48, 41, 34, 27, 20, 13,  6, 56, 49, 42, 35,
        28, 21, 14,  7, 57, 50, 43, 36, 29, 22, 15, 58, 51, 44, 37, 30,
        23, 59, 52, 45, 38, 31, 60, 53, 46, 39, 61, 54, 47, 62, 55, 63
    ];
pub fn parse_scaling_list_data(r: &mut BitReader) -> Result<ScalingLists, NalError> {
    let mut sl = ScalingLists::default();

    for size_id in 0..4 {
        let num_matrices = if size_id == 3 { 2 } else { 6 };

        for matrix_id in 0..num_matrices {
            let scaling_list_pred_mode_flag = r.read_flag();

            if scaling_list_pred_mode_flag {
                // DPCM Mode: Decode deltas
                let coef_num = std::cmp::min(64, 1 << (4 + (size_id << 1)));
                let mut next_coef = 8; // Starting value for DPCM is 8

                if size_id > 1 {
                    // Read DC coefficient for 16x16 and 32x32
                    let dc_coef_minus8 = r.read_se();
                    let dc_val = (dc_coef_minus8 + 8) as u8;
                    if size_id == 2 {
                        sl.dc16[matrix_id] = dc_val;
                    } else {
                        sl.dc32[matrix_id] = dc_val;
                    }
                    next_coef = i32::from(dc_val);
                }

                for i in 0..coef_num {
                    let delta = r.read_se();
                    next_coef = (next_coef + (delta as i32) + 256) % 256;

                    // Map to 2D using the diagonal scan order
                    if size_id == 0 {
                        sl.size0[matrix_id][SCAN_4X4[i]] = next_coef as u8;
                    } else {
                        // All larger blocks use the 8x8 scan indices
                        let val = next_coef as u8;
                        match size_id {
                            1 => sl.size1[matrix_id][SCAN_8X8[i]] = val,
                            2 => sl.size2[matrix_id][SCAN_8X8[i]] = val,
                            3 => sl.size3[matrix_id][SCAN_8X8[i]] = val,
                            _ => unreachable!()
                        }
                    }
                }
            } else {
                // Prediction Mode: Copy from a previous matrix
                let pred_matrix_id_delta = r.read_ue() as usize;
                let ref_matrix_id = matrix_id - pred_matrix_id_delta;

                match size_id {
                    0 => sl.size0[matrix_id] = sl.size0[ref_matrix_id],
                    1 => sl.size1[matrix_id] = sl.size1[ref_matrix_id],
                    2 => {
                        sl.size2[matrix_id] = sl.size2[ref_matrix_id];
                        sl.dc16[matrix_id] = sl.dc16[ref_matrix_id];
                    }
                    3 => {
                        sl.size3[matrix_id] = sl.size3[ref_matrix_id];
                        sl.dc32[matrix_id] = sl.dc32[ref_matrix_id];
                    }
                    _ => unreachable!()
                }
            }
        }
    }
    Ok(sl)
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

pub fn decode_slice_header(
    nal: &NalUnit, pps_storage: &[Option<Pps>], sps_storage: &[Option<Sps>], clean_payload: &[u8]
) -> Result<SliceHeader, NalError> {
    // 1. Clean the RBSP first to handle 0x03 Emulation Prevention Bytes
    let mut r = BitReader::new(clean_payload);

    let mut sh = SliceHeader::default();

    // 2. Initial Flags
    sh.first_slice_segment_in_pic_flag = r.read_flag();

    let nal_unit_type = nal.nal_type as u8;
    let is_irap = (16..=23).contains(&nal_unit_type);
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
            r.get_bits(pps.num_extra_slice_header_bits);
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
            sh.slice_pic_order_cnt_lsb = r.get_bits(sps.log2_max_pic_order_cnt_lsb);

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

        // 8.5 Chroma QP Offsets (se(v))
        if pps.slice_chroma_qp_offsets_present_flag {
            sh.slice_cb_qp_offset = r.read_se() as i8;
            sh.slice_cr_qp_offset = r.read_se() as i8;
        } else {
            sh.slice_cb_qp_offset = 0;
            sh.slice_cr_qp_offset = 0;
        }

        // 9. Deblocking Filter
        if pps.deblocking_filter_control_present_flag
            && pps.deblocking_filter_override_enabled_flag {
                let deblocking_filter_override_flag = r.read_flag();
                if deblocking_filter_override_flag {
                    let _slice_deblocking_filter_disabled_flag = r.read_flag();
                    if !_slice_deblocking_filter_disabled_flag {
                        r.read_se(); // beta_offset_div2
                        r.read_se(); // tc_offset_div2
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

    debug_more!(
        "Before byte alignment, start position :{}",
        r.byte_position()
    );
    // 12. Final Alignment
    r.byte_align();

    // sh.cabac_start_position = (r.position * 8 - r.bits_left) / 8
    sh.cabac_start_position = r.byte_position();
    debug_more!("cabac start position: {}", sh.cabac_start_position);

    debug_more!(false=>"{:#?}",sh);
    Ok(sh)
}
