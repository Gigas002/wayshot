//! Benchmarks for image operations (rotation, color conversion).
//! These run without a Wayland compositor.

use criterion::{Criterion, criterion_group, criterion_main};
use image::{DynamicImage, ImageBuffer, RgbaImage};
use std::hint::black_box;
use std::time::Duration;
use wayland_client::protocol::wl_output::Transform;

use libwayshot::region::Size;
use libwayshot::{create_converter, rotate_image_buffer};

fn make_image(w: u32, h: u32) -> DynamicImage {
    let buf: RgbaImage =
        ImageBuffer::from_raw(w, h, (0..w * h * 4).map(|i| i as u8).collect()).unwrap();
    DynamicImage::ImageRgba8(buf)
}

fn bench_rotate(c: &mut Criterion) {
    let mut group = c.benchmark_group("rotate_image_buffer");
    // 90° and 270° are ~334 ms/iter; need enough time to collect 100 samples
    group.measurement_time(Duration::from_secs(35));
    let logical = Size {
        width: 1920,
        height: 1080,
    };

    for (name, transform) in [
        ("normal", Transform::Normal),
        ("90", Transform::_90),
        ("180", Transform::_180),
        ("270", Transform::_270),
        ("flipped", Transform::Flipped),
    ] {
        group.bench_function(name, |b| {
            let image = make_image(1920, 1080);
            b.iter(|| {
                let img = image.clone();
                black_box(rotate_image_buffer(img, transform, logical, 1.0))
            });
        });
    }
    group.finish();
}

fn bench_convert(c: &mut Criterion) {
    use wayland_client::protocol::wl_shm;

    let mut group = c.benchmark_group("convert_inplace");
    // xrgb8888 ~6s, bgr10 ~17s for 100 samples
    group.measurement_time(Duration::from_secs(20));
    let size = 1920 * 1080 * 4;
    let mut data = vec![0u8; size];

    for (name, format) in [
        ("xbgr8888", wl_shm::Format::Xbgr8888),
        ("xrgb8888", wl_shm::Format::Xrgb8888),
        ("bgr10", wl_shm::Format::Abgr2101010),
    ] {
        let converter = match create_converter(format) {
            Some(c) => c,
            None => continue,
        };
        group.bench_function(name, |b| {
            b.iter(|| {
                data.fill(0x42);
                black_box(converter.convert_inplace(black_box(&mut data)));
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_rotate, bench_convert);
criterion_main!(benches);
