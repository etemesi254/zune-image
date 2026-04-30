use crate::channel::ChannelErrors;
use crate::image::{Image, MAX_CHANNELS};
use crate::traits::ZuneInts;
use bytemuck::Pod;

/// An iterator over the pixels of a [`zune_image::image::Image`].
///
/// This iterator converts planar image data (stored as separate channel arrays)
/// into an interleaved format (arrays of pixels) on the fly.
///
/// Each iteration returns a tuple: `(y,x,[T; MAX_CHANNELS])`.
pub struct PixelIterator<'a, T> {
    pixel: [T; MAX_CHANNELS],
    channels: [&'a [T]; MAX_CHANNELS],
    components: usize,
    width: usize,
    current_pos: usize,
    total_pixels: usize,
}

impl<'a, T> Iterator for PixelIterator<'a, T>
where
    T: Default + Copy,
{
    type Item = (usize, usize, [T; MAX_CHANNELS]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_pos >= self.total_pixels {
            return None;
        }
        let default = T::default();

        let i = self.current_pos;
        let y = i / self.width;
        let x = i % self.width;
        // this is the fastest option i could find, idk why it works that
        // better than all others
        for (channel_slice, pixel_channel) in self
            .channels
            .iter()
            .zip(self.pixel.iter_mut())
            .take(self.components)
        {
            *pixel_channel = *channel_slice.get(self.current_pos).unwrap_or(&default);
        }

        self.current_pos += 1;
        Some((y, x, self.pixel))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.total_pixels - self.current_pos;
        (remaining, Some(remaining))
    }
}
impl<'a, T> ExactSizeIterator for PixelIterator<'a, T> where T: Default + Copy {}

impl Image {
    /// Returns an iterator over the pixels of this image.
    ///
    /// This method exposes image data as an iterator of interleaved pixels,
    /// regardless of the internal planar storage format (separate channel buffers).
    ///
    /// Each item yielded by the iterator is a tuple:
    /// `(y, x, [T; MAX_CHANNELS])`, where:
    /// - `y` is the row index
    /// - `x` is the column index
    /// - the array contains pixel channel values in colorspace order
    ///
    /// # Type Parameters
    /// - `T`: The output sample type. This must match the underlying image
    ///   channel representation, or be safely reinterpret-able via [`bytemuck::Pod`].
    ///
    /// # Returns
    /// - `Ok(PixelIterator<T>)` if all channels can be safely reinterpreted as `T`
    /// - `Err(ChannelErrors)` if a channel cannot be cast to the requested type
    ///
    /// # Behavior
    /// - The iterator walks pixels in **row-major order** (top-to-bottom, left-to-right).
    /// - Channel data is converted from planar → interleaved on-the-fly.
    /// - If the image has fewer than `MAX_CHANNELS`, remaining entries are filled with `T::default()`.
    ///
    /// # Errors
    /// This function returns an error if:
    /// - The underlying channel buffers cannot be safely reinterpreted as `T`
    /// - There is a mismatch between the requested type and the stored pixel format
    ///
    /// # Performance
    /// - This iterator performs **on-the-fly planar → interleaved conversion**, which may
    ///   be slower than working directly with channel slices.
    /// - Each iteration touches multiple channel buffers, which can reduce cache locality.
    /// - For high-performance workloads (e.g. SIMD, bulk processing), prefer accessing
    ///   raw channel data via `channels_ref` instead of using this iterator.
    /// - For simple traversal or debugging, this iterator is ergonomic and efficient enough.
    ///
    /// A sample benchmark where we use `std::hint::black_box` to prevent compiler optimizations
    /// The benchmark sums all pixels in an image
    ///
    /// ``` text
    /// test iters::benchmarks::bench_channel_iteration  ... bench:   1,604,200.00 ns/iter
    /// test iters::benchmarks::bench_pixel_iterator     ... bench:   1,673,537.50 ns/iter [THIS]
    /// ```
    ///
    /// But removing the `black_box` in the inner loop leads to LLVM optimizing it, which changes the
    /// numbers to
    /// ```text
    /// test iters::benchmarks::bench_channel_iteration   ... bench:     159,231.26 ns/iter (+/- 623.69)
    /// test iters::benchmarks::bench_pixel_iterator      ... bench:   1,996,820.15 ns/iter (+/- 2,789.05)
    /// ```
    /// So where possible, prefer raw loops using `reintepret_as` and zipping iterators together
    /// # Examples
    ///
    /// ```
    /// use zune_image::image::Image;
    /// use zune_core::colorspace::ColorSpace;
    ///
    /// let img = Image::fill(255_u8, ColorSpace::RGB, 2, 2);
    ///
    /// for (y, x, pixel) in img.pixels::<u8>().unwrap() {
    ///     println!("({}, {}) = {:?}", y, x, &pixel[..3]);
    /// }
    /// ```
    ///
    /// # Notes
    /// - This iterator does not allocate per pixel; it reuses a fixed-size array.
    /// - For performance-sensitive code, prefer iterating once rather than collecting.
    pub fn pixels<T>(&self) -> Result<PixelIterator<'_, T>, ChannelErrors>
    where
        T: ZuneInts<T> + Default + Copy + 'static + Pod,
    {
        let colorspace = self.colorspace();
        let (width, height) = self.dimensions();

        let frame = &self.frames[0];
        let mut channels: [&[T]; MAX_CHANNELS] = [&[]; MAX_CHANNELS];

        for (pos, channel) in frame.channels_ref(colorspace, false).iter().enumerate() {
            channels[pos] = channel.reinterpret_as::<T>()?;
        }

        Ok(PixelIterator {
            channels,
            width,
            components: colorspace.num_components(),
            current_pos: 0,
            pixel: [T::default(); MAX_CHANNELS],
            total_pixels: width * height,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;
    use zune_core::colorspace::ColorSpace;

    #[test]
    fn test_pixel_iterator_bounds() {
        let width = 10;
        let height = 10;
        // Create a 10x10 RGB image (3 channels)
        let img = Image::fill(128_u8, ColorSpace::RGB, width, height);

        let iter = img.pixels::<u8>().expect("Failed to create iterator");

        // Ensure size hint matches dimensions
        assert_eq!(iter.len(), 100);

        let pixels: Vec<_> = iter.collect();
        assert_eq!(pixels.len(), 100);

        // Check the last pixel coordinates
        let (y, x, color) = pixels[99];
        assert_eq!(x, 9);
        assert_eq!(y, 9);
        assert_eq!(color[0], 128);
    }

    #[test]
    fn test_pixel_iterator_planar_to_interleaved() {
        let width = 2;
        let height = 1;

        // Manually set colors:
        // Pixel 0: [10, 20, 30]
        // Pixel 1: [40, 50, 60]
        let img = Image::from_fn(
            width,
            height,
            ColorSpace::RGB,
            |_y, x, pixels: &mut [u8; 4]| {
                if x == 0 {
                    pixels.copy_from_slice(&[10, 20, 30, 0])
                }
                if x == 1 {
                    pixels.copy_from_slice(&[40, 50, 60, 0])
                }
            },
        );

        let mut iter = img.pixels::<u8>().unwrap();

        // First pixel
        let (_, _, p1) = iter.next().unwrap();
        assert_eq!(p1[0], 10);
        assert_eq!(p1[1], 20);
        assert_eq!(p1[2], 30);

        // Second pixel
        let (_, _, p2) = iter.next().unwrap();
        assert_eq!(p2[0], 40);
        assert_eq!(p2[1], 50);
        assert_eq!(p2[2], 60);

        assert!(iter.next().is_none());
    }

    #[test]
    fn test_u16_iteration() {
        let img = Image::fill(5000_u16, ColorSpace::Luma, 5, 5);
        let mut iter = img.pixels::<u16>().unwrap();

        let (_, _, pix) = iter.next().unwrap();
        assert_eq!(pix[0], 5000);
    }
}

#[cfg(feature = "benchmarks")]
mod benchmarks {

    extern crate test;

    use crate::image::Image;
    use test::Bencher;
    use zune_core::colorspace::ColorSpace;

    fn make_image() -> Image {
        // Use a reasonably large image to expose cache effects
        Image::fill(128_u8, ColorSpace::RGB, 1920, 1080)
    }

    #[bench]
    fn bench_pixel_iterator(b: &mut Bencher) {
        let img = make_image();

        b.iter(|| {
            let mut sum = 0u64;

            for (_, _, px) in img.pixels::<u8>().unwrap() {
                sum += px[0] as u64;
                sum += px[1] as u64;
                sum += px[2] as u64;
            }

            test::black_box(sum);
        });
    }

    #[bench]
    fn bench_channel_iteration(b: &mut Bencher) {
        let img = make_image();

        b.iter(|| {
            let mut sum = 0u64;

            let channels = img.frames[0].channels_ref(img.colorspace(), false);

            let r = channels[0].reinterpret_as::<u8>().unwrap();
            let g = channels[1].reinterpret_as::<u8>().unwrap();
            let b_ch = channels[2].reinterpret_as::<u8>().unwrap();

            for ((&rv, &gv), &bv) in test::black_box(r)
                .iter()
                .zip(test::black_box(g).iter())
                .zip(test::black_box(b_ch).iter())
            {
                sum += rv as u64;
                sum += gv as u64;
                sum += bv as u64;
                // prevent simd from auto optimizing this
                test::black_box(sum);
            }

            test::black_box(sum);
        });
    }
}
