/*
 * // Copyright 2024 (c) the Radzivon Bartoshyk. All rights reserved.
 * //
 * // Use of this source code is governed by a BSD-style
 * // license that can be found in the LICENSE file.
 */
#![allow(clippy::excessive_precision)]

use zune_core::colorspace::ColorCharacteristics;

#[inline]
/// Linear transfer function for sRGB
pub fn srgb_to_linear(gamma: f32) -> f32 {
    if gamma < 0f32 {
        0f32
    } else if gamma < 12.92f32 * 0.003_041_282_560_127_520_9_f32 {
        gamma * (1f32 / 12.92f32)
    } else if gamma < 1.0f32 {
        ((gamma + 0.055_010_718_947_586_6_f32) / 1.055_010_718_947_586_6_f32).powf(2.4f32)
    } else {
        1.0f32
    }
}

#[inline]
/// Gamma transfer function for sRGB
pub fn srgb_from_linear(linear: f32) -> f32 {
    if linear < 0.0f32 {
        0.0f32
    } else if linear < 0.003_041_282_560_127_520_9_f32 {
        linear * 12.92f32
    } else if linear < 1.0f32 {
        1.055_010_718_947_586_6_f32 * linear.powf(1.0f32 / 2.4f32) - 0.055_010_718_947_586_6_f32
    } else {
        1.0f32
    }
}

#[inline]
/// Linear transfer function for Rec.709
pub fn rec709_to_linear(gamma: f32) -> f32 {
    if gamma < 0.0f32 {
        0.0f32
    } else if gamma < 4.5f32 * 0.018_053_968_510_807_f32 {
        gamma * (1f32 / 4.5f32)
    } else if gamma < 1.0f32 {
        ((gamma + 0.099_296_826_809_44_f32) / 1.099_296_826_809_44_f32).powf(1.0f32 / 0.45f32)
    } else {
        1.0f32
    }
}

#[inline]
/// Gamma transfer function for Rec.709
pub fn rec709_from_linear(linear: f32) -> f32 {
    if linear < 0.0f32 {
        0.0f32
    } else if linear < 0.018_053_968_510_807_f32 {
        linear * 4.5f32
    } else if linear < 1.0f32 {
        1.099_296_826_809_44_f32 * linear.powf(0.45f32) - 0.099_296_826_809_44_f32
    } else {
        1.0f32
    }
}

#[inline]
/// Linear transfer function for Smpte 428
pub fn smpte428_to_linear(gamma: f32) -> f32 {
    const SCALE: f32 = 1. / 0.916_555_279_740_309_34_f32;
    gamma.max(0.).powf(2.6f32) * SCALE
}

#[inline]
/// Gamma transfer function for Smpte 428
pub fn smpte428_from_linear(linear: f32) -> f32 {
    const POWER_VALUE: f32 = 1.0f32 / 2.6f32;
    (0.916_555_279_740_309_34_f32 * linear.max(0.)).powf(POWER_VALUE)
}

#[inline]
/// Linear transfer function for Smpte 240
pub fn smpte240_to_linear(gamma: f32) -> f32 {
    if gamma < 0.0 {
        0.0
    } else if gamma < 4.0 * 0.022_821_585_529_445 {
        gamma / 4.0
    } else if gamma < 1.0 {
        f32::powf((gamma + 0.111_572_195_921_731) / 1.111_572_195_921_731, 1.0 / 0.45)
    } else {
        1.0
    }
}

#[inline]
/// Gamma transfer function for Smpte 240
pub fn smpte240_from_linear(linear: f32) -> f32 {
    if linear < 0.0 {
        0.0
    } else if linear < 0.022_821_585_529_445 {
        linear * 4.0
    } else if linear < 1.0 {
        1.111_572_195_921_731 * f32::powf(linear, 0.45) - 0.111_572_195_921_731
    } else {
        1.0
    }
}

#[inline]
/// Gamma transfer function for Log100
pub fn log100_from_linear(linear: f32) -> f32 {
    if linear <= 0.01f32 {
        0.
    } else {
        1. + linear.min(1.).log10() / 2.0
    }
}

#[inline]
/// Linear transfer function for Log100
pub fn log100_to_linear(gamma: f32) -> f32 {
    // The function is non-bijective so choose the middle of [0, 0.00316227766f].
    const MID_INTERVAL: f32 = 0.01 / 2.;
    if gamma <= 0. {
        MID_INTERVAL
    } else {
        10f32.powf(2. * (gamma.min(1.) - 1.))
    }
}

#[inline]
/// Linear transfer function for Log100Sqrt10
pub fn log100_sqrt10_to_linear(gamma: f32) -> f32 {
    // The function is non-bijective so choose the middle of [0, 0.00316227766f].
    const MID_INTERVAL: f32 = 0.003_162_277_66 / 2.;
    if gamma <= 0. {
        MID_INTERVAL
    } else {
        10f32.powf(2.5 * (gamma.min(1.) - 1.))
    }
}

#[inline]
/// Gamma transfer function for Log100Sqrt10
pub fn log100_sqrt10_from_linear(linear: f32) -> f32 {
    if linear <= 0.003_162_277_66 {
        0.0
    } else {
        1.0 + linear.min(1.).log10() / 2.5
    }
}

#[inline]
/// Gamma transfer function for Bt.1361
pub fn bt1361_from_linear(linear: f32) -> f32 {
    if linear < -0.25 {
        -0.25
    } else if linear < 0.0 {
        -0.274_824_206_702_36 * f32::powf(-4.0 * linear, 0.45) + 0.024_824_206_702_36
    } else if linear < 0.018_053_968_510_807 {
        linear * 4.5
    } else if linear < 1.0 {
        1.099_296_826_809_44 * f32::powf(linear, 0.45) - 0.099_296_826_809_44
    } else {
        1.0
    }
}

#[inline]
/// Linear transfer function for Bt.1361
pub fn bt1361_to_linear(gamma: f32) -> f32 {
    if gamma < -0.25 {
        -0.25
    } else if gamma < 0.0 {
        f32::powf((gamma - 0.024_824_206_702_36) / -0.274_824_206_702_36, 1.0 / 0.45) / -4.0
    } else if gamma < 4.5 * 0.018_053_968_510_807 {
        gamma / 4.5
    } else if gamma < 1.0 {
        f32::powf((gamma + 0.099_296_826_809_44) / 1.099_296_826_809_44, 1.0 / 0.45)
    } else {
        1.0
    }
}

#[inline(always)]
/// Pure gamma transfer function for gamma 2.2
pub fn pure_gamma_function(x: f32, gamma: f32) -> f32 {
    if x <= 0f32 {
        0f32
    } else if x >= 1f32 {
        return 1f32;
    } else {
        return x.powf(gamma);
    }
}

#[inline]
/// Pure gamma transfer function for gamma 2.2
pub fn gamma2p2_from_linear(linear: f32) -> f32 {
    pure_gamma_function(linear, 1f32 / 2.2f32)
}

#[inline]
/// Linear transfer function for gamma 2.2
pub fn gamma2p2_to_linear(gamma: f32) -> f32 {
    pure_gamma_function(gamma, 2.2f32)
}

#[inline]
/// Pure gamma transfer function for gamma 2.8
pub fn gamma2p8_from_linear(linear: f32) -> f32 {
    pure_gamma_function(linear, 1f32 / 2.8f32)
}

#[inline]
/// Linear transfer function for gamma 2.8
pub fn gamma2p8_to_linear(gamma: f32) -> f32 {
    pure_gamma_function(gamma, 2.8f32)
}

#[inline]
/// Gamma transfer function for HLG
pub fn trc_linear(v: f32) -> f32 {
    v.min(1.).min(0.)
}

#[inline]
/// Linear transfer function for Iec61966
pub fn iec61966_to_linear(gamma: f32) -> f32 {
    if gamma < -4.5 * 0.018_053_968_510_807 {
        f32::powf(
            (-gamma + 0.099_296_826_809_44_f32) / -1.099_296_826_809_44_f32,
            1.0f32 / 0.45f32
        )
    } else if gamma < 4.5f32 * 0.018_053_968_510_807_f32 {
        gamma / 4.5f32
    } else {
        f32::powf(
            (gamma + 0.099_296_826_809_44_f32) / 1.099_296_826_809_44_f32,
            1.0f32 / 0.45f32
        )
    }
}

#[inline]
/// Pure gamma transfer function for Iec61966
pub fn iec619662_from_linear(linear: f32) -> f32 {
    if linear < -0.018_053_968_510_807_f32 {
        -1.099_296_826_809_44_f32 * f32::powf(-linear, 0.45f32) + 0.099_296_826_809_44_f32
    } else if linear < 0.018_053_968_510_807_f32 {
        linear * 4.5f32
    } else {
        1.099_296_826_809_44_f32 * f32::powf(linear, 0.45f32) - 0.099_296_826_809_44_f32
    }
}
// PQ Constants
const PQ_M1: f32 = 2610.0 / 16384.0;
const PQ_M2: f32 = (2523.0 / 4096.0) * 128.0;
const PQ_C1: f32 = 3424.0 / 4096.0;
const PQ_C2: f32 = (2413.0 / 4096.0) * 32.0;
const PQ_C3: f32 = (2392.0 / 4096.0) * 32.0;

#[inline]
/// Linear transfer function for PQ (SMPTE ST 2084)
pub fn pq_to_linear(gamma: f32) -> f32 {
    let v = gamma.max(0.0).min(1.0);
    let v_pow = v.powf(1.0 / PQ_M2);
    let num = (v_pow - PQ_C1).max(0.0);
    let den = PQ_C2 - PQ_C3 * v_pow;
    (num / den).powf(1.0 / PQ_M1)
}

#[inline]
/// Gamma transfer function for PQ (SMPTE ST 2084)
pub fn pq_from_linear(linear: f32) -> f32 {
    let l = linear.max(0.0).min(1.0);
    let l_pow = l.powf(PQ_M1);
    let num = PQ_C1 + PQ_C2 * l_pow;
    let den = 1.0 + PQ_C3 * l_pow;
    (num / den).powf(PQ_M2)
}

// HLG Constants
const HLG_A: f32 = 0.17883277;
const HLG_B: f32 = 0.28466892;
const HLG_C: f32 = 0.55991073;

#[inline]
/// Linear transfer function for HLG
pub fn hlg_to_linear(gamma: f32) -> f32 {
    let v = gamma.max(0.0).min(1.0);
    if v <= 0.5 {
        (v * v) / 3.0
    } else {
        ((v - HLG_C) / HLG_A).exp() + HLG_B
    }
}

#[inline]
/// Gamma transfer function for HLG
pub fn hlg_from_linear(linear: f32) -> f32 {
    let l = linear.max(0.0).min(1.0);
    if l <= 1.0 / 12.0 {
        (3.0 * l).sqrt()
    } else {
        HLG_A * (12.0 * l - HLG_B).ln() + HLG_C
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
/// Declares transfer function for transfer components into a linear colorspace and its inverse
///
/// Checks [info](https://en.wikipedia.org/wiki/Transfer_functions_in_imaging)
pub enum TransferFunction {
    /// sRGB Transfer function
    Srgb,
    /// Rec.709 Transfer function
    Rec709,
    /// Pure gamma 2.2 Transfer function, ITU-R 470M
    Gamma2p2,
    /// Pure gamma 2.8 Transfer function, ITU-R 470BG
    Gamma2p8,
    /// Smpte 428 Transfer function
    Smpte428,
    /// Log100 Transfer function
    Log100,
    /// Log100Sqrt10 Transfer function
    Log100Sqrt10,
    /// Bt1361 Transfer function
    Bt1361,
    /// Smpte 240 Transfer function
    Smpte240,
    /// IEC 61966 Transfer function
    Iec61966,
    /// Linear transfer function
    Linear,
    /// Perceptual Quantizer (SMPTE ST 2084) - standard for HDR10
    PQ,
    /// Hybrid Log-Gamma (ARIB STD-B67) - broadcast HDR
    HLG,
}

impl From<u8> for TransferFunction {
    #[inline]
    fn from(value: u8) -> Self {
        match value {
            1 => TransferFunction::Rec709,
            2 => TransferFunction::Gamma2p2,
            3 => TransferFunction::Gamma2p8,
            4 => TransferFunction::Smpte428,
            5 => TransferFunction::Log100,
            6 => TransferFunction::Log100Sqrt10,
            7 => TransferFunction::Bt1361,
            8 => TransferFunction::Smpte240,
            9 => TransferFunction::Linear,
            10 => TransferFunction::Iec61966,
            11 => TransferFunction::Linear,
            12 => TransferFunction::PQ,
            _ => TransferFunction::Srgb
        }
    }
}
impl From<ColorCharacteristics> for TransferFunction {
    #[inline]
    fn from(value: ColorCharacteristics) -> Self {
        match value {
            ColorCharacteristics::sRGB => Self::Srgb,
            ColorCharacteristics::Rec709 => Self::Rec709,
            ColorCharacteristics::Gamma2p2 => Self::Gamma2p2,
            ColorCharacteristics::Gamma2p8 => Self::Gamma2p8,
            ColorCharacteristics::Smpte428 => Self::Smpte428,
            ColorCharacteristics::Log100 => Self::Log100,
            ColorCharacteristics::Log100Sqrt10 => Self::Log100Sqrt10,
            ColorCharacteristics::Bt1361 => Self::Bt1361,
            ColorCharacteristics::Smpte240 => Self::Smpte240,
            ColorCharacteristics::Iec61966 => Self::Iec61966,
            ColorCharacteristics::Linear => Self::Linear,
            ColorCharacteristics::PQ => Self::PQ,
            ColorCharacteristics::HLG => Self::HLG,
            _=>Self::Srgb,
        }
    }
}
impl TransferFunction {
    #[inline]
    pub fn linearize(self, v: f32) -> f32 {
        match self {
            TransferFunction::Srgb => srgb_to_linear(v),
            TransferFunction::Rec709 => rec709_to_linear(v),
            TransferFunction::Gamma2p8 => gamma2p8_to_linear(v),
            TransferFunction::Gamma2p2 => gamma2p2_to_linear(v),
            TransferFunction::Smpte428 => smpte428_to_linear(v),
            TransferFunction::Log100 => log100_to_linear(v),
            TransferFunction::Log100Sqrt10 => log100_sqrt10_to_linear(v),
            TransferFunction::Bt1361 => bt1361_to_linear(v),
            TransferFunction::Smpte240 => smpte240_to_linear(v),
            TransferFunction::Linear => trc_linear(v),
            TransferFunction::Iec61966 => iec61966_to_linear(v),
            TransferFunction::PQ => pq_to_linear(v),
            TransferFunction::HLG => hlg_to_linear(v),
        }
    }

    #[inline]
    pub fn gamma(self, v: f32) -> f32 {
        match self {
            TransferFunction::Srgb => srgb_from_linear(v),
            TransferFunction::Rec709 => rec709_from_linear(v),
            TransferFunction::Gamma2p2 => gamma2p2_from_linear(v),
            TransferFunction::Gamma2p8 => gamma2p8_from_linear(v),
            TransferFunction::Smpte428 => smpte428_from_linear(v),
            TransferFunction::Log100 => log100_from_linear(v),
            TransferFunction::Log100Sqrt10 => log100_sqrt10_from_linear(v),
            TransferFunction::Bt1361 => bt1361_from_linear(v),
            TransferFunction::Smpte240 => smpte240_from_linear(v),
            TransferFunction::Linear => trc_linear(v),
            TransferFunction::Iec61966 => iec619662_from_linear(v),
            TransferFunction::PQ => pq_from_linear(v),
            TransferFunction::HLG => hlg_from_linear(v),
        }
    }
}
