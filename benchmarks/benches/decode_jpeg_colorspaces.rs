use std::fs::read;
use std::hint::black_box;
use std::time::{Duration, Instant};

use zune_benches::sample_path;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

const REPETITIONS: usize = 7;
const MODES: [(&str, ColorSpace); 4] = [
    ("RGB", ColorSpace::RGB),
    ("RGBA", ColorSpace::RGBA),
    ("BGR", ColorSpace::BGR),
    ("BGRA", ColorSpace::BGRA),
];

fn output_buffer(data: &[u8], colorspace: ColorSpace) -> Vec<u8> {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(colorspace);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode_headers().unwrap();
    vec![0; decoder.output_buffer_size().unwrap()]
}

fn decode_once(data: &[u8], colorspace: ColorSpace, output: &mut [u8]) -> Duration {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(colorspace);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    let start = Instant::now();
    decoder.decode_into(output).unwrap();
    let elapsed = start.elapsed();
    black_box(output);
    elapsed
}

fn statistics(samples: &mut [Duration]) -> (f64, f64) {
    samples.sort_unstable();
    let median = samples[samples.len() / 2].as_secs_f64() * 1_000.0;
    let mean =
        samples.iter().map(Duration::as_secs_f64).sum::<f64>() / samples.len() as f64 * 1_000.0;
    let variance = samples
        .iter()
        .map(|sample| {
            let milliseconds = sample.as_secs_f64() * 1_000.0;
            (milliseconds - mean).powi(2)
        })
        .sum::<f64>()
        / samples.len() as f64;
    (median, variance.sqrt())
}

fn benchmark_image(name: &str, data: &[u8]) {
    let mut outputs: [Vec<u8>; MODES.len()] =
        core::array::from_fn(|index| output_buffer(data, MODES[index].1));

    for (index, (_, colorspace)) in MODES.iter().enumerate() {
        black_box(decode_once(data, *colorspace, &mut outputs[index]));
    }

    let mut samples: [Vec<Duration>; 4] = core::array::from_fn(|_| Vec::new());
    for repetition in 0..REPETITIONS {
        let mut order = [0, 1, 2, 3];
        order.rotate_left(repetition % MODES.len());
        if repetition % 2 != 0 {
            order.reverse();
        }
        for index in order {
            samples[index].push(decode_once(data, MODES[index].1, &mut outputs[index]));
        }
    }

    println!("{name}");
    for (index, (mode, _)) in MODES.iter().enumerate() {
        let (median, standard_deviation) = statistics(&mut samples[index]);
        println!(
            "  {mode}: median={median:.3} ms stddev={standard_deviation:.3} ms cv={:.2}%",
            standard_deviation / median * 100.0
        );
    }
}

fn main() {
    let core = core_affinity::get_core_ids()
        .and_then(|cores| cores.into_iter().next())
        .expect("no CPU core available for benchmark affinity");
    assert!(core_affinity::set_for_current(core));

    let baseline =
        read(sample_path().join("test-images/jpeg/benchmarks/speed_bench_hv_subsampling.jpg"))
            .unwrap();
    let progressive =
        read(sample_path().join("test-images/jpeg/benchmarks/speed_bench_prog.jpg")).unwrap();

    benchmark_image("baseline", &baseline);
    benchmark_image("progressive", &progressive);
}
