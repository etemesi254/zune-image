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
    } else if gamma < 12.92f32 * 0.0030412825601275209f32 {
        gamma * (1f32 / 12.92f32)
    } else if gamma < 1.0f32 {
        ((gamma + 0.0550107189475866f32) / 1.0550107189475866f32).powf(2.4f32)
    } else {
        1.0f32
    }
}

#[inline]
/// Gamma transfer function for sRGB
pub fn srgb_from_linear(linear: f32) -> f32 {
    if linear < 0.0f32 {
        0.0f32
    } else if linear < 0.0030412825601275209f32 {
        linear * 12.92f32
    } else if linear < 1.0f32 {
        1.0550107189475866f32 * linear.powf(1.0f32 / 2.4f32) - 0.0550107189475866f32
    } else {
        1.0f32
    }
}

#[inline]
/// Linear transfer function for Rec.709
pub fn rec709_to_linear(gamma: f32) -> f32 {
    if gamma < 0.0f32 {
        0.0f32
    } else if gamma < 4.5f32 * 0.018053968510807f32 {
        gamma * (1f32 / 4.5f32)
    } else if gamma < 1.0f32 {
        ((gamma + 0.09929682680944f32) / 1.09929682680944f32).powf(1.0f32 / 0.45f32)
    } else {
        1.0f32
    }
}

#[inline]
/// Gamma transfer function for Rec.709
pub fn rec709_from_linear(linear: f32) -> f32 {
    if linear < 0.0f32 {
        0.0f32
    } else if linear < 0.018053968510807f32 {
        linear * 4.5f32
    } else if linear < 1.0f32 {
        1.09929682680944f32 * linear.powf(0.45f32) - 0.09929682680944f32
    } else {
        1.0f32
    }
}

#[inline]
/// Linear transfer function for Smpte 428
pub fn smpte428_to_linear(gamma: f32) -> f32 {
    const SCALE: f32 = 1. / 0.91655527974030934f32;
    gamma.max(0.).powf(2.6f32) * SCALE
}

#[inline]
/// Gamma transfer function for Smpte 428
pub fn smpte428_from_linear(linear: f32) -> f32 {
    const POWER_VALUE: f32 = 1.0f32 / 2.6f32;
    (0.91655527974030934f32 * linear.max(0.)).powf(POWER_VALUE)
}

#[inline]
/// Linear transfer function for Smpte 240
pub fn smpte240_to_linear(gamma: f32) -> f32 {
    if gamma < 0.0 {
        0.0
    } else if gamma < 4.0 * 0.022821585529445 {
        gamma / 4.0
    } else if gamma < 1.0 {
        f32::powf((gamma + 0.111572195921731) / 1.111572195921731, 1.0 / 0.45)
    } else {
        1.0
    }
}

#[inline]
/// Gamma transfer function for Smpte 240
pub fn smpte240_from_linear(linear: f32) -> f32 {
    if linear < 0.0 {
        0.0
    } else if linear < 0.022821585529445 {
        linear * 4.0
    } else if linear < 1.0 {
        1.111572195921731 * f32::powf(linear, 0.45) - 0.111572195921731
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
    const MID_INTERVAL: f32 = 0.00316227766 / 2.;
    if gamma <= 0. {
        MID_INTERVAL
    } else {
        10f32.powf(2.5 * (gamma.min(1.) - 1.))
    }
}

#[inline]
/// Gamma transfer function for Log100Sqrt10
pub fn log100_sqrt10_from_linear(linear: f32) -> f32 {
    if linear <= 0.00316227766 {
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
        -0.27482420670236 * f32::powf(-4.0 * linear, 0.45) + 0.02482420670236
    } else if linear < 0.018053968510807 {
        linear * 4.5
    } else if linear < 1.0 {
        1.09929682680944 * f32::powf(linear, 0.45) - 0.09929682680944
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
        f32::powf((gamma - 0.02482420670236) / -0.27482420670236, 1.0 / 0.45) / -4.0
    } else if gamma < 4.5 * 0.018053968510807 {
        gamma / 4.5
    } else if gamma < 1.0 {
        f32::powf((gamma + 0.09929682680944) / 1.09929682680944, 1.0 / 0.45)
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
    if gamma < -4.5 * 0.018053968510807 {
        f32::powf(
            (-gamma + 0.09929682680944f32) / -1.09929682680944f32,
            1.0f32 / 0.45f32
        )
    } else if gamma < 4.5f32 * 0.018053968510807f32 {
        gamma / 4.5f32
    } else {
        f32::powf(
            (gamma + 0.09929682680944f32) / 1.09929682680944f32,
            1.0f32 / 0.45f32
        )
    }
}

#[inline]
/// Pure gamma transfer function for Iec61966
pub fn iec619662_from_linear(linear: f32) -> f32 {
    if linear < -0.018053968510807f32 {
        -1.09929682680944f32 * f32::powf(-linear, 0.45f32) + 0.09929682680944f32
    } else if linear < 0.018053968510807f32 {
        linear * 4.5f32
    } else {
        1.09929682680944f32 * f32::powf(linear, 0.45f32) - 0.09929682680944f32
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
    Linear
}

impl From<u8> for TransferFunction {
    #[inline]
    fn from(value: u8) -> Self {
        match value {
            0 => TransferFunction::Srgb,
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
            ColorCharacteristics::Linear => Self::Linear
        }
    }
}
impl TransferFunction {
    #[inline]
    pub fn linearize(&self, v: f32) -> f32 {
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
            TransferFunction::Iec61966 => iec61966_to_linear(v)
        }
    }

    #[inline]
    pub fn gamma(&self, v: f32) -> f32 {
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
            TransferFunction::Iec61966 => iec619662_from_linear(v)
        }
    }
}
