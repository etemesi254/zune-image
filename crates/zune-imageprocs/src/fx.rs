#![cfg(feature = "exmex")]
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
use exmex::prelude::*;
use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

/// Apply a custom mathematical expression to every pixel in an image.
///
/// The `Fx` operator uses a JIT-compiled expression evaluator to perform
/// per-pixel calculations. It is highly optimized for planar image data
/// and supports multi-threaded execution.
///
/// This uses the [exmex] library for parsing and evaluation
///
/// # Variables
///
/// The following variables are available within the expression:
///
/// | Variable | Description |
/// | :--- | :--- |
/// | `val` | The value of the current channel being processed. |
/// | `r, g, b, a` , `h, s, l` | Specific channel values (mapped dynamically to the image colorspace). This, is dynamically mapped based on channel, so for RGB image, it will be r,g,b, for hsl it will be h,s,l |
/// | `i, j` | Current pixel coordinates (x, y). |
/// | `w, h` | Image width and height. |
/// | `rand` | A random float between `0.0` and `1.0`, unique per pixel. |
/// | `img_min` | Global minimum normalized value in the image. |
/// | `img_max` | Global maximum normalized value in the image. |
/// | `img_mean`| Global average value of all pixels. |
///
///
/// # Channel mapping
///
/// COLORSPACE VARIABLES (Dynamically mapped based on active Colorspace):
///
/// - r, g, b, a   : RGB, RGBA, BGR, BGRA, ARGB
/// -  h, s, l      : HSL
/// -  h, s, v      : HSV
/// -  c, m, y, k   : CMYK
/// -  y, cb, cr    : YCbCr, YCCK
/// -  luma (or y)  : Luma / Grayscale
///
/// GENERIC CHANNELS:
///  - c0, c1... cN : Target an exact channel index instead of using color channels
///
/// # Channel Targeting
///
/// You can target a specific channel by prefixing the expression with the channel name
/// and a colon. If no prefix is provided, the expression is applied to all color channels.
///
/// * `r: r * 1.5` - Only affects the Red channel.
/// * `val * 0.5` - Darkens all color channels.
///
/// # Examples
///
/// ### Basic Adjustments
/// ```rust
/// use zune_imageprocs::fx::Fx;
/// // Increase brightness of all channels by 20%
/// let fx = Fx::new("val * 1.2");
///
/// // Boost red channel only
/// let fx_red = Fx::new("r: r * 1.5");
/// ```
///
/// ### Procedural Gradients
/// You can create a linear horizontal gradient by using the `i` (x-coordinate)
/// and `w` (width) variables.
/// ```rust
/// use zune_imageprocs::fx::Fx;
/// // Create a horizontal fade from black to original color
/// let fx = Fx::new("val * (i / w)");
/// ```
///
///
/// ### Artistic Effects
/// ```rust
/// use zune_imageprocs::fx::Fx;
/// // CRT Scanlines: Multiply by a sine wave based on vertical position
/// let scanlines = Fx::new("val * (0.8 + 0.2 * sin(j * 1.5))");
///
/// // Film Grain: Mix 90% of the image with 10% random noise
/// let grain = Fx::new("(val * 0.9) + (rand * 0.1)");
/// ```
///
/// ### Color Balancing
/// Stretch the contrast of the image to the full 0.0-1.0 range.
/// ```rust
/// use zune_imageprocs::fx::Fx;
/// let contrast = Fx::new("(val - img_min) / (img_max - img_min)");
/// ```
///
/// [exmex]: https://crates.io/crates/exmex
pub struct Fx {
    expression: String,
}

impl Fx {
    /// Create a new Fx operation with the given mathematical expression.
    ///
    /// # Panics
    /// The constructor does not panic, but the `execute` method will return an error
    /// if the expression string is mathematically invalid or contains unknown variables.
    pub fn new<S: Into<String>>(expression: S) -> Self {
        Self {
            expression: expression.into(),
        }
    }
}

impl OperationsTrait for Fx {
    fn name(&self) -> &'static str {
        "FX Expression"
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }

    #[allow(clippy::too_many_lines)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let colorspace = image.colorspace();

        // 1. DYNAMIC TARGET PARSING: Check if the user specified a target channel (e.g., "r: r * 1.3")
        let (target_channel, expr_str) =
            if let Some((prefix, rest)) = self.expression.split_once(':') {
                let ch_idx = get_channel_index(prefix.trim(), colorspace).ok_or_else(|| {
                    ImageErrors::GenericString(format!("Unknown target channel '{prefix}'"))
                })?;
                (Some(ch_idx), rest.trim())
            } else {
                (None, self.expression.as_str())
            };

        // 2. Parse the expression using the cleaned string (without the prefix)
        let expr = exmex::parse::<f64>(expr_str)
            .map_err(|e| ImageErrors::GenericString(format!("FX Parse Error: {e}")))?;

        let expected_vars = expr.var_names();
        let num_vars = expected_vars.len();

        let mut w_idx = None;
        let mut h_idx = None;
        let mut i_idx = None;
        let mut j_idx = None;
        let mut max_idx = None;
        let mut min_idx = None;
        let mut pi_idx = None;
        let mut e_idx = None;
        let mut val_idx = None;
        let mut rand_idx = None;
        let mut img_min_idx = None;
        let mut img_max_idx = None;
        let mut img_mean_idx = None;

        let mut channel_mappings: Vec<(usize, usize)> = Vec::new();

        for (idx, name) in expected_vars.iter().enumerate() {
            match name.as_str() {
                "val" | "u" => val_idx = Some(idx),
                "w" => w_idx = Some(idx),
                "h" => h_idx = Some(idx),
                "i" => i_idx = Some(idx),
                "j" => j_idx = Some(idx),
                "MAX" => max_idx = Some(idx),
                "MIN" => min_idx = Some(idx),
                "PI" => pi_idx = Some(idx),
                "rand" => rand_idx = Some(idx),
                "img_min" => img_min_idx = Some(idx),
                "img_max" => img_max_idx = Some(idx),
                "img_mean" => img_mean_idx = Some(idx),

                "E" => e_idx = Some(idx),
                other => {
                    if let Some(ch_idx) = get_channel_index(other, colorspace) {
                        channel_mappings.push((ch_idx, idx));
                    } else {
                        return Err(ImageErrors::GenericString(format!(
                            "Unknown variable '{other}' for colorspace {colorspace:?}",
                        )));
                    }
                }
            }
        }

        let (width, height) = image.dimensions();
        let depth = image.depth().bit_type();

        let max_val = match depth {
            BitType::U8 => 255.0,
            BitType::U16 => 65535.0,
            BitType::F32 => 1.0,
            _ => return Err(ImageErrors::GenericStr("Unsupported bit type for FX")),
        };

        let mut args = vec![0.0; num_vars];
        if let Some(idx) = w_idx {
            args[idx] = width as f64;
        }
        if let Some(idx) = h_idx {
            args[idx] = height as f64;
        }
        if let Some(idx) = max_idx {
            args[idx] = max_val;
        }
        if let Some(idx) = min_idx {
            args[idx] = 0.0;
        }
        if let Some(idx) = pi_idx {
            args[idx] = std::f64::consts::PI;
        }
        if let Some(idx) = e_idx {
            args[idx] = std::f64::consts::E;
        }
        let num_threads = if cfg!(feature = "threads") { 4 } else { 1 };

        for frame in image.frames_mut() {
            let channels = frame.channels_mut(colorspace, false);
            let num_channels = channels.len();

            macro_rules! eval_loop {
                ($type:ty, $is_float:expr) => {{

                    // --- CONDITIONAL PRE-PASS STATISTICS ---
                    let mut overall_min = 0.0f64;
                    let mut overall_max = 1.0f64;
                    let mut overall_mean = 0.0f64;

                    // ONLY scan the image if the user actually typed these variables!
                    if img_min_idx.is_some() || img_max_idx.is_some() || img_mean_idx.is_some() {
                        overall_min = 1.0;
                        overall_max = 0.0;
                        let mut overall_sum = 0.0f64;
                        let total_pixels = (width * height * num_channels) as f64;

                        for ch in channels.iter() {
                            let slice = ch.reinterpret_as::<$type>().unwrap();
                            for &val in slice.iter() {
                                let v = f64::from(val) / max_val;
                                if v < overall_min { overall_min = v; }
                                if v > overall_max { overall_max = v; }
                                overall_sum += v;
                            }
                        }
                        overall_mean = overall_sum / total_pixels;

                    }

                    let mut args_template = args.clone();
                    // Inject stats into the template
                    if let Some(idx) = img_min_idx { args_template[idx] = overall_min; }
                    if let Some(idx) = img_max_idx { args_template[idx] = overall_max; }
                    if let Some(idx) = img_mean_idx { args_template[idx] = overall_mean; }

                    // 1. Get safe mutable slices for each channel
                    let channel_slices: Vec<&mut [$type]> = channels.iter_mut()
                        .map(|c| c.reinterpret_as_mut::<$type>().unwrap())
                        .collect();


                    let chunk_height = height.div_ceil(num_threads);
                    let pixels_per_chunk = chunk_height * width;

                    // 3. SAFE SLICING: We split every channel array into `num_threads` disjoint pieces.
                    // This creates a list of 'bands' (one for each thread) containing the specific rows it owns.
                    let mut thread_bands: Vec<Vec<&mut [$type]>> = (0..num_threads)
                        .map(|_| Vec::with_capacity(num_channels))
                        .collect();

                    for ch_slice in channel_slices {
                        let mut current_slice = ch_slice;
                        for t in 0..num_threads {
                            let take_len = std::cmp::min(pixels_per_chunk, current_slice.len());
                            // split_at_mut proves to the compiler that the two halves don't overlap!
                            let (band, rest) = current_slice.split_at_mut(take_len);
                            thread_bands[t].push(band);
                            current_slice = rest;
                        }
                    }

                    // 4. Define the core processing logic as a closure
                    let process_chunk = |start_y: usize, end_y: usize, mut local_args: Vec<f64>, mut slices: Vec<&mut [$type]>| {
                        let mut seed = 0x123456789ABCDEF ^ (start_y as u64);
                        for y in start_y..end_y {
                            if let Some(idx) = j_idx { local_args[idx] = y as f64; }

                            // Because this thread only holds a slice of the full image,
                            // its internal row index starts at 0, not at start_y!
                            let local_y = y - start_y;

                            for x in 0..width {
                                let px_idx = local_y * width + x;
                                if let Some(idx) = i_idx { local_args[idx] = x as f64; }

                                // --- NEW: GENERATE RANDOM NOISE ---
                                if let Some(idx) = rand_idx {
                                    // Ultra-fast LCG implementation
                                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                                    // Extract top 53 bits for a perfect 0.0 to 1.0 float
                                    local_args[idx] = (seed >> 11) as f64 / (1u64 << 53) as f64;
                                }

                                // Read targeted channels
                                for &(ch_idx, arg_idx) in &channel_mappings {
                                    if ch_idx < num_channels {
                                        local_args[arg_idx] = f64::from(slices[ch_idx][px_idx]) / max_val;
                                    }
                                }

                                if let Some(tc) = target_channel {
                                    // TARGETED CHANNEL WRITE
                                    let result = expr.eval(&local_args).unwrap_or(0.0);
                                    let out_val = if $is_float {
                                        (result.clamp(0.0, 1.0) * max_val) as $type
                                    } else {
                                        (result.clamp(0.0, 1.0) * max_val).round() as $type
                                    };

                                    if tc < num_channels {
                                        slices[tc][px_idx] = out_val;
                                    }
                                } else {
                                    // GLOBAL PER-CHANNEL WRITE
                                    let target_channels_count = if colorspace.has_alpha() { num_channels - 1 } else { num_channels };

                                    for c in 0..target_channels_count {
                                        if let Some(idx) = val_idx {
                                            local_args[idx] = f64::from(slices[c][px_idx]) / max_val;
                                        }

                                        let result = expr.eval(&local_args).unwrap_or(0.0);
                                        let out_val = if $is_float {
                                            (result.clamp(0.0, 1.0) * max_val) as $type
                                        } else {
                                            (result.clamp(0.0, 1.0) * max_val).round() as $type
                                        };

                                        slices[c][px_idx] = out_val;
                                    }
                                }
                            }
                        }
                    };

                    // 5. Execute
                    if num_threads <= 1 {
                        // WASM / Single-core execution
                        let single_band = thread_bands.remove(0);
                        process_chunk(0, height, args_template, single_band);
                    } else {
                        // Multi-core execution
                        std::thread::scope(|s| {
                            for (t, band) in thread_bands.into_iter().enumerate() {
                                let local_args = args_template.clone();
                                let start_y = t * chunk_height;
                                let end_y = (start_y + chunk_height).min(height);

                                if start_y >= height { break; }

                                let process_ref = &process_chunk;

                                s.spawn(move || {
                                    // Give this thread ownership of 'band', which contains only its disjoint slices!
                                    process_ref(start_y, end_y, local_args, band);
                                });
                            }
                        });
                    }
                }};
            }

            match depth {
                BitType::U8 => eval_loop!(u8, false),
                BitType::U16 => eval_loop!(u16, false),
                BitType::F32 => eval_loop!(f32, true),
                _ => {
                    return Err(ImageErrors::ImageOperationNotImplemented(
                        self.name(),
                        depth,
                    ))
                }
            }
        }
        Ok(())
    }
}

/// Maps a user's variable string (like "r", "cb", "k", "c0") to the actual channel index
/// based on the image's active colorspace.
#[allow(clippy::match_same_arms)]
fn get_channel_index(name: &str, cs: ColorSpace) -> Option<usize> {
    // 1. Generic Channel Matcher for Unknown or MultiBand images (c0, c1, c2...)
    // This allows users to type "c5" for a 6-channel MultiBand image and it just works.
    if let Some(num_str) = name.strip_prefix('c') {
        if let Ok(idx) = num_str.parse::<usize>() {
            return Some(idx);
        }
    }

    // 2. Specific Colorspace Bindings
    match (name, cs) {
        // RGB / RGBA
        ("r", ColorSpace::RGB | ColorSpace::RGBA) => Some(0),
        ("g", ColorSpace::RGB | ColorSpace::RGBA) => Some(1),
        ("b", ColorSpace::RGB | ColorSpace::RGBA) => Some(2),
        ("a", ColorSpace::RGBA) => Some(3),

        // BGR / BGRA
        ("b", ColorSpace::BGR | ColorSpace::BGRA) => Some(0),
        ("g", ColorSpace::BGR | ColorSpace::BGRA) => Some(1),
        ("r", ColorSpace::BGR | ColorSpace::BGRA) => Some(2),
        ("a", ColorSpace::BGRA) => Some(3),

        // ARGB (Alpha is first!)
        ("a", ColorSpace::ARGB) => Some(0),
        ("r", ColorSpace::ARGB) => Some(1),
        ("g", ColorSpace::ARGB) => Some(2),
        ("b", ColorSpace::ARGB) => Some(3),

        // HSL
        ("h", ColorSpace::HSL) => Some(0),
        ("s", ColorSpace::HSL) => Some(1),
        ("l", ColorSpace::HSL) => Some(2),

        // HSV
        ("h", ColorSpace::HSV) => Some(0),
        ("s", ColorSpace::HSV) => Some(1),
        ("v", ColorSpace::HSV) => Some(2),

        // CMYK
        ("c", ColorSpace::CMYK) => Some(0),
        ("m", ColorSpace::CMYK) => Some(1),
        ("y", ColorSpace::CMYK) => Some(2), // Yellow, not Luma Y!
        ("k", ColorSpace::CMYK) => Some(3),

        // YCbCr (JPEG internal format)
        ("y", ColorSpace::YCbCr) => Some(0),  // Luma Y
        ("cb", ColorSpace::YCbCr) => Some(1), // Blue-difference chroma
        ("cr", ColorSpace::YCbCr) => Some(2), // Red-difference chroma

        // YCCK (CMYK JPEG internal format)
        ("y", ColorSpace::YCCK) => Some(0),
        ("cb", ColorSpace::YCCK) => Some(1),
        ("cr", ColorSpace::YCCK) => Some(2),
        ("k", ColorSpace::YCCK) => Some(3),

        // Luma / LumaA (Grayscale)
        ("y" | "luma" | "v", ColorSpace::Luma | ColorSpace::LumaA) => Some(0),
        ("a", ColorSpace::LumaA) => Some(1),

        // Unrecognized variable for this colorspace
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zune_core::colorspace::ColorSpace;
    use zune_image::image::Image;
    use zune_image::traits::OperationsTrait;

    #[test]
    fn test_fx_exact_addition() {
        let mut img = Image::fill::<u8>(128, ColorSpace::RGB, 10, 10);

        // Because max_val is injected dynamically, this safely adds exactly 2
        // whether the image is u8, u16, or f32!
        let expression = Fx::new("r + (2 / MAX)");
        expression.execute(&mut img).unwrap();

        img.iter_pixels::<u8, _>(|_, _, pix| {
            assert_eq!(pix[0], 130);
            assert_eq!(pix[1], 130);
            assert_eq!(pix[2], 130);
        })
        .expect("Channel panicked");
    }

    #[test]
    fn test_fx_sine_wave_math() {
        let mut img = Image::fill::<u8>(100, ColorSpace::RGB, 10, 10);

        // Testing that built-in constants (pi) and functions (sin) map correctly
        let expression = Fx::new("r + sin(PI)");
        expression.execute(&mut img).unwrap();

        // sin(PI) is 0.0, so the pixel should remain exactly 100
        img.iter_pixels::<u8, _>(|_w, _h, pix| {
            assert_eq!(pix[0], 100);
        })
        .expect("Channel panicked");
    }
    #[test]
    fn test_fx_spatial_gradient() {
        // Create a blank 100x100 image
        let mut img = Image::fill::<u8>(0, ColorSpace::RGB, 100, 100);

        // Expression: Pixel value = X coordinate / Width
        // This generates a perfect horizontal black-to-white gradient!
        let expression = Fx::new("i / w");
        expression.execute(&mut img).unwrap();

        let channels = img.frames_ref()[0].channels_ref(ColorSpace::RGB, false);
        let r_slice = channels[0].reinterpret_as::<u8>().unwrap();

        // At x = 0 (left edge), value should be 0 (Black)
        assert_eq!(r_slice[0], 0);

        // At x = 50 (middle), value should be approx 128 (50% Gray)
        let middle_idx = 50;
        assert_eq!(r_slice[middle_idx], 128); // 50 / 100 * 255 = 127.5

        // At x = 99 (right edge), value should be approx 252 (99 / 100 * 255)
        let right_idx = 99;
        assert_eq!(r_slice[right_idx], 252);
    }

    #[test]
    fn test_fx_grayscale_conversion() {
        // Create an image that is purely solid blue
        let mut img = Image::from_fn::<u8, _>(10, 10, ColorSpace::RGB, |_, _, px| {
            px[0] = 0; // R
            px[1] = 0; // G
            px[2] = 255; // B
        });

        // Expression: Standard Luma mapping using all three channels!
        let expression = Fx::new("(r * 0.3) + (g * 0.59) + (b * 0.11)");
        expression.execute(&mut img).unwrap();

        img.iter_pixels::<u8, _>(|_, _, pix| {
            // Since only Blue was 255 (1.0 in math), the math becomes:
            // 0.0 + 0.0 + (1.0 * 0.11) = 0.11
            // 0.11 * 255 = 28.05 (Rounds to 28)
            assert_eq!(pix[0], 28);
            assert_eq!(pix[1], 28);
            assert_eq!(pix[2], 28);
        })
        .expect("Channel panicked");
    }

    #[test]
    fn test_fx_color_inversion() {
        let mut img = Image::fill::<u8>(50, ColorSpace::RGB, 10, 10);

        // Invert the Red channel
        let expression = Fx::new("1.0 - r");
        expression.execute(&mut img).unwrap();

        img.iter_pixels::<u8, _>(|_, _, pix| {
            // 50 is ~0.196. 1.0 - 0.196 = 0.804. 0.804 * 255 = 205.
            assert_eq!(pix[0], 205);
        })
        .expect("Channel panicked");
    }
}
