use log::trace;
use zune_imageprocs::convolve::convolve_1d;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

/// Rearrange the pixels up side down
#[derive(Default)]
pub struct Convolve
{
    weights: Vec<f64>,
}

impl Convolve
{
    pub fn new(weights: Vec<f64>) -> Convolve
    {
        Convolve { weights }
    }
}

impl OperationsTrait for Convolve
{
    fn get_name(&self) -> &'static str
    {
        "1D convolution"
    }

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();

        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(input) =>
            {
                let mut out_channel = vec![0; input.len()];

                convolve_1d(
                    input,
                    &mut out_channel,
                    width,
                    height,
                    &self.weights,
                    self.weights.len() as f64,
                );

                *input = out_channel;
            }
            ImageChannels::TwoChannels(input) =>
            {
                for inp in input.iter_mut().take(1)
                {
                    let mut out_channel = vec![0; inp.len()];

                    convolve_1d(
                        inp,
                        &mut out_channel,
                        width,
                        height,
                        &self.weights,
                        self.weights.len() as f64,
                    );

                    *inp = out_channel;
                }
            }
            ImageChannels::ThreeChannels(input) =>
            {
                #[cfg(not(feature = "threads"))]
                {
                    trace!("Running 1D convolution correction in single threaded mode");

                    for inp in input
                    {
                        let mut out_channel = vec![0; inp.len()];

                        convolve_1d(
                            inp,
                            &mut out_channel,
                            width,
                            height,
                            &self.weights,
                            self.weights.len() as f64,
                        );
                        *inp = out_channel;
                    }
                }
                #[cfg(feature = "threads")]
                {
                    trace!("Running 1D convolution correction in multithreaded mode");
                    std::thread::scope(|s| {
                        // blur each channel on a separate thread
                        for inp in input
                        {
                            let inp_len = inp.len();
                            s.spawn(move || {
                                let mut out_channel = vec![0; inp_len];
                                convolve_1d(
                                    inp,
                                    &mut out_channel,
                                    width,
                                    height,
                                    &self.weights,
                                    self.weights.len() as f64,
                                );

                                *inp = out_channel;
                            });
                        }
                    });
                }
            }
            ImageChannels::FourChannels(input) =>
            {
                #[cfg(not(feature = "threads"))]
                {
                    trace!("Running 1D  convolution correction in single threaded mode");

                    for inp in input
                    {
                        let mut out_channel = vec![0; inp.len()];

                        convolve_1d(
                            inp,
                            &mut out_channel,
                            width,
                            height,
                            &self.weights,
                            self.weights.len() as f64,
                        );
                        *inp = out_channel;
                    }
                }
                #[cfg(feature = "threads")]
                {
                    trace!("Running 1D convolution correction in multithreaded mode");

                    std::thread::scope(|s| {
                        // blur each channel on a separate thread
                        for inp in input.iter_mut().take(3)
                        {
                            let input_len = inp.len();

                            s.spawn(move || {
                                let mut out_channel = vec![0; input_len];
                                convolve_1d(
                                    inp,
                                    &mut out_channel,
                                    width,
                                    height,
                                    &self.weights,
                                    self.weights.len() as f64,
                                );
                                *inp = out_channel;
                            });
                        }
                    });
                }
            }
            ImageChannels::Interleaved(_) =>
            {
                return Err(ImgOperationsErrors::Generic(
                    "Cannot convolve interleaved pixels",
                ))
            }
            ImageChannels::Uninitialized =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot convolve pixels",
                ))
            }
        }
        Ok(())
    }
}
