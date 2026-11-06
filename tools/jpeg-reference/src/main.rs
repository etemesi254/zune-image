use std::error::Error;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

use turbojpeg::{Colorspace, DecompressHeader, Subsamp, YuvImage};
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const USAGE: &str = "Usage: jpeg-reference report|check [JPEG ...]\n\
                    \x20      jpeg-reference bench ITERATIONS [JPEG ...]";
const MAX_OUTPUT_BYTES: usize = 256 * 1024 * 1024;

struct Geometry {
    widths:  [usize; 3],
    heights: [usize; 3],
    strides: [usize; 3],
    lengths: [usize; 3]
}

impl Geometry {
    fn new(header: &DecompressHeader) -> Result<Self> {
        if header.colorspace != Colorspace::YCbCr
            || header.is_lossless
            || header.is_arithmetic
            || !matches!(
                header.subsamp,
                Subsamp::None | Subsamp::Sub2x1 | Subsamp::Sub2x2
            )
        {
            return Err("only lossy Huffman YCbCr 4:4:4, 4:2:2, and 4:2:0 are supported".into());
        }
        let reference = YuvImage {
            pixels:  (),
            width:   header.width,
            height:  header.height,
            align:   4,
            subsamp: header.subsamp
        };
        let uv_width = header.width.div_ceil(header.subsamp.width());
        let uv_height = header.height.div_ceil(header.subsamp.height());
        let strides = [
            reference.y_width(),
            reference.uv_width(),
            reference.uv_width()
        ];
        let lengths = [
            strides[0]
                .checked_mul(reference.y_height())
                .ok_or("Y plane size overflow")?,
            strides[1]
                .checked_mul(reference.uv_height())
                .ok_or("Cb plane size overflow")?,
            strides[2]
                .checked_mul(reference.uv_height())
                .ok_or("Cr plane size overflow")?
        ];
        let total = lengths
            .iter()
            .try_fold(0usize, |total, length| total.checked_add(*length))
            .ok_or("YUV allocation size overflow")?;
        if total > MAX_OUTPUT_BYTES {
            return Err("reference output exceeds the tool's 256 MiB limit".into());
        }
        Ok(Self {
            widths: [header.width, uv_width, uv_width],
            heights: [header.height, uv_height, uv_height],
            strides,
            lengths
        })
    }

    fn len(&self) -> usize {
        self.lengths.iter().sum()
    }
}

fn load(path: &Path) -> Result<(Vec<u8>, Geometry)> {
    let data = std::fs::read(path)?;
    if !data.ends_with(&[0xFF, 0xD9]) {
        return Err("input must end with EOI".into());
    }
    let header = turbojpeg::read_header(&data)?;
    Ok((data, Geometry::new(&header)?))
}

fn decode_zune(data: &[u8], geometry: &Geometry) -> Result<Vec<u8>> {
    let options = DecoderOptions::default().set_strict_mode(true);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode_headers()?;
    let mut raw = decoder.raw_output();
    if raw.num_components() != Some(3) || raw.component_ids().as_deref() != Some(&[1, 2, 3][..]) {
        return Err("expected Y, Cb, Cr components with selectors 1, 2, 3".into());
    }
    let layout = raw.layout().ok_or("raw layout unavailable")?;
    for (index, plane) in layout[..3].iter().enumerate() {
        if (plane.width, plane.height) != (geometry.widths[index], geometry.heights[index]) {
            return Err(format!("component {index}: decoder plane dimensions disagree").into());
        }
    }
    // Use the reference's padded allocation and strides, but compare logical samples only.
    let mut pixels = vec![0; geometry.len()];
    let (y, uv) = pixels.split_at_mut(geometry.lengths[0]);
    let (u, v) = uv.split_at_mut(geometry.lengths[1]);
    raw.decode_into_planes_strided(&mut [y, u, v], &geometry.strides)?;
    Ok(pixels)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Difference {
    samples: usize,
    maximum: u8,
    first:   Option<(usize, usize, u8, u8)>
}

fn plane_difference(
    actual: &[u8], reference: &[u8], width: usize, height: usize, stride: usize
) -> Difference {
    let mut difference = Difference::default();
    for y in 0..height {
        for x in 0..width {
            let actual = actual[y * stride + x];
            let expected = reference[y * stride + x];
            if actual != expected {
                difference.samples += 1;
                difference.maximum = difference.maximum.max(actual.abs_diff(expected));
                difference.first.get_or_insert((x, y, actual, expected));
            }
        }
    }
    difference
}

fn compare(data: &[u8], geometry: &Geometry) -> Result<[Difference; 3]> {
    let actual = decode_zune(data, geometry)?;
    let reference = turbojpeg::decompress_to_yuv(data)?;
    if reference.pixels.len() != geometry.len() {
        return Err("reference allocation size disagrees with its header".into());
    }
    let mut offset = 0;
    Ok(std::array::from_fn(|index| {
        let end = offset + geometry.lengths[index];
        let difference = plane_difference(
            &actual[offset..end],
            &reference.pixels[offset..end],
            geometry.widths[index],
            geometry.heights[index],
            geometry.strides[index]
        );
        offset = end;
        difference
    }))
}

fn milliseconds<T>(iterations: usize, mut operation: impl FnMut() -> Result<T>) -> Result<f64> {
    let start = Instant::now();
    for _ in 0..iterations {
        drop(black_box(operation()?));
    }
    Ok(start.elapsed().as_secs_f64() * 1000.0 / iterations as f64)
}

fn default_paths() -> Vec<PathBuf> {
    [
        "flat_420.jpg",
        "texture_444.jpg",
        "texture_422.jpg",
        "texture_420.jpg"
    ]
    .map(|name| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(name)
    })
    .to_vec()
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().ok_or(USAGE)?;
    if mode == "--help" {
        println!("{USAGE}");
        return Ok(());
    }
    if !matches!(mode.as_str(), "report" | "check" | "bench") {
        return Err(USAGE.into());
    }
    let iterations = if mode == "bench" {
        let count = args
            .next()
            .ok_or("bench requires a positive iteration count")?;
        let count = count.parse::<usize>()?;
        if count == 0 {
            return Err("bench requires a positive iteration count".into());
        }
        count
    } else {
        0
    };
    let mut paths: Vec<PathBuf> = args.map(PathBuf::from).collect();
    if paths.is_empty() {
        paths = default_paths();
    }
    println!("Reference: locked TurboJPEG 1.5.1, full-size YUV, four-byte row alignment");
    let mut different = false;
    for path in paths {
        let (data, geometry) =
            load(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let differences =
            compare(&data, &geometry).map_err(|error| format!("{}: {error}", path.display()))?;
        let matches = differences.iter().all(|difference| difference.samples == 0);
        println!(
            "{} {}",
            if matches { "MATCH" } else { "DIFFERENT" },
            path.display()
        );
        for (name, difference) in ["Y", "Cb", "Cr"].iter().zip(&differences) {
            println!(
                "  {name}: differing={}, max_delta={}, first(x,y,actual,reference)={:?}",
                difference.samples, difference.maximum, difference.first
            );
        }
        different |= !matches;
        if mode == "bench" {
            if !matches {
                return Err("refusing to benchmark outputs that do not match exactly".into());
            }
            let zune = milliseconds(iterations, || decode_zune(&data, &geometry))?;
            let reference = milliseconds(iterations, || Ok(turbojpeg::decompress_to_yuv(&data)?))?;
            println!(
                "  fresh decoder + allocation, {iterations} iterations: \
                 zune={zune:.3} ms/image, turbojpeg={reference:.3} ms/image"
            );
        }
    }
    if mode == "check" && different {
        return Err("exact raw sample comparison failed".into());
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_is_not_compared() {
        assert_eq!(
            plane_difference(&[1, 2, 99, 99], &[1, 2, 0, 0], 2, 1, 4),
            Difference::default()
        );
    }

    #[test]
    fn differences_include_count_maximum_and_first_coordinate() {
        assert_eq!(
            plane_difference(&[9, 10, 0, 0], &[8, 7, 0, 0], 2, 1, 4),
            Difference {
                samples: 2,
                maximum: 3,
                first:   Some((0, 0, 9, 8))
            }
        );
    }

    #[test]
    fn coordinates_follow_row_stride() {
        assert_eq!(
            plane_difference(
                &[1, 2, 99, 99, 3, 4, 99, 99],
                &[1, 2, 0, 0, 3, 1, 0, 0],
                2,
                2,
                4
            ),
            Difference {
                samples: 1,
                maximum: 3,
                first:   Some((1, 1, 4, 1))
            }
        );
    }

    #[test]
    fn oversized_output_is_rejected_before_allocation() {
        let data = std::fs::read(&default_paths()[0]).unwrap();
        let mut header = turbojpeg::read_header(&data).unwrap();
        header.width = 65535;
        header.height = 65535;
        assert!(Geometry::new(&header).is_err());
    }

    #[test]
    fn flat_control_matches_reference() {
        let (data, geometry) = load(&default_paths()[0]).unwrap();
        assert!(compare(&data, &geometry)
            .unwrap()
            .iter()
            .all(|difference| difference.samples == 0));
    }
}
