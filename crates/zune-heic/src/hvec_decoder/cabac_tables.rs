
/// State transition table when the Most Probable Symbol (MPS) is decoded.
/// (State 63 is a special termination state for the end of the slice).
pub const TRANSITION_MPS: [u8; 64] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
    51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 62, 63
];

/// State transition table when the Least Probable Symbol (LPS) is decoded.
pub const TRANSITION_LPS: [u8; 64] = [
    0, 0, 1, 2, 2, 4, 4, 5, 6, 7, 8, 9, 9, 11, 11, 12, 13, 13, 15, 15, 16, 16, 18, 18, 19, 19, 21,
    21, 22, 22, 23, 24, 24, 25, 26, 26, 27, 27, 28, 29, 29, 30, 30, 30, 31, 32, 32, 33, 33, 33, 34,
    34, 35, 35, 35, 36, 36, 36, 37, 37, 37, 38, 38, 63
];

/// A 64x4 lookup table.
/// Row = The current probability state (p_state) [0-63]
/// Col = The current range quantized to 4 bins: (range >> 6) & 3
#[rustfmt::skip]
pub const RANGE_LPS_TABLE: [[u8; 4]; 64] = [
    [128, 176, 208, 240], [128, 167, 197, 227], [128, 158, 187, 216], [123, 150, 178, 205],
    [116, 142, 169, 195], [111, 135, 160, 185], [105, 128, 152, 175], [100, 122, 144, 166],
    [ 95, 116, 137, 158], [ 90, 110, 130, 150], [ 85, 104, 123, 142], [ 81,  99, 117, 135],
    [ 77,  94, 111, 128], [ 73,  89, 105, 122], [ 69,  85, 100, 116], [ 66,  80,  95, 110],
    [ 62,  76,  90, 104], [ 59,  72,  86,  99], [ 56,  69,  81,  94], [ 53,  65,  77,  89],
    [ 51,  62,  73,  85], [ 48,  59,  69,  80], [ 46,  56,  66,  76], [ 43,  53,  63,  72],
    [ 41,  50,  59,  69], [ 39,  48,  56,  65], [ 37,  45,  54,  62], [ 35,  43,  51,  59],
    [ 33,  41,  48,  56], [ 32,  39,  46,  53], [ 30,  37,  43,  50], [ 29,  35,  41,  48],
    [ 27,  33,  39,  45], [ 26,  31,  37,  43], [ 24,  30,  35,  41], [ 23,  28,  33,  39],
    [ 22,  27,  32,  37], [ 21,  26,  30,  35], [ 20,  24,  29,  33], [ 19,  23,  27,  31],
    [ 18,  22,  26,  30], [ 17,  21,  25,  28], [ 16,  20,  23,  27], [ 15,  19,  22,  25],
    [ 14,  18,  21,  24], [ 14,  17,  20,  23], [ 13,  16,  19,  22], [ 12,  15,  18,  21],
    [ 12,  14,  17,  20], [ 11,  14,  16,  19], [ 11,  13,  15,  18], [ 10,  12,  15,  17],
    [ 10,  12,  14,  16], [  9,  11,  13,  15], [  9,  11,  12,  14], [  8,  10,  12,  14],
    [  8,   9,  11,  13], [  7,   9,  11,  12], [  7,   9,  10,  12], [  7,   8,  10,  11],
    [  6,   8,   9,  11], [  6,   7,   9,  10], [  6,   7,   8,   9], [  2,   2,   2,   2]
];

const CNU: u8 = 154;

#[rustfmt::skip]
pub const CABAC_INIT_VALUES: [[u8; 151]; 3] = [
    // init_type 0: B slices
    [
        // sao_merge_flag (1)
        153,
        // sao_type_idx (1)
        185,
        // split_coding_unit_flag (3)
        107, 139, 126,
        // cu_transquant_bypass_flag (1)
        154,
        // skip_flag (3)
        197, 185, 201,
        // cu_qp_delta_abs (2)
        154, 154,
        // pred_mode_flag (1)
        134,
        // part_mode (4)
        154, 139, 154, 154,
        // prev_intra_luma_pred_mode (1)
        183,
        // intra_chroma_pred_mode (1)
        152,
        // merge_flag (1)
        154,
        // merge_idx (1)
        137,
        // inter_pred_idc (5)
        95, 79, 63, 31, 31,
        // ref_idx_l0 (2)
        153, 153,
        // ref_idx_l1 (2)
        153, 153,
        // abs_mvd_greater0_flag (2)
        169, 198,
        // abs_mvd_greater1_flag (2)
        169, 198,
        // mvp_lx_flag (1)
        168,
        // no_residual_data_flag (1)
        79,
        // split_transform_flag (3)
        224, 167, 122,
        // cbf_luma (2)
        153, 111,
        // cbf_cb, cbf_cr (4)
        149, 92, 167, 154,
        // abs_delta_qp (1) — actually part of cu_qp_delta
        154,
        // last_sig_coeff_x_prefix (15)
        125, 110, 94, 110,  95,  79, 125, 111, 110,  78, 110, 111, 111,  95,  94,
        // last_sig_coeff_y_prefix (15)
        125, 110,  94, 110,  95,  79, 125, 111, 110,  78, 110, 111, 111,  95,  94,
        // significant_coeff_group_flag (4)
        121, 140,  61, 154,
        // significant_coeff_flag (42)
        170, 154, 139, 153, 139, 123, 123,  63, 124, 166, 183, 140, 136, 153, 154,
        166, 183, 140, 136, 153, 154, 166, 183, 140, 136, 153, 154, 170, 153, 138,
        138, 122, 121, 122, 121, 167, 151, 183, 140, 151, 183, 140,
        // coeff_abs_level_greater1_flag (24)
        154, 196, 167, 167, 154, 152, 167, 182, 182, 134, 149, 136,
        153, 121, 136, 137, 169, 194, 166, 167, 154, 167, 137, 182,
        // coeff_abs_level_greater2_flag (6)
        107, 167,  91, 107, 107, 167,
    ],

    // init_type 1: P slices
    [
        // sao_merge_flag (1)
        153,
        // sao_type_idx (1)
        185,
        // split_coding_unit_flag (3)
        107, 139, 126,
        // cu_transquant_bypass_flag (1)
        154,
        // skip_flag (3)
        197, 185, 201,
        // cu_qp_delta_abs (2)
        154, 154,
        // pred_mode_flag (1)
        149,
        // part_mode (4)
        154, 139, 154, 154,
        // prev_intra_luma_pred_mode (1)
        183,
        // intra_chroma_pred_mode (1)
        152,
        // merge_flag (1)
        110,
        // merge_idx (1)
        122,
        // inter_pred_idc (5)
        95, 79, 63, 31, 31,
        // ref_idx_l0 (2)
        153, 153,
        // ref_idx_l1 (2)
        153, 153,
        // abs_mvd_greater0_flag (2)
        140, 198,
        // abs_mvd_greater1_flag (2)
        140, 198,
        // mvp_lx_flag (1)
        168,
        // no_residual_data_flag (1)
        79,
        // split_transform_flag (3)
        224, 167, 122,
        // cbf_luma (2)
        153, 111,
        // cbf_cb, cbf_cr (4)
        149,  92, 167, 154,
        // abs_delta_qp (1)
        154,
        // last_sig_coeff_x_prefix (15)
        125, 110,  94, 110,  95,  79, 125, 111, 110,  78, 110, 111, 111,  95,  94,
        // last_sig_coeff_y_prefix (15)
        125, 110,  94, 110,  95,  79, 125, 111, 110,  78, 110, 111, 111,  95,  94,
        // significant_coeff_group_flag (4)
        121, 140,  61, 154,
        // significant_coeff_flag (42)
        155, 154, 139, 153, 139, 123, 123,  63, 153, 166, 183, 140, 136, 153, 154,
        166, 183, 140, 136, 153, 154, 166, 183, 140, 136, 153, 154, 170, 153, 123,
        123, 107, 121, 107, 121, 167, 151, 183, 140, 151, 183, 140,
        // coeff_abs_level_greater1_flag (24)
        154, 196, 196, 167, 154, 152, 167, 182, 182, 134, 149, 136,
        153, 121, 136, 122, 169, 208, 166, 167, 154, 152, 167, 182,
        // coeff_abs_level_greater2_flag (6)
        107, 167,  91, 107, 107, 167,
    ],

    // init_type 2: I slices
    [
        // sao_merge_flag (1)
        153,
        // sao_type_idx (1)
        200,
        // split_coding_unit_flag (3)
        139, 141, 157,
        // cu_transquant_bypass_flag (1)
        154,
        // skip_flag (3)
        CNU, CNU, CNU,
        // cu_qp_delta_abs (2)
        154, 154,
        // pred_mode_flag (1)
        CNU,
        // part_mode (4)
        184, CNU, CNU, CNU,
        // prev_intra_luma_pred_mode (1)
        184,
        // intra_chroma_pred_mode (1)
        63,
        // merge_flag (1)
        CNU,
        // merge_idx (1)
        CNU,
        // inter_pred_idc (5)
        CNU, CNU, CNU, CNU, CNU,
        // ref_idx_l0 (2)
        CNU, CNU,
        // ref_idx_l1 (2)
        CNU, CNU,
        // abs_mvd_greater0_flag (2)
        CNU, CNU,
        // abs_mvd_greater1_flag (2)
        CNU, CNU,
        // mvp_lx_flag (1)
        CNU,
        // no_residual_data_flag (1)
        CNU,
        // split_transform_flag (3)
        153, 138, 138,
        // cbf_luma (2)
        111, 141,
        // cbf_cb, cbf_cr (4)
        94, 138, 182, 154,
        // abs_delta_qp (1)
        154,
        // last_sig_coeff_x_prefix (15)
        110, 110,  94, 125, 110,  95,  79, 125, 111, 110,  78, 110, 111, 111,  95,
        // last_sig_coeff_y_prefix (15)
        110, 110,  94, 125, 110,  95,  79, 125, 111, 110,  78, 110, 111, 111,  95,
        // significant_coeff_group_flag (4)
        91, 171, 134, 141,
        // significant_coeff_flag (42)
        111, 111, 125, 110, 110,  94, 124, 108, 124, 107, 125, 141, 179, 153, 125,
        107, 125, 141, 179, 153, 125, 107, 125, 141, 179, 153, 125, 140, 139, 182,
        182, 152, 136, 152, 136, 153, 136, 139, 111, 136, 139, 111,
        // coeff_abs_level_greater1_flag (24)
        140,  92, 137, 138, 140, 152, 138, 139, 153,  74, 149,  92,
        139, 107, 122, 152, 140, 179, 166, 182, 140, 227, 122, 197,
        // coeff_abs_level_greater2_flag (6)
        138, 153, 136, 167, 152, 152,
    ],
];
