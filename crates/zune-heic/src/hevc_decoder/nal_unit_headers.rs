#![allow(dead_code)]
#![allow(clippy::struct_excessive_bools, clippy::struct_field_names)]
use crate::hevc_decoder::nal_parser::NalError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProfileIdc {
    Main = 1,
    Main10 = 2,
    MainStillPicture = 3,
    FormatRangeExtensions = 4,
}
#[derive(Debug, Clone)]
pub struct Vps {
    pub vps_id: u8,
    pub max_layers: u8,
    pub max_sub_layers: u8,
    pub profile_info: ProfileTierLevel,
    // Buffering (Crucial for DPB memory management)
    // We store these as Vecs or small arrays to handle sub-layers
    pub max_dec_pic_buffering: Vec<u32>,
    pub max_num_reorder_pics: Vec<u32>,
}
#[derive(Debug, Clone, Copy)]
pub struct ProfileTierLevel {
    pub profile_space: u8,
    pub tier_flag: u8,
    pub profile_idc: ProfileIdc,
    pub profile_compatibility_flags: [bool; 32],
    pub progressive_source_flag: u8,
    pub interlaced_source_flag: u8,
    pub non_packed_constraint_flag: u8,
    pub frame_only_constraint_flag: u8,
    pub level_idc: u8,
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
    pub log2_max_mv_length_vertical: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Sps {
    pub vps_id: u64,
    pub sps_id: u64,
    pub max_sub_layers: u64,
    pub chroma_format: ChromaFormat,
    pub separate_color_plane_flag: bool,

    // Dimensions
    pub pic_width_in_luma_samples: u64,
    pub pic_height_in_luma_samples: u64,

    // Conformance Window (Crop Info)
    pub conformance_window_flag: bool,
    pub conf_win_left_offset: u64,
    pub conf_win_right_offset: u64,
    pub conf_win_top_offset: u64,
    pub conf_win_bottom_offset: u64,

    // Bit Depths
    pub bit_depth_luma: u8,
    pub bit_depth_chroma: u8,
    pub log2_max_pic_order_cnt_lsb: u8,

    // Sub-layer Ordering Info
    pub max_dec_pic_buffering: [u64; 8],
    pub max_num_reorder_pics: [u64; 8],
    pub max_latency_increase_plus1: [u64; 8],

    // CTB / Transform Sizes
    pub log2_min_luma_coding_block_size: u8,
    pub log2_diff_max_min_luma_coding_block_size: u8,
    pub log2_min_transform_block_size: u8,
    pub log2_diff_max_min_transform_block_size: u8,
    pub max_transform_hierarchy_depth_inter: u64,
    pub max_transform_hierarchy_depth_intra: u64,

    // Features
    pub scaling_list_enabled_flag: bool,
    pub scaling_lists: ScalingLists,
    pub amp_enabled_flag: bool,
    pub sample_adaptive_offset_enabled_flag: bool,
    pub pcm_enabled_flag: bool,
    pub sps_temporal_mvp_enabled_flag: bool,
    pub strong_intra_smoothing_enable_flag: bool,

    // Derived Values
    pub min_cb_size_y: u64,
    pub ctb_size_y: u64,
    pub pic_width_in_ctbs_y: u64,
    pub pic_height_in_ctbs_y: u64,
    pub log2_ctb_size_y: u8,
    pub log2_max_transform_block_size: u8,

    // PCM
    pub pcm_loop_filter_disable_flag: bool,
    pub pcm_sample_bit_depth_luma: u8,
    pub pcm_sample_bit_depth_chroma: u8,
    pub log2_min_pcm_luma_coding_block_size: u8,
    pub log2_diff_max_min_pcm_luma_coding_block_size: u8,

    // Reference Picture Sets
    pub num_short_term_ref_pic_sets: u64,
    pub num_long_term_ref_pics_sps: u64,

    // optional ptl (generally present)
    pub ptl: Option<ProfileTierLevel>,

    // Optional VUI
    pub vui: Option<Vui>,
    pub range_extension: Option<SpsRangeExtension>,
    // derived values
    pub sub_width_c: u8,
    pub sub_height_c: u8,
}
#[derive(Debug, Clone, Default)]
pub struct SpsRangeExtension {
    pub transform_skip_rotation_enabled_flag: bool,
    pub transform_skip_context_enabled_flag: bool,
    pub implicit_rdpcm_enabled_flag: bool,
    pub explicit_rdpcm_enabled_flag: bool,
    pub extended_precision_processing_flag: bool,
    pub intra_smoothing_disabled_flag: bool,
    pub high_precision_offsets_enabled_flag: bool,
    pub persistent_rice_adaptation_enabled_flag: bool,
    pub cabac_bypass_alignment_enabled_flag: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[derive(Default)]
pub enum ChromaFormat {
    Monochrome = 0, // 4:0:0
    #[default]
    Yuv420 = 1, // 4:2:0
    Yuv422 = 2,     // 4:2:2
    Yuv444 = 3,     // 4:4:4
}
impl ChromaFormat {
    pub fn get_subsampling(self) -> (usize, usize) {
        match self {
            ChromaFormat::Monochrome | ChromaFormat::Yuv444 => (1, 1),
            ChromaFormat::Yuv420 => (2, 2),
            ChromaFormat::Yuv422 => (2, 1),
        }
    }
}
#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::upper_case_acronyms)]
pub enum VuiVideoFormat {
    Component,
    PAL,
    NTSC,
    SECAM,
    MAC,
    #[default]
    Unspecified,
}

#[derive(Default, Clone, Debug)]
pub struct PpsRangeExtension {
    pub cb_qp_offset_list: [i8; 6],
    pub cr_qp_offset_list: [i8; 6],
    pub log2_sao_offset_scale_luma: u8,
    pub log2_sao_offset_scale_chroma: u8,
    pub log2_max_transform_skip_block_size: u8,
    pub diff_cu_chroma_qp_offset_depth: u8,
    pub chroma_qp_offset_list_len: u8,
    pub cross_component_prediction_enabled_flag: bool,
}
#[derive(Debug, Clone, Default)]
pub struct Pps {
    pub pps_id: u8,
    pub sps_id: u8,

    // Flags
    pub dependent_slice_segments_enabled_flag: bool,
    pub output_flag_present_flag: bool,
    pub sign_data_hiding_enabled_flag: bool,
    pub cabac_init_present_flag: bool,

    // Reference indices
    pub num_ref_idx_l0_default_active: u8,
    pub num_ref_idx_l1_default_active: u8,
    // slice bits
    pub num_extra_slice_header_bits: u8,

    // Quantization
    pub init_qp_minus26: i64,
    pub constrained_intra_pred_flag: bool,
    pub transform_skip_enabled_flag: bool,
    pub cu_qp_delta_enabled_flag: bool,
    pub diff_cu_qp_delta_depth: u8,

    // Offsets
    pub cb_qp_offset: i64,
    pub cr_qp_offset: i64,
    pub slice_chroma_qp_offsets_present_flag: bool,

    // Weighted Prediction
    pub weighted_pred_flag: bool,
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
    pub pic_scaling_lists: ScalingLists,

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
    pub pic_init_qp: i64,
    pub log2_min_cu_qp_delta_size: u8,
    /// Derived: Map from CTB Raster Scan address to Tile ID
    pub tile_id_rs: Vec<u16>,
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
    pub slice_addr_rs: Box<Option<SliceHeader>>,
    pub slice_cb_qp_offset: i8,
    pub slice_cr_qp_offset: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliceType {
    B = 0,
    P = 1,
    #[default]
    I = 2,
}

impl TryFrom<u64> for SliceType {
    type Error = NalError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::B),
            1 => Ok(Self::P),
            2 => Ok(Self::I),
            _ => Err(NalError::Generic(format!("Invalid slice_type: {value}"))),
        }
    }
}

const DEFAULT_SCALING_8X8_INTRA: [u8; 64] = [
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 17, 16, 17, 16, 17, 18, 17, 18, 18, 17, 18, 21, 19, 20,
    21, 20, 19, 21, 24, 22, 22, 24, 24, 22, 22, 24, 25, 25, 27, 30, 27, 25, 25, 29, 31, 35, 35, 31,
    29, 36, 41, 44, 41, 36, 47, 54, 54, 47, 65, 70, 65, 88, 88, 115,
];

const DEFAULT_SCALING_8X8_INTER: [u8; 64] = [
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 17, 17, 17, 17, 17, 18, 18, 18, 18, 18, 18, 20, 20, 20,
    20, 20, 20, 20, 24, 24, 24, 24, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 28, 28, 28, 28, 28,
    28, 33, 33, 33, 33, 33, 41, 41, 41, 41, 54, 54, 54, 71, 71, 91,
];
#[rustfmt::skip]
const SCAN_8X8: [usize; 64] = [
    0,  8,  1, 16,  9,  2, 24, 17, 10,  3, 32, 25, 18, 11,  4, 40,
    33, 26, 19, 12,  5, 48, 41, 34, 27, 20, 13,  6, 56, 49, 42, 35,
    28, 21, 14,  7, 57, 50, 43, 36, 29, 22, 15, 58, 51, 44, 37, 30,
    23, 59, 52, 45, 38, 31, 60, 53, 46, 39, 61, 54, 47, 62, 55, 63
];
#[derive(Debug, Clone)]
pub struct ScalingLists {
    // 4x4 matrices (6 of them, 16 samples each)
    pub size0: [[u8; 16]; 6],
    // 8x8 matrices (6 of them, 64 samples each)
    pub size1: [[u8; 64]; 6],
    // 16x16 matrices (6 of them, 64 samples each - upsampled later)
    pub size2: [[u8; 64]; 6],
    // 32x32 matrices (2 of them, 64 samples each - upsampled later)
    pub size3: [[u8; 64]; 2],

    // Special DC values for 16x16 and 32x32
    pub dc16: [u8; 6],
    pub dc32: [u8; 2],
}

impl Default for ScalingLists {
    fn default() -> Self {
        let mut sl = Self {
            size0: [[16; 16]; 6],
            size1: [[16; 64]; 6],
            size2: [[16; 64]; 6],
            size3: [[16; 64]; 2],
            dc16: [16; 6],
            dc32: [16; 2],
        };

        // Standard HEVC Scan-Order Defaults
        let scan_intra = DEFAULT_SCALING_8X8_INTRA;
        let scan_inter = DEFAULT_SCALING_8X8_INTER;

        // Temporary buffers for rasterized versions
        let mut raster_intra = [0u8; 64];
        let mut raster_inter = [0u8; 64];

        for i in 0..64 {
            raster_intra[SCAN_8X8[i]] = scan_intra[i];
            raster_inter[SCAN_8X8[i]] = scan_inter[i];
        }

        // Apply to 8x8, 16x16, and 32x32
        for i in 0..6 {
            if i < 3 {
                sl.size1[i] = raster_intra;
                sl.size2[i] = raster_intra;
            } else {
                sl.size1[i] = raster_inter;
                sl.size2[i] = raster_inter;
            }
        }
        sl.size3[0] = raster_intra;
        sl.size3[1] = raster_inter;

        sl
    }
}
