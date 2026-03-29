/// State transition table when the Most Probable Symbol (MPS) is decoded.
/// (State 63 is a special termination state for the end of the slice).
#[rustfmt::skip]

pub const TRANSITION_MPS: [u8; 64] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
    17, 18, 19, 20, 21, 22, 23, 24, 25, 26,27, 28, 29, 30, 
    31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44,
    45, 46, 47, 48, 49, 50,51, 52, 53, 54, 55, 56, 57, 58, 
    59, 60, 61, 62, 62, 63
];

/// State transition table when the Least Probable Symbol (LPS) is decoded.
#[rustfmt::skip]
pub const TRANSITION_LPS: [u8; 64] = [
    0, 0, 1, 2, 2, 4, 4, 5, 6, 7, 8, 9, 9, 11, 11, 12, 13, 13, 15, 15, 16, 16, 18, 18, 19, 19, 21,
    21, 22, 22, 23, 24, 24, 25, 26, 26, 27, 27, 28, 29, 29, 30, 30, 30, 31, 32, 32, 33, 33, 33, 34,
    34, 35, 35, 35, 36, 36, 36, 37, 37, 37, 38, 38, 63
];
#[rustfmt::skip]
pub const  RENORM_TABLE: [u8; 32] = [
    6, 5, 4, 4, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];
static NEXT_STATE_MPS: [u8; 64] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
    51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 62, 63
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
pub const CONTEXT_MODEL_SAO_MERGE_FLAG: usize = 0;
pub const CONTEXT_MODEL_SAO_TYPE_IDX: usize = 1;
pub const CONTEXT_MODEL_SPLIT_CU_FLAG: usize = 2;
pub const CONTEXT_MODEL_CU_SKIP_FLAG: usize = 5;
pub const CONTEXT_MODEL_PART_MODE: usize = 8;
pub const CONTEXT_MODEL_PREV_INTRA_LUMA_PRED_FLAG: usize = 12;
pub const CONTEXT_MODEL_INTRA_CHROMA_PRED_MODE: usize = 13;
pub const CONTEXT_MODEL_CBF_LUMA: usize = 14;
pub const CONTEXT_MODEL_CBF_CHROMA: usize = 16;
pub const CONTEXT_MODEL_SPLIT_TRANSFORM_FLAG: usize = 20;
pub const CONTEXT_MODEL_CU_CHROMA_QP_OFFSET_FLAG: usize = 23;
pub const CONTEXT_MODEL_CU_CHROMA_QP_OFFSET_IDX: usize = 24;
pub const CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_X_PREFIX: usize = 25;
pub const CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_Y_PREFIX: usize = 43;
pub const CONTEXT_MODEL_CODED_SUB_BLOCK_FLAG: usize = 61;
pub const CONTEXT_MODEL_SIGNIFICANT_COEFF_FLAG: usize = 65;
// Note: SIGNIFICANT_COEFF has 42 + 2 = 44 total contexts
pub const CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER1_FLAG: usize = 109;
pub const CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER2_FLAG: usize = 133;
pub const CONTEXT_MODEL_CU_QP_DELTA_ABS: usize = 139;
pub const CONTEXT_MODEL_TRANSFORM_SKIP_FLAG: usize = 141;
pub const CONTEXT_MODEL_RDPCM_FLAG: usize = 143;
pub const CONTEXT_MODEL_RDPCM_DIR: usize = 145;
pub const CONTEXT_MODEL_MERGE_FLAG: usize = 147;
pub const CONTEXT_MODEL_MERGE_IDX: usize = 148;
pub const CONTEXT_MODEL_PRED_MODE_FLAG: usize = 149;
pub const CONTEXT_MODEL_ABS_MVD_GREATER01_FLAG: usize = 150;
pub const CONTEXT_MODEL_MVP_LX_FLAG: usize = 152;
pub const CONTEXT_MODEL_RQT_ROOT_CBF: usize = 153;
pub const CONTEXT_MODEL_REF_IDX_LX: usize = 154;
pub const CONTEXT_MODEL_INTER_PRED_IDC: usize = 156;
pub const CONTEXT_MODEL_CU_TRANSQUANT_BYPASS_FLAG: usize = 161;
pub const CONTEXT_MODEL_LOG2_RES_SCALE_ABS_PLUS1: usize = 162;
pub const CONTEXT_MODEL_RES_SCALE_SIGN_FLAG:usize=170;
// ========================================================================
// HEVC CABAC INITIALIZATION VALUES (libde265 / Table 9-4)
// ========================================================================

pub const INIT_SPLIT_CU: [[u8; 3]; 3] = [
    [139, 141, 157], // B
    [107, 139, 126], // P
    [107, 139, 126]  // I
];

pub const INIT_CU_SKIP: [[u8; 3]; 2] = [
    [197, 185, 201], // B
    [197, 185, 201]  // P
];

pub const INIT_PART_MODE: [u8; 9] = [184, 154, 139, 154, 154, 154, 139, 154, 154];

pub const INIT_PREV_INTRA: [u8; 3] = [184, 154, 183];

pub const INIT_CHROMA_PRED: [u8; 3] = [63, 152, 152];

pub const INIT_CBF_LUMA: [u8; 4] = [111, 141, 153, 111];

pub const INIT_CBF_CHROMA: [u8; 12] = [
    94, 138, 182, 154, // B
    149, 107, 167, 154, // P
    149, 92, 167, 154 // I
];

pub const INIT_SPLIT_TRANS: [u8; 9] = [
    153, 138, 138, // B
    124, 138, 94, // P
    224, 167, 122 // I
];

pub const INIT_LAST_COEFF: [u8; 54] = [
    110, 110, 124, 125, 140, 153, 125, 127, 140, 109, 111, 143, 127, 111, 79, 108, 123,
    63, // B
    125, 110, 94, 110, 95, 79, 125, 111, 110, 78, 110, 111, 111, 95, 94, 108, 123, 108, // P
    125, 110, 124, 110, 95, 94, 125, 111, 111, 79, 125, 126, 111, 111, 79, 108, 123, 93 // I
];

pub const INIT_CODED_SUB: [u8; 12] = [
    91, 171, 134, 141, // B
    121, 140, 61, 154, // P
    121, 140, 61, 154 // I
];

pub const INIT_SIG_COEFF: [[u8; 42]; 3] = [
    [
        111, 111, 125, 110, 110, 94, 124, 108, 124, 107, 125, 141, 179, 153, 125, 107, 125, 141,
        179, 153, 125, 107, 125, 141, 179, 153, 125, 140, 139, 182, 182, 152, 136, 152, 136, 153,
        136, 139, 111, 136, 139, 111
    ],
    [
        155, 154, 139, 153, 139, 123, 123, 63, 153, 166, 183, 140, 136, 153, 154, 166, 183, 140,
        136, 153, 154, 166, 183, 140, 136, 153, 154, 170, 153, 123, 123, 107, 121, 107, 121, 167,
        151, 183, 140, 151, 183, 140
    ],
    [
        170, 154, 139, 153, 139, 123, 123, 63, 124, 166, 183, 140, 136, 153, 154, 166, 183, 140,
        136, 153, 154, 166, 183, 140, 136, 153, 154, 170, 153, 138, 138, 122, 121, 122, 121, 167,
        151, 183, 140, 151, 183, 140
    ]
];

#[rustfmt::skip]
pub const INIT_SIG_COEFF_SKIP: [[u8; 2]; 3] = [
    [141, 111],
    [140, 140],
    [140, 140]];

pub const INIT_SAO_MERGE: [u8; 3] = [153, 153, 153];

pub const INIT_SAO_TYPE: [u8; 3] = [200, 185, 160];

pub const INIT_QP_DELTA: [u8; 2] = [154, 154];

pub const INIT_TRANSFORM_SKIP: [u8; 2] = [139, 139];

pub const INIT_MERGE_FLAG: [u8; 2] = [110, 154];

pub const INIT_MERGE_IDX: [u8; 2] = [122, 137];

pub const INIT_PRED_MODE: [u8; 2] = [149, 134];

pub const INIT_ABS_MVD: [u8; 4] = [140, 198, 169, 198];

pub const INIT_MVP_LX: [u8; 1] = [168];

pub const INIT_RQT_ROOT: [u8; 1] = [79];

pub const INIT_REF_IDX: [u8; 2] = [153, 153];

pub const INIT_INTER_PRED_IDC: [u8; 5] = [95, 79, 63, 31, 31];

pub const INIT_TRANSQUANT_BYPASS: [u8; 3] = [154, 154, 154];
// Mapping indices:

#[rustfmt::skip]
pub const INIT_GTR_1:[u8;72]=[
    140,92,137,138,140,152,138,139,153,
    74,149, 92,139,107,122,152,140,179,
    166,182,140,227,122,197,154,196,196,
    167,154,152,167,182,182,134,149,136,
    153,121,136,137,169,194,166,167,154,
    167,137,182,154,196,167,167,154,152,
    167,182,182,134,149,136,153,121,136,
    122,169,208,166,167,154,152,167,182
];
#[rustfmt::skip]
pub const INIT_GTR_2:[u8;18]=[
    138, 153,136, 167, 152, 152,
    107, 167, 91, 122, 107, 167,
    107, 167, 91, 107, 107, 167
];
