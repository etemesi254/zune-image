use crate::hevc_decoder::DEBUG_MORE;
use zune_core::log::warn;
use crate::debug_more;
use crate::hevc_decoder::bitstream::BitReader;
use crate::hevc_decoder::nal_parser::{NalError, NalUnit};
use crate::hevc_decoder::nal_unit_headers::{ChromaFormat, Pps, PpsRangeExtension, Sps};
use crate::hevc_decoder::nal_unit_parsers::skip_scaling_list_data;

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
        let _multilayer_extension_flag = r.read_flag();
        // skips
        // 3d extension flag => 1 bit
        // scc extension flag => 1 bit
        let _extension_6_bits = r.get_bits(6);

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