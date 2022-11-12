use crate::bitstream::BitStreamReader;
use crate::constants::{
    DEFLATE_MAX_CODEWORD_LENGTH, DEFLATE_NUM_PRECODE_SYMS, DEFLATE_PRECODE_LENS_PERMUTATION,
    PRECODE_ENOUGH,
};
use crate::enums::DeflateState;
use crate::errors::ZlibDecodeErrors;
use crate::errors::ZlibDecodeErrors::InsufficientData;

struct ZlibDecoder<'a>
{
    data:                &'a [u8],
    position:            usize,
    state:               DeflateState,
    stream:              BitStreamReader<'a>,
    is_last_block:       bool,
    static_codes_loaded: bool,
}
impl<'a> ZlibDecoder<'a>
{
    fn new_zlib(data: &'a [u8]) -> ZlibDecoder<'a>
    {
        // create stream

        ZlibDecoder {
            data,
            position: 0,
            state: DeflateState::Initialized,
            stream: BitStreamReader::new(data),
            is_last_block: false,
            static_codes_loaded: false,
        }
    }
    pub fn decode_zlib(&mut self) -> Result<(), ZlibDecodeErrors>
    {
        if self.data.len()
            < 2 /* zlib header */
            + 4
        /* Deflate */
        {
            return Err(InsufficientData);
        }

        // Zlib flags
        // See https://www.ietf.org/rfc/rfc1950.txt for
        // the RFC
        let cmf = self.data[0];
        let flg = self.data[1];

        let cm = cmf & 0xF;
        let cinfo = cmf >> 4;

        // let fcheck = flg & 0xF;
        // let fdict = (flg >> 4) & 1;
        // let flevel = flg >> 5;

        // confirm we have the right deflate methods
        if cm != 8
        {
            if cm == 15
            {
                return Err(ZlibDecodeErrors::Generic(
                    "CM of 15 is preserved by the standard,currently don't know how to handle it",
                ));
            }
            return Err(ZlibDecodeErrors::GenericStr(format!(
                "Unknown zlib compression method {}",
                cm
            )));
        }
        if cinfo > 7
        {
            return Err(ZlibDecodeErrors::GenericStr(format!(
                "Unknown cinfo `{}` greater than 7, not allowed",
                cinfo
            )));
        }
        let flag_checks = (u16::from(cmf) * 256) + u16::from(flg);

        if flag_checks % 31 != 0
        {
            return Err(ZlibDecodeErrors::Generic("FCHECK integrity not preserved"));
        }

        self.position = 2;

        Ok(())
    }
    ///Decode a deflate stream
    pub fn decode_deflate(&mut self) -> Result<(), ZlibDecodeErrors>
    {
        match self.state
        {
            DeflateState::Initialized => self.start_deflate_block()?,

            _ => todo!(),
        }
        Ok(())
    }
    fn start_deflate_block(&mut self) -> Result<(), ZlibDecodeErrors>
    {
        let mut precode_lens = [0; DEFLATE_NUM_PRECODE_SYMS];
        let mut precode_decode_table = [0_u32; PRECODE_ENOUGH];
        // start deflate decode
        // re-read the stream so that we can remove code read by zlib
        self.stream = BitStreamReader::new(&self.data[self.position..]);

        self.stream.refill();

        // bits used
        self.is_last_block = self.stream.get_bits(1) == 1;
        let block_type = self.stream.get_bits(2);

        if block_type == 2
        {
            const SINGLE_PRECODE: usize = 3;

            // Dynamic Huffman block

            // Read codeword lengths
            let num_litlen_syms = 257 + (self.stream.get_bits(5)) as usize;
            let num_offset_syms = 1 + (self.stream.get_bits(5)) as usize;
            let num_explicit_precode_lens = 4 + (self.stream.get_bits(4)) as usize;

            self.static_codes_loaded = false;

            precode_lens[usize::from(DEFLATE_PRECODE_LENS_PERMUTATION[0])] =
                self.stream.get_bits(3) as u8;
            self.stream.refill();

            let expected = (SINGLE_PRECODE * num_explicit_precode_lens) as u8;

            if !self.stream.has(expected)
            {
                return Err(ZlibDecodeErrors::InsufficientData);
            }

            for i in DEFLATE_PRECODE_LENS_PERMUTATION
                .iter()
                .take(num_explicit_precode_lens)
            {
                let bits = self.stream.get_bits(3) as u8;

                precode_lens[usize::from(*i)] = bits;
            }

            self.build_decode_table(&precode_lens);
        }

        Ok(())
    }
    /// Build the decode table for the precode
    fn build_decode_table(&mut self, precode_lens: &[u8; DEFLATE_NUM_PRECODE_SYMS])
    {
        let len_counts: [u32; DEFLATE_MAX_CODEWORD_LENGTH + 1] =
            [0; DEFLATE_MAX_CODEWORD_LENGTH + 1];
        let offsets: [u32; DEFLATE_MAX_CODEWORD_LENGTH + 1] = [0; DEFLATE_MAX_CODEWORD_LENGTH + 1];

        todo!()
    }
}
