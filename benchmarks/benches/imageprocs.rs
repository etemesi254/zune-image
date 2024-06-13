use std::fs::read;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use libvips::ops::{Angle, GammaOptions};
use libvips::VipsImage;
use zune_benches::sample_path;
use zune_hdr::zune_core::bytestream::ZCursor;
use zune_hdr::zune_core::options::DecoderOptions;
use zune_image::image::Image;
use zune_image::metadata::AlphaState;
use zune_image::traits::OperationsTrait;
use zune_imageprocs::gamma::Gamma;
use zune_imageprocs::gaussian_blur::GaussianBlur;
use zune_imageprocs::premul_alpha::PremultiplyAlpha;
use zune_imageprocs::rotate::Rotate;
use zune_imageprocs::sobel::Sobel;

fn vips_sobel_bench(input: &VipsImage) {
    let im = libvips::ops::sobel(input).unwrap();
    im.image_write_to_memory();
    black_box(im);
}

fn zune_sobel_bench(input: &zune_image::image::Image) {
    let im = Sobel::new().clone_and_execute(input).unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}

fn vips_gamma_bench(input: &VipsImage) {
    let mut gamma = GammaOptions::default();
    gamma.exponent = 2.5;
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = libvips::ops::gamma_with_opts(input, &gamma).unwrap();
    im.image_write_to_memory();
    black_box(im);
}
fn zune_gamma_bench(input: &zune_image::image::Image) {
    let im = Gamma::new(2.4).clone_and_execute(input).unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}

fn vips_gauss_blur_bench(input: &VipsImage) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = libvips::ops::gaussblur(input, 3.0).unwrap();
    im.image_write_to_memory();
    black_box(im);
}

fn zune_image_gauss_blur_bench(input: &Image) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = GaussianBlur::new(3.0).clone_and_execute(input).unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}

fn vips_premultiply_bench(input: &VipsImage) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = libvips::ops::premultiply(input).unwrap();
    im.image_write_to_memory();
    black_box(im);
}

fn zune_image_premultiply(input: &Image) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = PremultiplyAlpha::new(AlphaState::PreMultiplied)
        .clone_and_execute(input)
        .unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}

fn vips_rotate90_bench(input: &VipsImage) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = libvips::ops::rot(input, Angle::D90).unwrap();
    im.image_write_to_memory();
    black_box(im);
}

fn zune_image_rotate90_bench(input: &Image) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = Rotate::new(90.0).clone_and_execute(input).unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}

fn bench_inner<T, U>(c: &mut Criterion, name: &str, zune_fn: T, vips_fn: U)
where
    T: Fn(&Image),
    U: Fn(&VipsImage)
{
    let path = sample_path().join("test-images/jpeg/benchmarks/speed_bench.jpg");

    let data = read(path).unwrap();
    let zune_im = Image::read(ZCursor::new(&data), DecoderOptions::default()).unwrap();
    let vips_im = libvips::VipsImage::new_from_buffer(&data, ".jpg").unwrap();

    let mut group = c.benchmark_group(name);

    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("libvips", |b| {
        b.iter(|| {
            vips_fn(&vips_im);
            black_box(());
        })
    });

    group.bench_function("zune-image", |b| {
        b.iter(|| {
            zune_fn(&zune_im);
            black_box(());
        })
    });
}

fn bench_gamma(c: &mut Criterion) {
    bench_inner(c, "imageprocs: gamma", zune_gamma_bench, vips_gamma_bench);
}
fn bench_sobel(c: &mut Criterion) {
    bench_inner(c, "imageprocs: sobel", zune_sobel_bench, vips_sobel_bench);
}

fn bench_gaussian(c: &mut Criterion) {
    bench_inner(
        c,
        "imageprocs: gaussian blur",
        zune_image_gauss_blur_bench,
        vips_gauss_blur_bench
    );
}

fn bench_premultiply_alpha(c: &mut Criterion) {
    bench_inner(
        c,
        "imageprocs: premultiply",
        zune_image_premultiply,
        vips_premultiply_bench
    );
}

fn bench_rotate90(c: &mut Criterion) {
    bench_inner(
        c,
        "imageprocs: rotate 90",
        zune_image_rotate90_bench,
        vips_rotate90_bench
    );
}
criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(10))
      };
    targets=bench_sobel,bench_gamma,bench_gaussian,bench_premultiply_alpha,bench_rotate90);

criterion_main!(benches);
