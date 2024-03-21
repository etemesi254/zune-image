//! Calculate channel histogram statistics
//!
//! An image histogram is a graph that shows the number of pixels in an image at each intensity value
//!
//! ## Supported depths
//! - [BitDepth::Eight](zune_core::bit_depth::BitDepth::Eight), [BitDepth::Sixteen](zune_core::bit_depth::BitDepth::Sixteen)
//!
//! [BitDepth::Float32](zune_core::bit_depth::BitDepth::Float32) is unsupported due to the ability of it storing
//! way too many colors to properly histogram
//!
//!
use std::cell::{BorrowError, Ref, RefCell};
use std::usize;

use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;
/// A channel histogram instance
///
/// Histogram statistics can be fetched via  `.histogram()`  after calling `execute`
///
/// The return type is a vector of vectors, with the interpretation of each vector depending on the colorspace
/// E.g if image is in RGBA, the vector would be of len 4, each the first innermost vector would give you
/// `R` channel histogram details, the last giving you "A" histogram details
///
/// This struct does not mutate the image in any way, but it needs to conform to the trait
/// definition of `OperationsTrait` hence why it needs a mutable image
///
/// # Example
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::histogram::ChannelHistogram;
/// let mut image = Image::fill(100_u8,ColorSpace::RGB,100,100);
/// let histogram = ChannelHistogram::new();
/// histogram.execute(&mut image).unwrap();
///let values =  histogram.histogram().unwrap();
/// // r had 100 items
/// assert_eq!(values[0][100], 100_u32*100);
/// assert_eq!(values[1][100], 100_u32*100);
/// ```
#[derive(Default)]
pub struct ChannelHistogram {
    histogram: RefCell<Vec<Vec<u32>>>
}

impl ChannelHistogram {
    /// Create a new channel histogram
    #[must_use]
    pub fn new() -> ChannelHistogram {
        ChannelHistogram::default()
    }
    /// Returns the histogram after a single pass on an image
    ///
    /// This will contain histogram details of each channel,
    ///
    /// # Returns
    /// - Ok(reference): A reference to the underlying result
    /// - Err(BorrowError): Indicates this filter has borrowed the reference
    pub fn histogram(&self) -> Result<Ref<'_, Vec<Vec<u32>>>, BorrowError> {
        self.histogram.try_borrow()
    }
}
impl OperationsTrait for ChannelHistogram {
    fn name(&self) -> &'static str {
        "Channel Histogram"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth().bit_type();

        self.histogram.borrow_mut().clear();

        match depth {
            BitType::U8 => {
                for channel in image.channels_ref(false) {
                    let pixels = channel.reinterpret_as::<u8>()?;

                    let histo = histogram(pixels);
                    self.histogram.borrow_mut().push(histo.to_vec());
                }
            }
            BitType::U16 => {
                for channel in image.channels_ref(false) {
                    let pixels = channel.reinterpret_as::<u16>()?;

                    let histo = histogram_u16(pixels);
                    self.histogram.borrow_mut().push(histo.clone());
                }
            }
            _ => {
                return Err(ImageErrors::GenericStr(
                    "Histogram isn't implemented for f32 images"
                ))
            }
        }
        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}

#[must_use]
pub fn histogram(data: &[u8]) -> [u32; 256] {
    // Histogram calculation
    //
    //
    // From https://fastcompression.blogspot.com/2014/09/counting-bytes-fast-little-trick-from.html

    // contains our count values
    let mut start1 = [0; 256];
    // allocate 4x size
    let mut counts = [0_u32; 256 * 3];
    // break into  4.
    let (start2, counts) = counts.split_at_mut(256);
    let (start3, start4) = counts.split_at_mut(256);
    let chunks = data.chunks_exact(8);
    let remainder = chunks.remainder();

    for i in chunks {
        // count as fast as possible
        // This is the fastest platform independent histogram function I could find.
        //
        // Probably attributed to powturbo and Nathan Kurtz but it's also in
        // FSE/lib/hist.c

        let tmp1 = u64::from_le_bytes(i[0..8].try_into().unwrap());

        start1[((tmp1 >> 56) & 255) as usize] += 1;
        start2[((tmp1 >> 48) & 255) as usize] += 1;
        start3[((tmp1 >> 40) & 255) as usize] += 1;
        start4[((tmp1 >> 32) & 255) as usize] += 1;
        start1[((tmp1 >> 24) & 255) as usize] += 1;
        start2[((tmp1 >> 16) & 255) as usize] += 1;
        start3[((tmp1 >> 8) & 255) as usize] += 1;

        start4[(tmp1 & 255) as usize] += 1;
    }

    for i in remainder {
        start1[usize::from(*i)] += 1;
    }
    // add them together
    for (((b, c), d), e) in start1
        .iter_mut()
        .zip(start2.iter())
        .zip(start3.iter())
        .zip(start4.iter())
    {
        *b += c + d + e;
    }

    start1
}

fn histogram_u16(data: &[u16]) -> Vec<u32> {
    let mut size = vec![0; usize::from(u16::MAX) + 1];
    let size_arr: &mut [u32; { u16::MAX as usize } + 1] =
        size.get_mut(..).unwrap().try_into().unwrap();

    let chunks = data.chunks_exact(4);
    let remainder = chunks.remainder();

    // we don't apply the histogram optimization for u16 as that uses a lot of memory
    // so let's do simple unrolling
    for i in chunks {
        size_arr[usize::from(i[0])] += 1;
        size_arr[usize::from(i[1])] += 1;
        size_arr[usize::from(i[2])] += 1;
        size_arr[usize::from(i[3])] += 1;
    }
    // remainder
    for i in remainder {
        size_arr[usize::from(*i)] += 1;
    }
    size
}

#[test]
fn test_histogram_u8() {
    use nanorand::Rng;
    use zune_core::colorspace::ColorSpace;

    let (w, h) = (400, 400);

    // randomize inputs
    let mut pixels = vec![0_u8; w * h];
    nanorand::WyRand::new().fill(&mut pixels);

    let mut image = Image::from_u8(&pixels, w, h, ColorSpace::Luma);

    let histo = ChannelHistogram::new();

    histo.execute_impl(&mut image).unwrap();
    let data = histo.histogram().expect("Reference is borrowed");
    assert_eq!(data.len(), 1);
    assert_eq!(
        data[0].iter().sum::<u32>(),
        u32::try_from(pixels.len()).unwrap_or(0)
    );
}

#[test]
fn test_histogram_u16() {
    use nanorand::Rng;
    use zune_core::colorspace::ColorSpace;

    let (w, h) = (400, 400);

    // randomize inputs
    let mut pixels = vec![0_u16; w * h];
    nanorand::WyRand::new().fill(&mut pixels);

    let mut image = Image::from_u16(&pixels, w, h, ColorSpace::Luma);

    let histo = ChannelHistogram::new();

    histo.execute_impl(&mut image).unwrap();
    let data = histo.histogram().expect("Reference is borrowed");
    // ensure everything was summed
    assert_eq!(
        data[0].iter().sum::<u32>(),
        u32::try_from(pixels.len()).unwrap_or(0)
    );
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use nanorand::Rng;

    use crate::histogram::{histogram, histogram_u16};
    #[bench]
    fn bench_histogram_u8(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut in_vec = vec![255_u8; dimensions];
        nanorand::WyRand::new().fill(&mut in_vec);

        b.iter(|| histogram(&in_vec));
    }
    #[bench]
    fn bench_histogram_u16(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut in_vec = vec![0_u16; dimensions];
        nanorand::WyRand::new().fill(&mut in_vec);

        b.iter(|| histogram_u16(&in_vec));
    }
}
