use crate::hevc_decoder::nal_parser::NalError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProfileIdc {
    Main = 1,
    Main10 = 2,
    MainStillPicture = 3,
    FormatRangeExtensions = 4
}
#[derive(Debug, Clone)]
pub struct Vps {
    pub vps_id:                u8,
    pub max_layers:            u8,
    pub max_sub_layers:        u8,
    pub profile_info:          ProfileTierLevel,
    // Buffering (Crucial for DPB memory management)
    // We store these as Vecs or small arrays to handle sub-layers
    pub max_dec_pic_buffering: Vec<u32>,
    pub max_num_reorder_pics:  Vec<u32>
}
///
#[derive(Debug, Clone, Copy)]
pub struct ProfileTierLevel {
    pub profile_space:               u8,
    pub tier_flag:                   u8,
    pub profile_idc:                 ProfileIdc,
    pub profile_compatibility_flags: [bool; 32],
    pub progressive_source_flag:     u8,
    pub interlaced_source_flag:      u8,
    pub non_packed_constraint_flag:  u8,
    pub frame_only_constraint_flag:  u8,
    pub level_idc:                   u8
}

#[derive(Debug, Clone, Default)]
pub struct Vui {
    pub aspect_ratio_idc: u8,
    pub sar_width: u16,
    pub sar_height: u16,
    pub video_signal_type_present_flag: bool,
    pub video_format: VuiVideoFormat,
    pub video_full_range_flag: bool,
    pub colour_description_present_flag: bool,
    pub colour_primaries: u8,
    pub transfer_characteristics: u8,
    pub matrix_coeffs: u8,
    pub chroma_loc_info_present_flag: bool,
    pub chroma_sample_loc_type_top_field: u64,
    pub chroma_sample_loc_type_bottom_field: u64,
    pub tiles_fixed_structure_flag: bool,
    pub motion_vectors_over_pic_boundaries_flag: bool,
    pub min_spatial_segmentation_idc: u64,
    pub max_bytes_per_pic_denom: u64,
    pub max_bits_per_min_cu_denom: u64,
    pub log2_max_mv_length_horizontal: u64,
    pub log2_max_mv_length_vertical: u64
}

#[derive(Debug, Clone, Default)]
pub struct Sps {
    pub vps_id:                    u64,
    pub sps_id:                    u64,
    pub max_sub_layers:            u64,
    pub chroma_format:             ChromaFormat,
    pub separate_color_plane_flag: bool,

    // Dimensions
    pub pic_width_in_luma_samples:  u64,
    pub pic_height_in_luma_samples: u64,

    // Conformance Window (Crop Info)
    pub conformance_window_flag: bool,
    pub conf_win_left_offset:    u64,
    pub conf_win_right_offset:   u64,
    pub conf_win_top_offset:     u64,
    pub conf_win_bottom_offset:  u64,

    // Bit Depths
    pub bit_depth_luma:             u8,
    pub bit_depth_chroma:           u8,
    pub log2_max_pic_order_cnt_lsb: u8,

    // Sub-layer Ordering Info
    pub max_dec_pic_buffering:      [u64; 8],
    pub max_num_reorder_pics:       [u64; 8],
    pub max_latency_increase_plus1: [u64; 8],

    // CTB / Transform Sizes
    pub log2_min_luma_coding_block_size:          u8,
    pub log2_diff_max_min_luma_coding_block_size: u8,
    pub log2_min_transform_block_size:            u8,
    pub log2_diff_max_min_transform_block_size:   u8,
    pub max_transform_hierarchy_depth_inter:      u64,
    pub max_transform_hierarchy_depth_intra:      u64,

    // Features
    pub scaling_list_enabled_flag:           bool,
    pub amp_enabled_flag:                    bool,
    pub sample_adaptive_offset_enabled_flag: bool,
    pub pcm_enabled_flag:                    bool,
    pub sps_temporal_mvp_enabled_flag:       bool,
    pub strong_intra_smoothing_enable_flag:  bool,

    // Derived Values
    pub min_cb_size_y:                 u64,
    pub ctb_size_y:                    u64,
    pub pic_width_in_ctbs_y:           u64,
    pub pic_height_in_ctbs_y:          u64,
    pub log2_ctb_size_y:               u8,
    pub log2_max_transform_block_size: u8,

    // PCM
    pub pcm_loop_filter_disable_flag:                 bool,
    pub pcm_sample_bit_depth_luma:                    u8,
    pub pcm_sample_bit_depth_chroma:                  u8,
    pub log2_min_pcm_luma_coding_block_size:          u8,
    pub log2_diff_max_min_pcm_luma_coding_block_size: u8,

    // Reference Picture Sets
    pub num_short_term_ref_pic_sets: u64,
    pub num_long_term_ref_pics_sps:  u64,

    // optional ptl (generally present)
    pub ptl: Option<ProfileTierLevel>,

    // Optional VUI
    pub vui: Option<Vui>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChromaFormat {
    Monochrome = 0, // 4:0:0
    Yuv420 = 1,     // 4:2:0
    Yuv422 = 2,     // 4:2:2
    Yuv444 = 3      // 4:4:4
}
impl Default for ChromaFormat {
    fn default() -> Self {
        ChromaFormat::Yuv420
    }
}
#[derive(Debug, Clone, Copy)]
pub enum VuiVideoFormat {
    Component,
    PAL,
    NTSC,
    SECAM,
    MAC,
    Unspecified
}
impl Default for VuiVideoFormat {
    fn default() -> Self {
        VuiVideoFormat::Unspecified
    }
}

#[derive(Default, Clone, Debug)]
pub struct PpsRangeExtension {
    pub cb_qp_offset_list:                  [i8; 6],
    pub cr_qp_offset_list:                  [i8; 6],
    pub log2_sao_offset_scale_luma:         u8,
    pub log2_sao_offset_scale_chroma:       u8,
    pub log2_max_transform_skip_block_size: u8,
    pub diff_cu_chroma_qp_offset_depth:     u8,
    pub chroma_qp_offset_list_len:          u8
}
#[derive(Debug, Clone, Default)]
pub struct Pps {
    pub pps_id: u8,
    pub sps_id: u8,

    // Flags
    pub dependent_slice_segments_enabled_flag: bool,
    pub output_flag_present_flag:              bool,
    pub sign_data_hiding_enabled_flag:         bool,
    pub cabac_init_present_flag:               bool,

    // Reference indices
    pub num_ref_idx_l0_default_active: u8,
    pub num_ref_idx_l1_default_active: u8,
    // slice bits
    pub num_extra_slice_header_bits:   u8,

    // Quantization
    pub init_qp_minus26:             i64,
    pub constrained_intra_pred_flag: bool,
    pub transform_skip_enabled_flag: bool,
    pub cu_qp_delta_enabled_flag:    bool,
    pub diff_cu_qp_delta_depth:      u8,

    // Offsets
    pub cb_qp_offset: i64,
    pub cr_qp_offset: i64,
    pub slice_chroma_qp_offsets_present_flag: bool,

    // Weighted Prediction
    pub weighted_pred_flag:   bool,
    pub weighted_bipred_flag: bool,

    // Tiles (Crucial for multi-threading)
    pub tiles_enabled_flag: bool,
    pub entropy_coding_sync_enabled_flag: bool,
    pub num_tile_columns: u64,
    pub num_tile_rows: u64,
    pub uniform_spacing_flag: bool,
    pub column_width: Vec<u64>,
    pub row_height: Vec<u64>,
    pub loop_filter_across_tiles_enabled_flag: bool,

    pub pic_scaling_list_data_present_flag: bool,

    // Loop Filter & Control
    pub loop_filter_across_slices_enabled_flag: bool,
    pub deblocking_filter_control_present_flag: bool,
    pub deblocking_filter_override_enabled_flag: bool,
    pub deblocking_filter_disabled_flag: bool,
    pub beta_offset_div2: i64,
    pub tc_offset_div2: i64,

    // Extensions
    pub lists_modification_present_flag: bool,
    pub log2_parallel_merge_level: u8,
    pub slice_segment_header_extension_present_flag: bool,
    pub transquant_bypass_enabled_flag: bool,
    pub range_extension: Option<PpsRangeExtension>,

    // Derived
    pub pic_init_qp:               i64,
    pub log2_min_cu_qp_delta_size: u8
}

#[derive(Debug, Default, Clone)]
pub struct SliceHeader {
    pub first_slice_segment_in_pic_flag: bool,
    pub dependent_slice_segment_flag: bool,
    pub slice_segment_address: u64,
    pub slice_pic_parameter_set_id: u64,
    pub slice_type: SliceType,
    pub slice_pic_order_cnt_lsb: u64,
    pub cabac_start_position: usize,
    pub slice_qp_delta: i64,
    pub slice_sao_luma_flag: bool,
    pub slice_sao_chroma_flag: bool,
    pub slice_addr_rs: Box<Option<SliceHeader>>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceType {
    B = 0,
    P = 1,
    I = 2
}

impl TryFrom<u64> for SliceType {
    type Error = NalError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::B),
            1 => Ok(Self::P),
            2 => Ok(Self::I),
            _ => Err(NalError::Generic(format!("Invalid slice_type: {}", value)))
        }
    }
}

impl Default for SliceType {
    fn default() -> Self {
        Self::I // Defaulting to I-slice is safest for initialization
    }
}
