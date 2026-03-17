use criterion::{Criterion, black_box, criterion_group, criterion_main};
use image::{ImageFormat, Rgba, RgbaImage};
use rayo_visual::{DiffOptions, compare};

fn make_gradient_png(width: u32, height: u32) -> Vec<u8> {
    let mut img = RgbaImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let r = ((x as f32 / width as f32) * 255.0) as u8;
        let g = ((y as f32 / height as f32) * 255.0) as u8;
        let b = 128;
        *pixel = Rgba([r, g, b, 255]);
    }
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, ImageFormat::Png).unwrap();
    buf
}

fn make_shifted_png(width: u32, height: u32) -> Vec<u8> {
    let mut img = RgbaImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let r = (((x + 1) as f32 / width as f32) * 255.0).min(255.0) as u8;
        let g = (((y + 1) as f32 / height as f32) * 255.0).min(255.0) as u8;
        let b = 128;
        *pixel = Rgba([r, g, b, 255]);
    }
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, ImageFormat::Png).unwrap();
    buf
}

fn bench_compare_identical_720p(c: &mut Criterion) {
    let img = make_gradient_png(1280, 720);
    let opts = DiffOptions::default();
    c.bench_function("compare_identical_720p", |b| {
        b.iter(|| compare(black_box(&img), black_box(&img), black_box(&opts)).unwrap())
    });
}

fn bench_compare_different_720p(c: &mut Criterion) {
    let baseline = make_gradient_png(1280, 720);
    let current = make_shifted_png(1280, 720);
    let opts = DiffOptions::default();
    c.bench_function("compare_different_720p", |b| {
        b.iter(|| compare(black_box(&baseline), black_box(&current), black_box(&opts)).unwrap())
    });
}

fn bench_compare_no_overlay_720p(c: &mut Criterion) {
    let baseline = make_gradient_png(1280, 720);
    let current = make_shifted_png(1280, 720);
    let opts = DiffOptions {
        generate_overlay: false,
        ..Default::default()
    };
    c.bench_function("compare_no_overlay_720p", |b| {
        b.iter(|| compare(black_box(&baseline), black_box(&current), black_box(&opts)).unwrap())
    });
}

criterion_group!(
    benches,
    bench_compare_identical_720p,
    bench_compare_different_720p,
    bench_compare_no_overlay_720p,
);
criterion_main!(benches);
