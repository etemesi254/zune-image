use std::fs::read;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use image::imageops::FilterType;
use image::DynamicImage;
use libvips::ops::{Angle, Direction, GammaOptions, Kernel, ResizeOptions};
use libvips::VipsImage;
use zune_benches::sample_path;
use zune_hdr::zune_core::bytestream::ZCursor;
use zune_hdr::zune_core::options::DecoderOptions;
use zune_image::image::Image;
use zune_image::metadata::AlphaState;
use zune_image::traits::OperationsTrait;
use zune_imageprocs::flip::{Flip, FlipDirection};
use zune_imageprocs::gamma::Gamma;
use zune_imageprocs::gaussian_blur::GaussianBlur;
use zune_imageprocs::invert::Invert;
use zune_imageprocs::premul_alpha::PremultiplyAlpha;
use zune_imageprocs::resize::{Resize, ResizeMethod};
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

fn image_rs_rotate_90(input: &image::DynamicImage) {
    black_box(input.rotate90().to_rgb8());
}

fn image_rs_gaussian_blur(input: &image::DynamicImage) {
    black_box(input.blur(3.0).to_rgb8());
}
fn vips_rotate180_bench(input: &VipsImage) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = libvips::ops::rot(input, Angle::D180).unwrap();
    im.image_write_to_memory();
    black_box(im);
}

fn zune_image_rotate180_bench(input: &Image) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = Rotate::new(180.0).clone_and_execute(input).unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}
fn vips_invert_bench(input: &VipsImage) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = libvips::ops::invert(input).unwrap();
    im.image_write_to_memory();
    black_box(im);
}

fn zune_image_invert_bench(input: &Image) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = Invert.clone_and_execute(input).unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}

fn vips_resize_bench(input: &VipsImage, kernel: Kernel) {
    let mut options = ResizeOptions::default();
    options.kernel = kernel;
    options.vscale = 0.5;
    let im = libvips::ops::resize_with_opts(input, 0.5, &options).unwrap();
    im.image_write_to_memory();
    black_box(im);
}

fn zune_image_resize_bench(input: &Image, resize_method: ResizeMethod) {
    let (w, h) = input.dimensions();
    let im = Resize::new(w / 2, h / 2, resize_method)
        .clone_and_execute(input)
        .unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}

fn image_rs_resize_bench(input: &DynamicImage, filter_type: FilterType) {
    let (w, h) = (input.width(), input.height());
    let im = input.resize(w / 2, h / 2, filter_type);
    let c = im.as_flat_samples_u8().unwrap().samples;
    black_box(c);
}

fn vips_flip_horizontal_bench(input: &VipsImage) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = libvips::ops::flip(input, Direction::Horizontal).unwrap();
    im.image_write_to_memory();
    black_box(im);
}

fn zune_image_flip_horizonal_bench(input: &Image) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = Flip::new(FlipDirection::Horizontal)
        .clone_and_execute(input)
        .unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}

fn vips_flip_vertical_bench(input: &VipsImage) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = libvips::ops::flip(input, Direction::Vertical).unwrap();
    im.image_write_to_memory();
    black_box(im);
}

fn zune_image_flip_vertical_bench(input: &Image) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = Flip::new(FlipDirection::Horizontal)
        .clone_and_execute(input)
        .unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}
fn bench_inner_zune_vips<T, U>(c: &mut Criterion, name: &str, zune_fn: T, vips_fn: U)
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

fn bench_inner_zune_vips_image_rs<T, U, V>(
    c: &mut Criterion, name: &str, zune_fn: T, image_rs_fn: U, vips_fn: V
) where
    T: Fn(&Image),
    U: Fn(&image::DynamicImage),
    V: Fn(&VipsImage)
{
    let path = sample_path().join("test-images/jpeg/benchmarks/speed_bench.jpg");

    let data = read(path).unwrap();
    let zune_im = Image::read(ZCursor::new(&data), DecoderOptions::default()).unwrap();
    let vips_im = VipsImage::new_from_buffer(&data, ".jpg").unwrap();
    let image_rs_im = image::load_from_memory(&data).unwrap();

    let mut group = c.benchmark_group(name);

    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("vips", |b| {
        b.iter(|| {
            vips_fn(&vips_im);
            black_box(());
        })
    });
    group.bench_function("image-rs", |b| {
        b.iter(|| {
            image_rs_fn(&image_rs_im);
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
    bench_inner_zune_vips(c, "imageprocs: gamma", zune_gamma_bench, vips_gamma_bench);
}
fn bench_sobel(c: &mut Criterion) {
    bench_inner_zune_vips(c, "imageprocs: sobel", zune_sobel_bench, vips_sobel_bench);
}

fn bench_gaussian(c: &mut Criterion) {
    bench_inner_zune_vips_image_rs(
        c,
        "imageprocs: gaussian blur",
        zune_image_gauss_blur_bench,
        image_rs_gaussian_blur,
        vips_gauss_blur_bench
    );
}

fn bench_premultiply_alpha(c: &mut Criterion) {
    bench_inner_zune_vips(
        c,
        "imageprocs: premultiply",
        zune_image_premultiply,
        vips_premultiply_bench
    );
}

fn bench_rotate90(c: &mut Criterion) {
    bench_inner_zune_vips_image_rs(
        c,
        "imageprocs: rotate 90",
        zune_image_rotate90_bench,
        image_rs_rotate_90,
        vips_rotate90_bench
    );
}

fn bench_rotate180(c: &mut Criterion) {
    bench_inner_zune_vips(
        c,
        "imageprocs: rotate 180",
        zune_image_rotate180_bench,
        vips_rotate180_bench
    );
}

fn bench_invert(c: &mut Criterion) {
    bench_inner_zune_vips(
        c,
        "imageprocs: invert",
        zune_image_invert_bench,
        vips_invert_bench
    );
}
fn bench_resize_linear(c: &mut Criterion) {
    bench_inner_zune_vips_image_rs(
        c,
        "imageprocs: resize-linear-kernel",
        |c| zune_image_resize_bench(c, ResizeMethod::Bilinear),
        |c| image_rs_resize_bench(c, FilterType::Triangle),
        |c| vips_resize_bench(c, Kernel::Linear)
    );
}

fn bench_resize_bicubic(c: &mut Criterion) {
    bench_inner_zune_vips_image_rs(
        c,
        "imageprocs: resize-cubic-kernel",
        |c| zune_image_resize_bench(c, ResizeMethod::Bicubic),
        |c| image_rs_resize_bench(c, FilterType::CatmullRom),
        |c| vips_resize_bench(c, Kernel::Cubic)
    );
}
fn bench_flip_horizontal(c: &mut Criterion) {
    bench_inner_zune_vips(
        c,
        "imageprocs: flip-horizontal",
        zune_image_flip_horizonal_bench,
        vips_flip_horizontal_bench
    );
}
fn bench_flip_vertical(c: &mut Criterion) {
    bench_inner_zune_vips(
        c,
        "imageprocs: flip-vertical",
        zune_image_flip_vertical_bench,
        vips_flip_vertical_bench
    );
}
criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(10))
      };
    targets=bench_sobel,bench_gamma,bench_gaussian,bench_premultiply_alpha,bench_rotate90,bench_rotate180,bench_invert,bench_resize_linear,bench_resize_bicubic,bench_flip_horizontal,bench_flip_vertical);

criterion_main!(benches);
