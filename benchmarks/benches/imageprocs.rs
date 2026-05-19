#![allow(clippy::field_reassign_with_default)]

use std::fs::read;
use std::hint::black_box;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use fast_image_resize::{PixelType, ResizeAlg};
use image::imageops::FilterType;
use image::DynamicImage;
use libblur::{
    AnisotropicRadius, BlurImageMut, EdgeMode, EdgeMode2D, FastBlurChannels, ThreadingPolicy,
};
use libvips::ops::{Angle, Direction, GammaOptions, Kernel, ResizeOptions};
use libvips::VipsImage;
use zune_benches::sample_path;
use zune_hdr::zune_core::bytestream::ZCursor;
use zune_hdr::zune_core::options::DecoderOptions;
use zune_image::image::Image;
use zune_image::metadata::AlphaState;
use zune_image::traits::OperationsTrait;
use zune_imageprocs::affine::AffineTransform;
use zune_imageprocs::flip::{Flip, FlipDirection};
use zune_imageprocs::gamma::Gamma;
use zune_imageprocs::gaussian_blur::GaussianBlur;
use zune_imageprocs::invert::Invert;
use zune_imageprocs::premul_alpha::PremultiplyAlpha;
use zune_imageprocs::resize::{Resize, ResizeDimensions, ResizeMethod};
use zune_imageprocs::rotate::Rotate;
use zune_imageprocs::sobel::Sobel;
use zune_imageprocs::transpose::Transpose;

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
    let im = libvips::ops::resize_with_opts(input, 0.79, &options).unwrap();
    im.image_write_to_memory();
    black_box(im);
}

fn fir_resize_bench(input: &fast_image_resize::images::Image, resize_alg: ResizeAlg) {
    let mut resizer = fast_image_resize::Resizer::new();
    let mut output = fast_image_resize::images::Image::new(
        input.width() * 79 / 100,
        input.height() * 79 / 100,
        input.pixel_type(),
    );

    resizer
        .resize(
            input,
            &mut output,
            Some(&fast_image_resize::ResizeOptions::new().resize_alg(resize_alg)),
        )
        .expect("image resize failed");
    black_box(output);
}
fn zune_image_resize_bench(input: &Image, resize_method: ResizeMethod) {
    let im = Resize::new(ResizeDimensions::Percentage(79, 79), resize_method)
        .clone_and_execute(input)
        .unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}

fn image_rs_resize_bench(input: &DynamicImage, filter_type: FilterType) {
    let (w, h) = (input.width(), input.height());
    let im = input.resize(w * 79 / 100, h * 79 / 100, filter_type);
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
fn zune_affine_transform_bench(input: &Image) {
    // hyperfine './zune --affine-transform 0.7071 -0.7071 0.7071 0.7071  0 0  -i "/Users/etemesi/Downloads/wallhaven-3qqdg6_3840x2160.png"  -o h.jpg' 'vips affine "/Users/etemesi/Downloads/wallhaven-3qqdg6_3840x2160.png"  output.jpg "0.707107 -0.707107 0.707107 0.707107"'
    let im = AffineTransform::new(
        std::f32::consts::FRAC_1_SQRT_2,
        -std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
        0.0,
        0.0,
    )
    .clone_and_execute(input)
    .unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}
fn vips_affine_transform_bench(input: &VipsImage) {
    let im = libvips::ops::affine(
        input,
        std::f64::consts::FRAC_1_SQRT_2,
        -std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
    )
    .unwrap();
    im.image_write_to_memory();
    black_box(im);
}
fn bench_inner_zune_vips<T, U>(c: &mut Criterion, name: &str, zune_fn: T, vips_fn: U)
where
    T: Fn(&Image),
    U: Fn(&VipsImage),
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
    c: &mut Criterion, name: &str, zune_fn: T, image_rs_fn: U, vips_fn: V,
) where
    T: Fn(&Image),
    U: Fn(&image::DynamicImage),
    V: Fn(&VipsImage),
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

fn bench_inner_resize_zune_vips_image_rs<T, U, V, W>(
    c: &mut Criterion, name: &str, zune_fn: T, image_rs_fn: U, vips_fn: V, fir_fn: W,
) where
    T: Fn(&Image),
    U: Fn(&image::DynamicImage),
    V: Fn(&VipsImage),
    W: Fn(&fast_image_resize::images::Image),
{
    let path = sample_path().join("test-images/jpeg/benchmarks/speed_bench.jpg");

    let data = read(path).unwrap();
    let zune_im = Image::read(ZCursor::new(&data), DecoderOptions::default()).unwrap();
    let vips_im = VipsImage::new_from_buffer(&data, ".jpg").unwrap();
    let image_rs_im = image::load_from_memory(&data).unwrap();
    let fir_image = fast_image_resize::images::Image::new(
        zune_im.width() as u32,
        zune_im.height() as u32,
        PixelType::U8x3,
    );
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
    group.bench_function("fir", |b| {
        b.iter(|| {
            fir_fn(&fir_image);
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
        vips_gauss_blur_bench,
    );
}

fn bench_premultiply_alpha(c: &mut Criterion) {
    bench_inner_zune_vips(
        c,
        "imageprocs: premultiply",
        zune_image_premultiply,
        vips_premultiply_bench,
    );
}

fn bench_rotate90(c: &mut Criterion) {
    bench_inner_zune_vips_image_rs(
        c,
        "imageprocs: rotate 90",
        zune_image_rotate90_bench,
        image_rs_rotate_90,
        vips_rotate90_bench,
    );
}

fn bench_rotate180(c: &mut Criterion) {
    bench_inner_zune_vips(
        c,
        "imageprocs: rotate 180",
        zune_image_rotate180_bench,
        vips_rotate180_bench,
    );
}

fn bench_invert(c: &mut Criterion) {
    bench_inner_zune_vips(
        c,
        "imageprocs: invert",
        zune_image_invert_bench,
        vips_invert_bench,
    );
}
fn bench_resize_generic(
    c: &mut Criterion, name: &str, zune_resize: ResizeMethod, image_resize: FilterType,
    vips_resize: Kernel, fir_algo: fast_image_resize::FilterType,
) {
    bench_inner_resize_zune_vips_image_rs(
        c,
        name,
        |c| zune_image_resize_bench(c, zune_resize),
        |c| image_rs_resize_bench(c, image_resize),
        |c| vips_resize_bench(c, vips_resize),
        |c| fir_resize_bench(c, fast_image_resize::ResizeAlg::Convolution(fir_algo)),
    );
}
fn bench_resize_linear(c: &mut Criterion) {
    bench_resize_generic(
        c,
        "imageprocs: resize-linear-kernel",
        ResizeMethod::Bilinear,
        FilterType::Triangle,
        Kernel::Linear,
        fast_image_resize::FilterType::Bilinear,
    );
}

fn bench_resize_bicubic(c: &mut Criterion) {
    bench_resize_generic(
        c,
        "imageprocs: resize - lanczos-kernel",
        ResizeMethod::Lanczos3,
        FilterType::Lanczos3,
        Kernel::Lanczos3,
        fast_image_resize::FilterType::Lanczos3,
    );
}
fn bench_resize_caltmull(c: &mut Criterion) {
    bench_resize_generic(
        c,
        "imageprocs: resize - mitchell",
        ResizeMethod::Mitchell,
        FilterType::Lanczos3,
        Kernel::Mitchell,
        fast_image_resize::FilterType::Mitchell,
    );
}
fn bench_flip_horizontal(c: &mut Criterion) {
    bench_inner_zune_vips(
        c,
        "imageprocs: flip-horizontal",
        zune_image_flip_horizonal_bench,
        vips_flip_horizontal_bench,
    );
}
fn bench_flip_vertical(c: &mut Criterion) {
    bench_inner_zune_vips(
        c,
        "imageprocs: flip-vertical",
        zune_image_flip_vertical_bench,
        vips_flip_vertical_bench,
    );
}
fn bench_affine_transform(c: &mut Criterion) {
    bench_inner_zune_vips(
        c,
        "imageprocs: affine-transform (45 degrees rotation)",
        zune_affine_transform_bench,
        vips_affine_transform_bench,
    )
}

fn zune_image_transpose(input: &Image) {
    // vips by default uses 2.4 for gamma, so no need to specify
    let im = Transpose::new().clone_and_execute(input).unwrap();
    im.flatten_frames::<u8>();
    black_box(im);
}

fn bench_transpose(c: &mut Criterion) {
    let path = sample_path().join("test-images/jpeg/benchmarks/speed_bench.jpg");

    let data = read(path).unwrap();
    let zune_im = Image::read(ZCursor::new(&data), DecoderOptions::default()).unwrap();

    let mut group = c.benchmark_group("imageprocs: transpose");

    group.bench_function("zune-image", |b| {
        b.iter(|| {
            zune_image_transpose(&zune_im);
            black_box(());
        })
    });
}

fn blur_gaussian_blur_bench(c: &mut Criterion) {
    let path = sample_path().join("test-images/jpeg/benchmarks/speed_bench.jpg");

    let data = read(path).unwrap();
    let zune_im = Image::read(ZCursor::new(&data), DecoderOptions::default()).unwrap();
    let vips_im = VipsImage::new_from_buffer(&data, ".jpg").unwrap();
    let raw_pix = zune_im.flatten_frames::<u8>();
    let mut out_side = vec![0; raw_pix[0].len()];
    out_side.copy_from_slice(&raw_pix[0]);
    let (w, h) = zune_im.dimensions();

    let mut group = c.benchmark_group("imageprocs: gaussian blur - new");

    group.bench_function("zune-image", |b| {
        b.iter(|| {
            let im = GaussianBlur::new(3.0).clone_and_execute(&zune_im).unwrap();
            im.flatten_frames::<u8>();
            black_box(());
        });
    });
    group.bench_function("vips-image", |b| {
        b.iter(|| {
            let im = libvips::ops::gaussblur(&vips_im, 3.0).unwrap();
            im.image_write_to_memory();
            black_box(im);
        })
    });
    group.bench_function("libblur (planar mode 3 passes)", |b| {
        b.iter(|| {
            for item in out_side.chunks_exact_mut(w * h) {
                let mut dst_image =
                    BlurImageMut::borrow(item, w as u32, h as u32, FastBlurChannels::Plane);
                libblur::fast_gaussian(
                    &mut dst_image,
                    AnisotropicRadius::new(3),
                    ThreadingPolicy::Adaptive,
                    EdgeMode2D::new(EdgeMode::Clamp),
                )
                .unwrap();
            }
        })
    });
    group.bench_function("libblur (interleaved mode (3 channels))", |b| {
        b.iter(|| {
            let mut dst_image = BlurImageMut::borrow(
                &mut out_side,
                w as u32,
                h as u32,
                FastBlurChannels::Channels3,
            );
            libblur::fast_gaussian(
                &mut dst_image,
                AnisotropicRadius::new(3),
                ThreadingPolicy::Adaptive,
                EdgeMode2D::new(EdgeMode::Clamp),
            )
            .unwrap();
        })
    });

    group.finish();
}
criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(10))
      };
    targets=bench_affine_transform,bench_sobel,bench_gamma,bench_gaussian,bench_premultiply_alpha,bench_rotate90,bench_rotate180,bench_invert,bench_resize_linear,bench_resize_bicubic,bench_flip_horizontal,bench_flip_vertical,bench_resize_caltmull,bench_transpose,blur_gaussian_blur_bench);

criterion_main!(benches);
