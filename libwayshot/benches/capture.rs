//! Benchmarks for capture backends (EGL vs Vulkan).
//!
//! **Requirements:** Run under a Wayland session with a compositor (e.g. Sway, Hyprland).
//! Set `WAYLAND_DISPLAY` and have a DRI device (e.g. `/dev/dri/renderD128`).
//!
//! - EGL: build with `--features egl` (default).
//! - Vulkan: build with `--features vulkan`. Needs a Vulkan driver and the same Wayland/DRI setup.

use criterion::{Criterion, criterion_group, criterion_main};
use libwayshot::{WayshotConnection, WayshotTarget};
use std::hint::black_box;
use std::time::Duration;
use wayland_client::Connection;

/// Setup connection with DMA-BUF (required for both EGL and Vulkan capture).
/// Uses the default render node (`/dev/dri/renderD128`).
fn connect_with_dmabuf() -> Result<(WayshotConnection, WayshotTarget), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let wayshot = WayshotConnection::from_connection_with_dmabuf(conn, "/dev/dri/renderD128")?;
    let outputs = wayshot.get_all_outputs();
    let output = outputs.first().ok_or("no outputs")?;
    let target = WayshotTarget::from(output.clone());
    Ok((wayshot, target))
}

#[cfg(feature = "egl")]
fn bench_egl_capture(c: &mut Criterion) {
    let (wayshot, target) = match connect_with_dmabuf() {
        Ok(x) => x,
        Err(e) => {
            eprintln!("EGL capture bench skipped (no Wayland/DRI): {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("capture_egl");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));
    group.bench_function("capture_target_frame_eglimage", |b| {
        b.iter(|| {
            black_box(wayshot.capture_target_frame_eglimage(black_box(&target), false, None))
        });
    });
    group.finish();
}

#[cfg(not(feature = "egl"))]
fn bench_egl_capture(_c: &mut Criterion) {}

#[cfg(feature = "vulkan")]
fn bench_vulkan_capture(c: &mut Criterion) {
    use ash::vk;
    use std::os::fd::AsRawFd;
    use std::sync::Arc;

    let (wayshot, target) = match connect_with_dmabuf() {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Vulkan capture bench skipped (no Wayland/DRI): {}", e);
            return;
        }
    };

    // One DMA-BUF capture to get a fd for querying memory type
    let (_format, _guard, bo) = match wayshot.capture_target_frame_dmabuf(&target, false, None) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Vulkan bench: dmabuf capture failed: {}", e);
            return;
        }
    };

    let entry = match unsafe { ash::Entry::load() } {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Vulkan bench skipped (Vulkan loader failed): {}", e);
            return;
        }
    };

    let instance = match unsafe { entry.create_instance(&vk::InstanceCreateInfo::default(), None) }
    {
        Ok(i) => std::sync::Arc::new(i),
        Err(e) => {
            eprintln!("Vulkan bench skipped (instance creation failed): {}", e);
            return;
        }
    };

    let physical_devices = unsafe { instance.enumerate_physical_devices().unwrap_or_default() };
    let physical = match physical_devices.first() {
        Some(&p) => p,
        None => {
            eprintln!("Vulkan bench skipped: no physical device");
            return;
        }
    };

    let queue_family_index = 0u32;
    let queue_info = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&[1.0]);

    let device_extensions = [
        b"VK_KHR_external_memory_fd\0".as_ptr().cast(),
        b"VK_EXT_external_memory_dma_buf\0".as_ptr().cast(),
    ];

    let device_create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(std::slice::from_ref(&queue_info))
        .enabled_extension_names(&device_extensions);

    let device = match unsafe { instance.create_device(physical, &device_create_info, None) } {
        Ok(d) => Arc::new(d),
        Err(e) => {
            eprintln!("Vulkan bench skipped (device creation failed): {}", e);
            return;
        }
    };

    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

    // Query memory type index for DMA-BUF import via KHR_external_memory_fd
    let fd = match bo.fd_for_plane(0) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Vulkan bench: fd_for_plane failed: {}", e);
            return;
        }
    };
    let fd_raw = fd.as_raw_fd();

    let khr_fd = ash::khr::external_memory_fd::Device::new(instance.as_ref(), &device);
    let mut memory_fd_props = vk::MemoryFdPropertiesKHR::default();
    let memory_type_index = match unsafe {
        khr_fd.get_memory_fd_properties(
            vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
            fd_raw,
            &mut memory_fd_props,
        )
    } {
        Ok(()) => {
            let mask = memory_fd_props.memory_type_bits;
            (0..32).find(|i| (mask & (1u32 << i)) != 0).unwrap_or(0)
        }
        Err(e) => {
            eprintln!("Vulkan bench: get_memory_fd_properties failed: {:?}", e);
            return;
        }
    };

    let context = libwayshot::VulkanCaptureContext {
        instance: Arc::clone(&instance),
        device: Arc::clone(&device),
        queue,
        queue_family_index,
        memory_type_index,
    };

    let mut group = c.benchmark_group("capture_vulkan");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));
    group.bench_function("capture_target_frame_vk_image", |b| {
        b.iter(|| {
            black_box(wayshot.capture_target_frame_vk_image(
                black_box(&context),
                black_box(&target),
                false,
                None,
            ))
        });
    });
    group.finish();
}

#[cfg(not(feature = "vulkan"))]
fn bench_vulkan_capture(_c: &mut Criterion) {}

fn criterion_benchmark(c: &mut Criterion) {
    bench_egl_capture(c);
    bench_vulkan_capture(c);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
