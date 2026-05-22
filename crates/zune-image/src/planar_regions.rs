use crate::channel::ChannelErrors;
use crate::frame::Frame;
use crate::image::Image;
use crate::traits::ZuneInts;
use bytemuck::Pod;
use rayon::prelude::*;
use zune_core::colorspace::ColorSpace;

/// Represents a mutable chunk of an image across all its planar channels.
pub struct PlanarRegionMut<'a, T> {
    /// The start from the top region for which this region is in
    pub y_offset: usize,
    /// The height of this region
    pub height: usize,
    /// The full width of this region, usually the full width of the image
    pub width: usize,
    /// The specific mutable region this thread is responsible for reading and or writing
    pub channels: &'a mut [&'a mut [T]],
}

impl<'a, T> PlanarRegionMut<'a, T> {
    pub fn process_region<F>(&mut self, mut op: F)
    where
        F: FnMut(&mut PlanarRegionMut<'_, T>),
    {
        op(self);
    }
}

// A tiny wrapper to allow passing raw pointers across Rayon threads.
#[derive(Copy, Clone)]
struct ThreadSafePtr<T>(*mut T);
unsafe impl<T> Send for ThreadSafePtr<T> {}
unsafe impl<T> Sync for ThreadSafePtr<T> {}

// A reasonable upper bound for stack-allocating channel pointers.
const MAX_SUPPORTED_CHANNELS: usize = 16;
// number of available threads + number of available regions
const REGION_DISTRIBUTION_SPLIT: usize = 4;

impl Image {
    /// Processes a single frame in parallel.
    pub fn par_process_frame_regions<T, F>(
        frame: &mut Frame, width: usize, height: usize, colorspace: ColorSpace, ignore_alpha: bool,
        op: F,
    ) -> Result<(), ChannelErrors>
    where
        T: ZuneInts<T> + Default + Copy + 'static + Pod + Send + Sync,
        F: Fn(&mut PlanarRegionMut<'_, T>) + Send + Sync,
    {
        let num_threads = rayon::current_num_threads();
        let target_regions = num_threads * REGION_DISTRIBUTION_SPLIT;
        let lines_per_region = height.div_ceil(target_regions).max(1);
        let num_regions = height.div_ceil(lines_per_region);

        let channel_slice = frame.channels_mut(colorspace, ignore_alpha);
        let num_channels = channel_slice.len();

        assert!(
            num_channels <= MAX_SUPPORTED_CHANNELS,
            "Image has {} channels, which exceeds the stack buffer limit of {}",
            num_channels,
            MAX_SUPPORTED_CHANNELS
        );

        let mut ptrs = [ThreadSafePtr(std::ptr::null_mut()); MAX_SUPPORTED_CHANNELS];
        let mut lengths = [0usize; MAX_SUPPORTED_CHANNELS];

        for (i, channel) in channel_slice.iter_mut().enumerate() {
            let slice = channel.reinterpret_as_mut::<T>()?;
            ptrs[i] = ThreadSafePtr(slice.as_mut_ptr());
            lengths[i] = slice.len();
        }

        (0..num_regions).into_par_iter().for_each(|i| {
            let y_offset = i * lines_per_region;
            let current_height = std::cmp::min(lines_per_region, height - y_offset);

            let start_idx = y_offset * width;
            let end_idx = start_idx + (current_height * width);

            let mut current_channels: [&mut [T]; MAX_SUPPORTED_CHANNELS] =
                std::array::from_fn(|_| &mut [] as &mut [T]);

            for c in 0..num_channels {
                let safe_end = std::cmp::min(end_idx, lengths[c]);
                let slice_len = safe_end.saturating_sub(start_idx);
                // safety, `i` will always be unique per chunk

                unsafe {
                    current_channels[c] =
                        std::slice::from_raw_parts_mut(ptrs[c].0.add(start_idx), slice_len);
                }
            }

            let mut region = PlanarRegionMut {
                y_offset,
                height: current_height,
                width,
                channels: &mut current_channels[..num_channels],
            };

            op(&mut region);
        });

        Ok(())
    }

    pub fn par_process_regions<T, F>(
        &mut self, ignore_alpha: bool, op: F,
    ) -> Result<(), ChannelErrors>
    where
        T: ZuneInts<T> + Default + Copy + 'static + Pod + Send + Sync,
        F: Fn(&mut PlanarRegionMut<'_, T>) + Send + Sync,
    {
        let width = self.width();
        let height = self.height();
        let colorspace = self.colorspace();

        for frame in self.frames.iter_mut() {
            Self::par_process_frame_regions(frame, width, height, colorspace, ignore_alpha, &op)?;
        }
        Ok(())
    }
}

pub struct PlanarRegionOut<'a, T> {
    /// The start from the top region for which this region is in
    pub y_offset: usize,
    /// Height of the sliced channel
    pub height: usize,
    /// Width of this full image
    pub width: usize,
    /// The entire immutable source image channels (Thread-safe to read from anywhere)
    pub src_channels: &'a [&'a [T]],
    /// The specific mutable region this thread is responsible for writing to
    pub dest_channels: &'a mut [&'a mut [T]],
}

impl Image {
    /// Processes regions out-of-place.
    /// Great for area operations like blurs, convolutions, and bilateral filters.
    pub fn par_process_regions_out_of_place<T, F>(
        &self,
        dest: &mut Image, // The pre-allocated output image
        ignore_alpha: bool,
        op: F,
    ) -> Result<(), ChannelErrors>
    where
        T: ZuneInts<T> + Default + Copy + 'static + Pod + Send + Sync,
        F: Fn(&mut PlanarRegionOut<'_, T>) + Send + Sync,
    {
        let colorspace = self.colorspace();
        let width = self.width();
        let height = self.height();

        let num_threads = rayon::current_num_threads();
        let target_regions = num_threads * REGION_DISTRIBUTION_SPLIT;
        let lines_per_region = height.div_ceil(target_regions).max(1);
        let num_regions = height.div_ceil(lines_per_region);

        for (src_frame, dest_frame) in self.frames.iter().zip(dest.frames.iter_mut()) {
            let src_slice = src_frame.channels_ref(colorspace, ignore_alpha);
            let dest_slice = dest_frame.channels_mut(colorspace, ignore_alpha);
            let num_channels = src_slice.len();

            // Extract immutable raw pointers for the source (so threads can read anywhere)
            let mut src_ptrs: [Option<&[T]>; MAX_SUPPORTED_CHANNELS] =
                [None; MAX_SUPPORTED_CHANNELS];
            let mut lengths = [0usize; MAX_SUPPORTED_CHANNELS];

            for (i, channel) in src_slice.iter().enumerate() {
                let slice = channel.reinterpret_as::<T>()?;
                src_ptrs[i] = Some(slice);
            }

            // Extract mutable thread-safe pointers for the destination
            let mut dest_ptrs = [ThreadSafePtr(std::ptr::null_mut()); MAX_SUPPORTED_CHANNELS];

            for (i, channel) in dest_slice.iter_mut().enumerate() {
                let slice = channel.reinterpret_as_mut::<T>()?;
                dest_ptrs[i] = ThreadSafePtr(slice.as_mut_ptr());
                lengths[i] = slice.len();
            }

            (0..num_regions).into_par_iter().for_each(|i| {
                let y_offset = i * lines_per_region;
                let current_height = std::cmp::min(lines_per_region, height - y_offset);

                let start_idx = y_offset * width;
                let end_idx = start_idx + (current_height * width);

                let mut current_src: [&[T]; MAX_SUPPORTED_CHANNELS] =
                    std::array::from_fn(|_| &[] as &[T]);

                let mut current_dest: [&mut [T]; MAX_SUPPORTED_CHANNELS] =
                    std::array::from_fn(|_| &mut [] as &mut [T]);

                for c in 0..num_channels {
                    let safe_end = std::cmp::min(end_idx, lengths[c]);
                    let slice_len = safe_end.saturating_sub(start_idx);

                    unsafe {
                        current_src[c] =
                            if let Some(src_slice) = src_ptrs[c] { src_slice } else { &[] };
                        // Dest is strictly chunked
                        current_dest[c] = std::slice::from_raw_parts_mut(
                            dest_ptrs[c].0.add(start_idx),
                            slice_len,
                        );
                    }
                }

                let mut region = PlanarRegionOut {
                    y_offset,
                    height: current_height,
                    width,
                    src_channels: &current_src[..num_channels],
                    dest_channels: &mut current_dest[..num_channels],
                };

                op(&mut region);
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_planar_regions() {
        let mut img = Image::fill(128_u8, ColorSpace::RGB, 1000, 1000);
        img.par_process_regions::<u8, _>(true, |x| {
            println!("{:?},{:?}", x.height, x.y_offset);
        })
        .unwrap();
    }
}
