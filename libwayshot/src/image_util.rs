use image::{DynamicImage, imageops::FilterType};
use wayland_client::protocol::wl_output::Transform;

use crate::region::Size;

/// When we still need to upscale this much to align with the composite (`scaling_left` > 1),
/// prefer a stronger filter — e.g. mixed-DPI layouts with a large correction factor.
const SCALING_LEFT_THRESHOLD: f64 = 2.0;

fn resize_filter_for_scale(max_scale: f64, scaling_left: f64) -> FilterType {
    if scaling_left >= SCALING_LEFT_THRESHOLD {
        return FilterType::Lanczos3;
    }
    let is_integer_dpi = (max_scale - max_scale.round()).abs() < 1e-3;
    if is_integer_dpi {
        FilterType::Triangle
    } else {
        FilterType::CatmullRom
    }
}

fn scaling_left(rotated_width: u32, logical_size: Size, max_scale: f64) -> f64 {
    tracing::trace!(
        "Rotated width: {rotated_width}, logical width: {}",
        logical_size.width
    );
    let scale = rotated_width as f64 / logical_size.width as f64;
    let scaling_left = max_scale / scale;
    tracing::debug!("Current scale: {scale}, scaling left (max/current): {scaling_left}");
    scaling_left
}

/// Rotate and optionally scale an image according to Wayland output transform.
/// Public for benchmarks (`bench` feature); otherwise use via crate internals.
#[tracing::instrument(skip(image))]
pub fn rotate_image_buffer(
    image: DynamicImage,
    transform: Transform,
    // Includes transform already.
    logical_size: Size,
    max_scale: f64,
) -> DynamicImage {
    let rotated_image = match transform {
        Transform::_90 => image::imageops::rotate90(&image).into(),
        Transform::_180 => image::imageops::rotate180(&image).into(),
        Transform::_270 => image::imageops::rotate270(&image).into(),
        Transform::Flipped => image::imageops::flip_horizontal(&image).into(),
        Transform::Flipped90 => {
            let flipped_buffer = image::imageops::flip_horizontal(&image);
            image::imageops::rotate90(&flipped_buffer).into()
        }
        Transform::Flipped180 => {
            let flipped_buffer = image::imageops::flip_horizontal(&image);
            image::imageops::rotate180(&flipped_buffer).into()
        }
        Transform::Flipped270 => {
            let flipped_buffer = image::imageops::flip_horizontal(&image);
            image::imageops::rotate270(&flipped_buffer).into()
        }
        Transform::Normal => return image,
        _ => image,
    };

    let sl = scaling_left(rotated_image.width(), logical_size, max_scale);
    if sl <= 1.0 {
        tracing::debug!("No scaling left to do");
        return rotated_image;
    }

    let new_width = (rotated_image.width() as f64 * sl).round() as u32;
    let new_height = (rotated_image.height() as f64 * sl).round() as u32;
    let filter = resize_filter_for_scale(max_scale, sl);
    tracing::debug!("Resizing image to {new_width}x{new_height} with {filter:?}");
    image::imageops::resize(&rotated_image, new_width, new_height, filter).into()
}
