#[cfg(feature = "allocation-count")]
mod allocations;
mod input;

use std::cell::Cell;
use std::hint::black_box;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use input::Growing;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::{JpegDecoder, RawImcuRowStatus, ScanlineReadStatus, ScanlineStatus};

struct Image {
    name:       String,
    group:      String,
    width:      usize,
    height:     usize,
    input:      ColorSpace,
    source:     ColorSpace,
    formats:    Vec<ColorSpace>,
    iterations: usize,
    bytes:      Vec<u8>
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Api {
    Into,
    Scan36,
    Scan1,
    Raw,
    Yuv,
    Decode
}

#[derive(Clone, Copy)]
struct Case {
    image:   usize,
    color:   ColorSpace,
    growing: bool,
    simd:    bool,
    api:     Api
}

struct Output {
    pixels:      Vec<u8>,
    calls:       usize,
    suspensions: usize,
    rows:        usize
}

fn color(name: &str) -> ColorSpace {
    match name {
        "RGB" => ColorSpace::RGB,
        "RGBA" => ColorSpace::RGBA,
        "BGRA" => ColorSpace::BGRA,
        "Luma" => ColorSpace::Luma,
        "CMYK" => ColorSpace::CMYK,
        _ => panic!("unknown corpus color {name}")
    }
}

fn load(path: &Path) -> Vec<Image> {
    let images: Vec<_> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .skip(1)
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 9, "invalid prepared corpus");
            let (input, source) = match fields[5] {
                "ycbcr" => (ColorSpace::YCbCr, ColorSpace::RGB),
                "gray" => (ColorSpace::Luma, ColorSpace::Luma),
                "rgb" => (ColorSpace::RGB, ColorSpace::RGB),
                "cmyk" => (ColorSpace::CMYK, ColorSpace::CMYK),
                kind => panic!("unknown corpus kind {kind}")
            };
            let bytes = std::fs::read(fields[2]).unwrap();
            assert!(bytes.starts_with(&[0xFF, 0xD8]) && bytes.len() > 4);
            Image {
                name: fields[0].to_owned(),
                group: fields[1].to_owned(),
                width: fields[3].parse().unwrap(),
                height: fields[4].parse().unwrap(),
                input,
                source,
                formats: fields[6].split('|').map(color).collect(),
                iterations: fields[7].parse().unwrap(),
                bytes
            }
        })
        .collect();
    assert_eq!(
        images.len(),
        16,
        "empty/incomplete corpus is not a benchmark"
    );
    images
}

fn expose(visible: &Cell<usize>, length: usize) {
    assert!(
        visible.get() < length,
        "needs input after all bytes were exposed"
    );
    visible.set((visible.get() + 16384).min(length));
}

fn once(image: &Image, case: Case, capture: bool) -> Output {
    let visible = Rc::new(Cell::new(if case.growing {
        image.bytes.len().min(4096)
    } else {
        image.bytes.len()
    }));
    let reader = Growing::new(black_box(&image.bytes), Rc::clone(&visible));
    let options = DecoderOptions::default()
        .set_use_unsafe(case.simd)
        .jpeg_set_out_colorspace(case.color);
    let mut decoder = JpegDecoder::new_with_options(reader, options);
    decoder.set_incremental_mode(case.growing);
    let mut suspensions = 0;
    loop {
        match decoder.decode_headers() {
            Ok(()) => break,
            Err(error) if case.growing && error.is_recoverable_eof() => {
                suspensions += 1;
                expose(&visible, image.bytes.len());
            }
            Err(error) => panic!("{} headers: {error:?}", image.name)
        }
    }
    assert_eq!(decoder.dimensions(), Some((image.width, image.height)));
    assert_eq!(decoder.input_colorspace(), Some(image.input));
    if matches!(case.api, Api::Into | Api::Decode | Api::Yuv) {
        let mut calls = 0;
        let pixels = if case.api == Api::Decode {
            assert!(!case.growing);
            calls += 1;
            decoder.decode().unwrap()
        } else {
            let mut output = vec![0; decoder.output_buffer_size().unwrap()];
            loop {
                calls += 1;
                match decoder.decode_into(&mut output) {
                    Ok(()) => break,
                    Err(error) if case.growing && error.is_recoverable_eof() => {
                        suspensions += 1;
                        expose(&visible, image.bytes.len());
                    }
                    Err(error) => panic!("{} body: {error:?}", image.name)
                }
            }
            output
        };
        assert_eq!(
            pixels.len(),
            image.width * image.height * case.color.num_components()
        );
        assert_eq!(decoder.decoded_scanlines(), Some(image.height));
        black_box(&pixels);
        return Output {
            pixels: if capture { pixels } else { Vec::new() },
            calls,
            suspensions,
            rows: image.height
        };
    }
    if case.api == Api::Raw {
        let mut raw = decoder.raw_output();
        let layout = raw.layout().unwrap();
        let count = raw.num_components().unwrap();
        assert_eq!(count, image.input.num_components());
        let strides: [usize; 4] =
            core::array::from_fn(|i| if i < count { layout[i].width } else { 0 });
        let mut chunks: [Vec<u8>; 4] = core::array::from_fn(|i| {
            if i < count {
                vec![0; strides[i] * 8 * layout[i].vertical_sampling_factor]
            } else {
                Vec::new()
            }
        });
        let mut planes: [Vec<u8>; 4] = core::array::from_fn(|i| {
            if capture && i < count {
                vec![0; strides[i] * layout[i].height]
            } else {
                Vec::new()
            }
        });
        let mut rows = [0; 4];
        let mut calls = 0;
        loop {
            calls += 1;
            let [a, b, c, d] = &mut chunks;
            let status = raw
                .decode_next_imcu_row(
                    &mut [&mut a[..], &mut b[..], &mut c[..], &mut d[..]][..count],
                    &strides[..count]
                )
                .unwrap();
            match status {
                RawImcuRowStatus::RowReady { rows_written } => {
                    assert!(rows_written[..count].iter().any(|&n| n > 0));
                    for i in 0..count {
                        let end = rows[i] + rows_written[i];
                        assert!(end <= layout[i].height);
                        if capture {
                            planes[i][rows[i] * strides[i]..end * strides[i]]
                                .copy_from_slice(&chunks[i][..rows_written[i] * strides[i]]);
                        }
                        black_box(&chunks[i][..rows_written[i] * strides[i]]);
                        rows[i] = end;
                    }
                }
                RawImcuRowStatus::NeedMoreInput if case.growing => {
                    suspensions += 1;
                    expose(&visible, image.bytes.len());
                }
                RawImcuRowStatus::Complete => break,
                status => panic!("{} raw: {status:?}", image.name)
            }
        }
        for i in 0..count {
            assert_eq!(rows[i], layout[i].height);
        }
        return Output {
            pixels: if capture { planes.into_iter().flatten().collect() } else { Vec::new() },
            calls,
            suspensions,
            rows: image.height
        };
    }
    let request = if case.api == Api::Scan1 { 1 } else { 36 };
    let row_bytes = image.width * case.color.num_components();
    let mut chunk = vec![0; row_bytes * request.min(image.height)];
    let mut pixels = if capture { vec![0; row_bytes * image.height] } else { Vec::new() };
    let mut session = decoder.scanline_output();
    assert!(matches!(session.start().unwrap(), ScanlineStatus::Ready));
    let mut rows = 0;
    let mut calls = 0;
    while rows < image.height {
        calls += 1;
        let capacity = request.min(image.height - rows);
        match session
            .read_scanlines(&mut chunk[..capacity * row_bytes], row_bytes)
            .unwrap()
        {
            ScanlineReadStatus::RowsProcessed { rows: written } => {
                assert!(written > 0 && written <= capacity);
                if capture {
                    pixels[rows * row_bytes..(rows + written) * row_bytes]
                        .copy_from_slice(&chunk[..written * row_bytes]);
                }
                black_box(&chunk[..written * row_bytes]);
                rows += written;
            }
            ScanlineReadStatus::NeedMoreInput if case.growing => {
                suspensions += 1;
                expose(&visible, image.bytes.len());
            }
            status => panic!("{} scanline {rows}: {status:?}", image.name)
        }
    }
    assert_eq!(session.output_scanline(), image.height);
    loop {
        match session.finish().unwrap() {
            ScanlineStatus::Complete => break,
            ScanlineStatus::NeedMoreInput if case.growing => {
                suspensions += 1;
                expose(&visible, image.bytes.len());
            }
            status => panic!("{} finish: {status:?}", image.name)
        }
    }
    Output {
        pixels,
        calls,
        suspensions,
        rows
    }
}

fn cases(images: &[Image]) -> Vec<Case> {
    let mut cases = Vec::new();
    for (image, item) in images.iter().enumerate() {
        for &color in &item.formats {
            for growing in [false, true] {
                for api in [Api::Into, Api::Scan36] {
                    cases.push(Case {
                        image,
                        color,
                        growing,
                        simd: true,
                        api
                    });
                }
            }
        }
        for growing in [false, true] {
            for api in [Api::Into, Api::Scan36] {
                cases.push(Case {
                    image,
                    color: item.source,
                    growing,
                    simd: false,
                    api
                });
            }
            for simd in [false, true] {
                for api in [Api::Scan1, Api::Raw] {
                    cases.push(Case {
                        image,
                        color: item.source,
                        growing,
                        simd,
                        api
                    });
                }
            }
            if item.input == ColorSpace::YCbCr {
                cases.push(Case {
                    image,
                    color: ColorSpace::YCbCr,
                    growing,
                    simd: true,
                    api: Api::Yuv
                });
            }
        }
        cases.push(Case {
            image,
            color: item.source,
            growing: false,
            simd: true,
            api: Api::Decode
        });
    }
    assert_eq!(cases.len(), 416);
    cases
}

fn id(image: &Image, case: Case) -> String {
    format!(
        "{}:{:?}:{}:{}:{:?}",
        image.name,
        case.color,
        if case.growing { "growing" } else { "resident" },
        if case.simd { "auto" } else { "scalar" },
        case.api
    )
}

fn primary(image: &Image, case: Case) -> bool {
    case.simd && case.api == Api::Scan36 && case.color == image.source
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(args.len() >= 3, "usage: jpeg-arm64-benchmark validate|emit|measure|dispatch|profile|allocations CORPUS [SEED|API MODE] [smoke]");
    let images = load(Path::new(&args[2]));
    let mut workloads = cases(&images);
    assert_eq!(
        workloads
            .iter()
            .filter(|&&case| primary(&images[case.image], case))
            .count(),
        32
    );
    match args[1].as_str() {
        "list" => {
            println!("case,primary,iterations,rows");
            for case in workloads {
                let image = &images[case.image];
                println!(
                    "{},{},{},{}",
                    id(image, case),
                    usize::from(primary(image, case)),
                    image.iterations,
                    image.height
                );
            }
        }
        "validate" => {
            for case in workloads {
                let image = &images[case.image];
                let actual = once(image, case, true);
                let reference = once(
                    image,
                    Case {
                        growing: false,
                        simd: false,
                        api: if case.api == Api::Raw { Api::Raw } else { Api::Into },
                        ..case
                    },
                    true
                );
                assert_eq!(actual.pixels, reference.pixels, "{}", id(image, case));
                assert!(!actual.pixels.is_empty() && actual.rows == image.height);
            }
            println!("VALIDATED 416 nonempty complete cases against scalar pixels");
        }
        "emit" => {
            let mut writer = std::io::BufWriter::new(std::io::stdout().lock());
            for case in workloads {
                let image = &images[case.image];
                let output = once(image, case, true);
                assert!(!output.pixels.is_empty());
                writeln!(
                    writer,
                    "{}\t{}\t{}",
                    id(image, case),
                    output.rows,
                    output.pixels.len()
                )
                .unwrap();
                writer.write_all(&output.pixels).unwrap();
            }
            writeln!(writer, "END\t416\t0").unwrap();
        }
        "measure" => {
            let mut seed: u64 = args[3].parse().unwrap();
            let smoke = args.get(4).is_some_and(|value| value == "smoke");
            for i in (1..workloads.len()).rev() {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                workloads.swap(i, seed as usize % (i + 1));
            }
            println!("case,primary,ns_per_image,batch_ns,iterations,calls,suspensions,rows");
            for case in workloads {
                let image = &images[case.image];
                let iterations = if smoke { 1 } else { image.iterations };
                black_box(once(image, case, false));
                let start = Instant::now();
                let mut last = Output {
                    pixels:      Vec::new(),
                    calls:       0,
                    suspensions: 0,
                    rows:        0
                };
                for _ in 0..iterations {
                    last = black_box(once(image, case, false));
                }
                let elapsed = start.elapsed().as_nanos();
                assert!(elapsed > 0 && (smoke || elapsed >= 100_000));
                println!(
                    "{},{},{:.3},{elapsed},{iterations},{},{},{}",
                    id(image, case),
                    usize::from(primary(image, case)),
                    elapsed as f64 / iterations as f64,
                    last.calls,
                    last.suspensions,
                    last.rows
                );
            }
        }
        "dispatch" => {
            let disabled = DecoderOptions::default().set_use_unsafe(false);
            assert!(!disabled.use_neon() && !disabled.use_avx2() && !disabled.use_sse2());
            let format = color(&args[3]);
            let simd = match args[4].as_str() {
                "auto" => true,
                "scalar" => false,
                _ => panic!("invalid dispatch policy")
            };
            let mut completed = 0;
            for (index, image) in images.iter().enumerate() {
                if (image.group == "small" || image.name == "odd-420-photo")
                    && image.formats.contains(&format)
                {
                    for api in [Api::Into, Api::Scan36] {
                        black_box(once(
                            image,
                            Case {
                                image: index,
                                color: format,
                                growing: false,
                                simd,
                                api
                            },
                            false
                        ));
                        completed += 1;
                    }
                }
            }
            println!("DISPATCH_COMPLETED {completed} JPEG cases");
            assert!(completed > 0);
        }
        "profile" => {
            let api = match args[3].as_str() {
                "Into" => Api::Into,
                "Scan36" => Api::Scan36,
                _ => panic!("invalid profile API")
            };
            let simd = match args[4].as_str() {
                "auto" => true,
                "scalar" => false,
                _ => panic!("invalid profile policy")
            };
            let until = Instant::now() + Duration::from_secs(1200);
            eprintln!("PROFILE_READY");
            let mut completed = 0;
            while Instant::now() < until {
                for (image, item) in images.iter().enumerate() {
                    for &color in &item.formats {
                        for growing in [false, true] {
                            black_box(once(
                                item,
                                Case {
                                    image,
                                    color,
                                    growing,
                                    simd,
                                    api
                                },
                                false
                            ));
                            completed += 1;
                            eprintln!("PROFILE_COMPLETED {completed}");
                        }
                    }
                }
            }
        }
        "allocations" => {
            #[cfg(feature = "allocation-count")]
            {
                println!("case,allocations,requested_bytes,peak_live_bytes,remaining_live_bytes");
                for case in workloads {
                    allocations::start();
                    drop(once(&images[case.image], case, false));
                    let (calls, bytes, peak, live) = allocations::stop();
                    assert_eq!(live, 0);
                    println!(
                        "{},{calls},{bytes},{peak},{live}",
                        id(&images[case.image], case)
                    );
                }
            }
            #[cfg(not(feature = "allocation-count"))]
            panic!("allocations mode requires allocation-count; never use this feature for timing");
        }
        _ => panic!("unknown mode")
    }
}
