// ============================================================
//  HEVC NAL Unit Parser
//
//  Responsibilities:
//  - Walk the length-prefixed NAL units inside HevcSample extents
//  - Parse the 2-byte NAL unit header (spec §7.3.1.2)
//  - Strip RBSP emulation prevention bytes from payloads
//  - Hand zero-copy NalUnit views to a caller-supplied visitor
// ============================================================

use std::fmt;

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum NalError {
    /// A length prefix or NAL header read would go past the extent boundary.
    UnexpectedEof {
        extent: usize,
        offset: usize
    },
    /// forbidden_zero_bit in the NAL header was 1 — stream is corrupt.
    ForbiddenBitSet,
    /// nuh_temporal_id_plus1 == 0 is illegal per spec §7.4.2.2.
    InvalidTemporalId,
    ParameterOutOfRange {
        limit: i64,
        value: i64,
        field: &'static str
    },
    Generic(String)
}

impl fmt::Display for NalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { extent, offset } => {
                write!(f, "unexpected EOF in extent {extent} at offset {offset}")
            }
            Self::ForbiddenBitSet => {
                write!(f, "NAL header forbidden_zero_bit is set — corrupt stream")
            }
            Self::InvalidTemporalId => {
                write!(f, "NAL header nuh_temporal_id_plus1 == 0 is illegal")
            }
            Self::ParameterOutOfRange {
                limit,
                value,
                field
            } => {
                write!(
                    f,
                    "NAL header parameter_out_of_bounds (param:{field}) limit={limit} value={value}"
                )
            }
            Self::Generic(msg) => write!(f, "{msg}")
        }
    }
}

// ── NAL unit types (HEVC spec Table 7-1) ─────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NalUnitType {
    // ── VCL: slice data ───────────────────────────────────────────────────
    TrailN = 0,
    TrailR = 1,
    TsaN = 2,
    TsaR = 3,
    StsaN = 4,
    StsaR = 5,
    RadlN = 6,
    RadlR = 7,
    RaslN = 8,
    RaslR = 9,
    IdrWRadl = 19,
    IdrNLp = 20,
    CraNut = 21,

    // ── Non-VCL: parameter sets & metadata ────────────────────────────────
    VpsNut = 32,
    SpsNut = 33,
    PpsNut = 34,
    AudNut = 35,
    EosNut = 36, // End of sequence
    EobNut = 37, // End of bitstream
    FdNut = 38,  // Filler data
    SeiPrefix = 39,
    SeiSuffix = 40 // Any type not listed above. Raw value is preserved for diagnostics.
                   //Unknown(u8)
}

impl NalUnitType {
    fn from_u8(v: u8) -> Result<Self, u8> {
        match v {
            0 => Ok(Self::TrailN),
            1 => Ok(Self::TrailR),
            2 => Ok(Self::TsaN),
            3 => Ok(Self::TsaR),
            4 => Ok(Self::StsaN),
            5 => Ok(Self::StsaR),
            6 => Ok(Self::RadlN),
            7 => Ok(Self::RadlR),
            8 => Ok(Self::RaslN),
            9 => Ok(Self::RaslR),
            19 => Ok(Self::IdrWRadl),
            20 => Ok(Self::IdrNLp),
            21 => Ok(Self::CraNut),
            32 => Ok(Self::VpsNut),
            33 => Ok(Self::SpsNut),
            34 => Ok(Self::PpsNut),
            35 => Ok(Self::AudNut),
            36 => Ok(Self::EosNut),
            37 => Ok(Self::EobNut),
            38 => Ok(Self::FdNut),
            39 => Ok(Self::SeiPrefix),
            40 => Ok(Self::SeiSuffix),
            _ => Err(v)
        }
    }
}

// ── NalUnit ───────────────────────────────────────────────────────────────────
//
// Produced by the parser for each NAL unit found in an extent.
// The `payload` field is a view directly into the caller's extent buffer
// (zero-copy) — it still contains emulation prevention bytes.
// Call `rbsp_payload()` to get a clean copy with those stripped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NalUnit<'a> {
    pub nal_type:    NalUnitType,
    /// nuh_layer_id — always 0 for the base layer in HEIF tiles.
    pub layer_id:    u8,
    /// nuh_temporal_id_plus1 − 1; 0 = base temporal layer.
    pub temporal_id: u8,
    /// Raw bytes after the 2-byte header. May contain 0x000003
    /// emulation prevention sequences.
    pub payload:     &'a [u8]
}

// ── Framing detection ────────────────────────────────────────────────────────

/// The two ways HEVC NAL units can be framed in a byte buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NalFraming {
    /// ISOBMFF / HVCC: each NAL is preceded by a 4-byte BE length field.
    /// Used in HEIF/MP4 containers and the HevcSample extents from the mdat box.
    ///
    ///   [u32 BE length][NAL bytes][u32 BE length][NAL bytes]…
    LengthPrefixed,

    /// Annex B: each NAL is preceded by a 3- or 4-byte start code.
    /// Used in raw bitstream files (.265, .hvc, ffmpeg output, etc.).
    ///
    ///   [0x00 0x00 0x00 0x01][NAL bytes][0x00 0x00 0x00 0x01][NAL bytes]…
    AnnexB,
    /// Raw NAL bytes, e.g already stripped from hvcc length
    RawBytes
}

impl NalFraming {
    /// Sniff framing from the first bytes of a buffer.
    /// Annex B always starts with 0x000001 or 0x00000001.
    pub fn detect(data: &[u8]) -> Self {
        if data.starts_with(&[0x00, 0x00, 0x00, 0x01]) || data.starts_with(&[0x00, 0x00, 0x01]) {
            Self::AnnexB
        } else {
            Self::LengthPrefixed
        }
    }
}

// ── NalParser ────────────────────────────────────────────────────────────────

pub(crate) struct NalParser<'a> {
    extents: &'a [&'a [u8]],
    framing: NalFraming
}

impl<'a> NalParser<'a> {
    /// Create a parser with explicit framing.
    pub fn new(extents: &'a [&'a [u8]], framing: NalFraming) -> Self {
        Self { extents, framing }
    }

    /// Sniff the framing from the first extent automatically.
    /// Use this when loading from files where you don't control the format.
    pub fn new_detect(extents: &'a [&'a [u8]]) -> Self {
        let framing = extents
            .first()
            .map_or(NalFraming::LengthPrefixed, |e| NalFraming::detect(e));
        Self { extents, framing }
    }

    /// Call `visitor` for every NAL unit found across all extents.
    ///
    ///   `Ok(true)`  — continue
    ///   `Ok(false)` — stop early
    ///   `Err(_)`    — propagate and stop
    pub fn for_each_nal<F>(&'a self, mut visitor: F) -> Result<(), NalError>
    where
        F: FnMut(NalUnit<'a>) -> Result<bool, NalError>
    {
        match self.framing {
            NalFraming::LengthPrefixed => self.walk_length_prefixed(&mut visitor),
            NalFraming::AnnexB => self.walk_annex_b(&mut visitor),
            NalFraming::RawBytes => self.walk_raw_bytes(&mut visitor)
        }
    }

    /// Collect all NAL units into a Vec (allocates; use for_each_nal in production).
    pub fn collect(&'a self) -> Result<Vec<NalUnit<'a>>, NalError> {
        let mut out = Vec::new();
        self.for_each_nal(|nal| {
            out.push(nal.clone());
            Ok(true)
        })?;
        Ok(out)
    }

    // ── length-prefixed walker ────────────────────────────────────────────
    fn walk_length_prefixed<F>(&self, mut visitor: F) -> Result<(), NalError>
    where
        F: FnMut(NalUnit<'a>) -> Result<bool, NalError>
    {
        for (extent_idx, &extent) in self.extents.iter().enumerate() {
            let mut remaining = extent;

            while !remaining.is_empty() {
                // Calculate the absolute offset just in case we need to throw an error
                let offset = extent.len() - remaining.len();

                // 1. Read the 4-byte length prefix
                if remaining.len() < 4 {
                    return Err(NalError::UnexpectedEof {
                        extent: extent_idx,
                        offset
                    });
                }
                let (len_bytes, tail) = remaining.split_at(4);
                let nal_len = u32::from_be_bytes(len_bytes.try_into().unwrap()) as usize;

                // 2. Extract the NAL payload
                if tail.len() < nal_len {
                    return Err(NalError::UnexpectedEof {
                        extent: extent_idx,
                        offset: offset + 4
                    });
                }
                let (nal_bytes, tail) = tail.split_at(nal_len);

                // 3. Advance the remaining slice
                remaining = tail;

                // 4. Process the NAL (offset + 4 is the exact start of the payload)
                // NB: CAE, there was something removed here, extent and offset+4
                // if important return
                if let Some(cont) = parse_nal_header(nal_bytes, &mut visitor)?
                    && !cont
                {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    // ── Annex B walker ────────────────────────────────────────────────────
    //
    // Splits on 3-byte (0x000001) or 4-byte (0x00000001) start codes.
    // The start code itself is not part of the NAL unit.
    fn walk_annex_b<F>(&self, visitor: &mut F) -> Result<(), NalError>
    where
        F: FnMut(NalUnit<'a>) -> Result<bool, NalError>
    {
        for extent in self.extents {
            let mut search_start = 0;

            // 1. Find the next 0x00 0x00 0x01 start code
            while let Some(sc_idx) = extent[search_start..]
                .windows(3)
                .position(|w| w == [0, 0, 1])
            {
                let absolute_sc_idx = search_start + sc_idx;
                let nal_start = absolute_sc_idx + 3;

                // 2. Look for the NEXT start code to figure out where this NAL ends
                let next_sc_offset = extent[nal_start..].windows(3).position(|w| w == [0, 0, 1]);

                let nal_end = if let Some(offset) = next_sc_offset {
                    // Backtrack any padding zeros belonging to the next start code
                    // by finding the last non-zero byte in the current slice.
                    extent[nal_start..nal_start + offset]
                        .iter()
                        .rposition(|&b| b != 0)
                        .map_or(nal_start, |last_non_zero_idx| {
                            nal_start + last_non_zero_idx + 1
                        }) // If all zeros, length is 0
                } else {
                    extent.len() // No more start codes; read to the end
                };

                // 3. Process the NAL if it's not empty
                if nal_end > nal_start {
                    let nal_bytes = &extent[nal_start..nal_end];
                    if let Some(cont) = parse_nal_header(nal_bytes, visitor)?
                        && !cont
                    {
                        return Ok(());
                    }
                }

                // 4. Advance the search window exactly to the start of the next start code
                search_start = if let Some(offset) = next_sc_offset {
                    nal_start + offset
                } else {
                    extent.len() // EOF
                };
            }
        }
        Ok(())
    }

    fn walk_raw_bytes<F>(&self, visitor: &mut F) -> Result<(), NalError>
    where
        F: FnMut(NalUnit<'a>) -> Result<bool, NalError>
    {
        for &extent in self.extents {

            // if important return
            if let Some(cont) = parse_nal_header(extent, visitor)?
                && !cont
            {
               return Ok(());
            }
        }

        Ok(())
    }
}

// ── Shared NAL header parser ──────────────────────────────────────────────────
//
// Both walkers feed raw NAL bytes here.  Returns Ok(Some(bool)) where the
// bool mirrors the visitor return, or Ok(None) if the NAL was too short to
// have a header (silently skipped).

fn parse_nal_header<'a, F>(nal_bytes: &'a [u8], visitor: &mut F) -> Result<Option<bool>, NalError>
where
    F: FnMut(NalUnit<'a>) -> Result<bool, NalError>
{
    if nal_bytes.len() < 2 {
        return Ok(None); // too short — skip silently
    }

    let header = u16::from_be_bytes([nal_bytes[0], nal_bytes[1]]);

    if header & 0x8000 != 0 {
        return Err(NalError::ForbiddenBitSet);
    }

    let raw_type = ((header >> 9) & 0x3F) as u8;
    let layer_id = ((header >> 3) & 0x3F) as u8;
    let temp_plus1 = (header & 0x07) as u8;

    if temp_plus1 == 0 {
        return Err(NalError::InvalidTemporalId);
    }

    let nal_type = NalUnitType::from_u8(raw_type)
        .map_err(|e| NalError::Generic(format!("Unknown nal type: {}", e)))?;

    let nal_unit = NalUnit {
        nal_type,
        layer_id,
        temporal_id: temp_plus1 - 1,
        payload: &nal_bytes[2..]
    };

    Ok(Some(visitor(nal_unit)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_extent(nal_type: u8, layer_id: u8, temporal_id_plus1: u8, payload: &[u8]) -> Vec<u8> {
        let header = u16::to_be_bytes(
            (u16::from(nal_type) << 9) | (u16::from(layer_id) << 3) | (temporal_id_plus1 as u16)
        );
        let nal_len = (2 + payload.len()) as u32;
        let mut out = nal_len.to_be_bytes().to_vec();
        out.extend_from_slice(&header);
        out.extend_from_slice(payload);
        out
    }

    // ── header parsing ────────────────────────────────────────────────────

    #[test]
    fn sps_header_parsed() {
        let extent = make_extent(33, 0, 1, &[]);
        let extents: &[&[u8]] = &[&extent];
        let parser = NalParser::new(extents, NalFraming::LengthPrefixed);
        let nals = parser.collect().unwrap();

        assert_eq!(nals.len(), 1);
        assert_eq!(nals[0].nal_type, NalUnitType::SpsNut);
        assert_eq!(nals[0].layer_id, 0);
        assert_eq!(nals[0].temporal_id, 0);
        assert!(nals[0].payload.is_empty());
    }

    #[test]
    fn idr_header_parsed() {
        let extent = make_extent(19, 0, 1, &[0xDE, 0xAD]);
        let extents: &[&[u8]] = &[&extent];
        let parser = NalParser::new(extents, NalFraming::LengthPrefixed);
        let nals = parser.collect().unwrap();

        assert_eq!(nals[0].nal_type, NalUnitType::IdrWRadl);
        assert_eq!(nals[0].payload, &[0xDE, 0xAD]);
    }

    // #[test]
    // fn unknown_nal_type_passes_through() {
    //     // Type 63 is not in the spec table — should become Unknown(63), not an error.
    //     let extent = make_extent(63, 0, 1, &[]);
    //     let extents: &[&[u8]] = &[&extent];
    //     let parser = NalParser::new(extents, NalFraming::LengthPrefixed);
    //     let nals = parser.collect().unwrap();
    //     assert_eq!(nals[0].nal_type, NalUnitType::Unknown(63));
    // }

    #[test]
    fn forbidden_bit_returns_error() {
        // Set forbidden_zero_bit (MSB of first header byte).
        let mut extent = make_extent(33, 0, 1, &[]);
        extent[4] |= 0x80; // byte 4 is first byte of NAL header (after 4-byte length)
        let extents: &[&[u8]] = &[&extent];
        let parser = NalParser::new(extents, NalFraming::LengthPrefixed);
        assert_eq!(parser.collect(), Err(NalError::ForbiddenBitSet));
    }

    #[test]
    fn invalid_temporal_id_returns_error() {
        // temporal_id_plus1 == 0 is illegal.
        let extent = make_extent(33, 0, 0, &[]);
        let extents: &[&[u8]] = &[&extent];
        let parser = NalParser::new(extents, NalFraming::LengthPrefixed);
        assert_eq!(parser.collect(), Err(NalError::InvalidTemporalId));
    }

    #[test]
    fn multiple_nals_in_one_extent() {
        let mut extent = make_extent(32, 0, 1, &[]); // VPS
        extent.extend(make_extent(33, 0, 1, &[])); // SPS
        extent.extend(make_extent(34, 0, 1, &[])); // PPS
        let extents: &[&[u8]] = &[&extent];
        let parser = NalParser::new(extents, NalFraming::LengthPrefixed);
        let nals = parser.collect().unwrap();

        assert_eq!(nals.len(), 3);
        assert_eq!(nals[0].nal_type, NalUnitType::VpsNut);
        assert_eq!(nals[1].nal_type, NalUnitType::SpsNut);
        assert_eq!(nals[2].nal_type, NalUnitType::PpsNut);
    }

    #[test]
    fn early_exit_from_visitor() {
        let mut extent = make_extent(32, 0, 1, &[]);
        extent.extend(make_extent(33, 0, 1, &[]));
        let extents: &[&[u8]] = &[&extent];
        let parser = NalParser::new(extents, NalFraming::LengthPrefixed);

        let mut seen = 0usize;
        parser
            .for_each_nal(|_nal| {
                seen += 1;
                Ok(false) // stop after first
            })
            .unwrap();
        assert_eq!(seen, 1);
    }

    // ── Annex B framing ───────────────────────────────────────────────────

    fn make_annex_b(nal_type: u8, layer_id: u8, temporal_id_plus1: u8, payload: &[u8]) -> Vec<u8> {
        let header = u16::to_be_bytes(
            (u16::from(nal_type) << 9) | (u16::from(layer_id) << 3) | (temporal_id_plus1 as u16)
        );
        let mut out = vec![0x00, 0x00, 0x00, 0x01];
        out.extend_from_slice(&header);
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn annex_b_single_nal() {
        let extent = make_annex_b(33, 0, 1, &[0xAB, 0xCD]);
        let extents: &[&[u8]] = &[&extent];
        let parser = NalParser::new_detect(extents);
        let nals = parser.collect().unwrap();
        assert_eq!(nals.len(), 1);
        assert_eq!(nals[0].nal_type, NalUnitType::SpsNut);
        assert_eq!(nals[0].payload, &[0xAB, 0xCD]);
    }

    #[test]
    fn annex_b_multiple_nals() {
        let mut extent = make_annex_b(32, 0, 1, &[]); // VPS
        extent.extend(make_annex_b(33, 0, 1, &[])); // SPS
        extent.extend(make_annex_b(34, 0, 1, &[])); // PPS
        let extents: &[&[u8]] = &[&extent];
        let parser = NalParser::new_detect(extents);
        let nals = parser.collect().unwrap();
        assert_eq!(nals.len(), 3);
        assert_eq!(nals[0].nal_type, NalUnitType::VpsNut);
        assert_eq!(nals[1].nal_type, NalUnitType::SpsNut);
        assert_eq!(nals[2].nal_type, NalUnitType::PpsNut);
    }

    #[test]
    fn annex_b_detect() {
        let data = vec![0x00, 0x00, 0x00, 0x01, 0x42];
        assert_eq!(NalFraming::detect(&data), NalFraming::AnnexB);
        let data = vec![0x00, 0x00, 0x00, 0x06, 0x42];
        assert_eq!(NalFraming::detect(&data), NalFraming::LengthPrefixed);
    }
}
