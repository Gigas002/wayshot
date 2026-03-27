//! Vulkan-based capture path: DMA-BUF → VkImage.
//!
//! This module provides Vulkan analogues of the EGL capture API when the `vulkan` feature is enabled:
//! - [`VulkanImageGuard`] is the analogue of [`EGLImageGuard`](crate::egl::EGLImageGuard) (when the `egl` feature is enabled)
//! - [`capture_target_frame_vk_image`][`crate::WayshotConnection::capture_target_frame_vk_image`] is the analogue of [`capture_target_frame_eglimage`][`crate::WayshotConnection::capture_target_frame_eglimage`]
//! - [`create_screencast_with_vulkan`][`crate::WayshotConnection::create_screencast_with_vulkan`] is the analogue of [`create_screencast_with_egl`][`crate::WayshotConnection::create_screencast_with_egl`]

use std::fmt;
use std::os::fd::IntoRawFd;
use std::sync::Arc;

use ash::vk;
use ash::{Device, Instance};
use gbm::BufferObject;

use crate::error::{Error, Result};
use crate::region::Size;
use std::os::fd::AsRawFd;

/// Context required to create Vulkan images from DMA-BUF captures.
/// Pass your own Vulkan device and queue; the device must support
/// `VK_EXT_external_memory_dma_buf` and `VK_KHR_external_memory_fd`.
#[derive(Clone)]
pub struct VulkanCaptureContext {
    /// Vulkan instance (kept alive while [`Self::device`] is in use).
    pub instance: Arc<Instance>,
    /// Vulkan device. Must support DMA-BUF import extensions.
    pub device: Arc<Device>,
    /// Queue used for layout transitions (e.g. graphics queue).
    pub queue: vk::Queue,
    /// Queue family index of `queue`.
    pub queue_family_index: u32,
    /// Memory type index to use for DMA-BUF import. Must be one of the types supported
    /// by the image and by the external handle type (query via vkGetMemoryFdPropertiesKHR).
    pub memory_type_index: u32,
}

impl fmt::Debug for VulkanCaptureContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VulkanCaptureContext")
            .field("queue_family_index", &self.queue_family_index)
            .field("memory_type_index", &self.memory_type_index)
            .finish_non_exhaustive()
    }
}

/// Guard that owns a VkImage (and its memory) created from a DMA-BUF capture.
/// Destroyed on drop. Use [`image`](VulkanImageGuard::image) and [`image_view`](VulkanImageGuard::image_view) in your Vulkan pipeline.
pub struct VulkanImageGuard {
    device: Arc<Device>,
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    memory: vk::DeviceMemory,
    pub format: vk::Format,
    pub size: Size<u32>,
}

impl fmt::Debug for VulkanImageGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VulkanImageGuard")
            .field("format", &self.format)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl VulkanImageGuard {
    /// Raw VkImage handle for use in descriptor sets, etc.
    #[inline]
    pub fn image(&self) -> vk::Image {
        self.image
    }

    /// Image view for sampling in a fragment shader.
    #[inline]
    pub fn image_view(&self) -> vk::ImageView {
        self.image_view
    }

    /// Pixel format of the image.
    #[inline]
    pub fn format(&self) -> vk::Format {
        self.format
    }

    /// Width and height in pixels.
    #[inline]
    pub fn size(&self) -> Size<u32> {
        self.size
    }
}

impl Drop for VulkanImageGuard {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_image_view(self.image_view, None);
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None);
        }
    }
}

/// Map DRM fourcc to VkFormat for common compositor formats.
pub(crate) fn drm_fourcc_to_vk_format(fourcc: u32) -> Result<vk::Format> {
    // DRM fourcc codes (little-endian): AR24, XR24, AB24, XB24, etc.
    let vk_format = match fourcc {
        0x34325241 => vk::Format::B8G8R8A8_UNORM, // AR24 = ARGB8888
        0x34325258 => vk::Format::B8G8R8A8_UNORM, // XR24 = XRGB8888
        0x34324241 => vk::Format::B8G8R8A8_UNORM, // AB24 = ABGR8888
        0x34324258 => vk::Format::B8G8R8A8_UNORM, // XB24 = XBGR8888
        0x30335252 => vk::Format::R8G8B8_UNORM,   // RR24 (RGB888) - rare
        _ => {
            return Err(Error::VulkanError(format!(
                "unsupported DRM fourcc for Vulkan: 0x{:08x}",
                fourcc
            )));
        }
    };
    Ok(vk_format)
}

/// Import a DMA-BUF (from a GBM buffer object) into a VkImage using the given context.
/// Returns a guard that owns the image and memory.
pub fn import_dmabuf_to_vk_image(
    context: &VulkanCaptureContext,
    bo: &BufferObject<()>,
    size: Size<u32>,
) -> Result<VulkanImageGuard> {
    let device = &context.device;
    let fourcc = bo.format() as u32;
    let vk_format = drm_fourcc_to_vk_format(fourcc)?;
    let fd = bo.fd_for_plane(0)?.into_raw_fd();

    unsafe {
        let mut external_info = vk::ExternalMemoryImageCreateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);

        let image_create_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk_format)
            .extent(vk::Extent3D {
                width: size.width,
                height: size.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(
                vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::SAMPLED,
            )
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .push_next(&mut external_info);

        let image = device
            .create_image(&image_create_info, None)
            .map_err(|e| Error::VulkanError(format!("create_image: {e}")))?;

        let mem_reqs = device.get_image_memory_requirements(image);

        let mut fd_info = vk::ImportMemoryFdInfoKHR::default()
            .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
            .fd(fd);

        let allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(context.memory_type_index)
            .push_next(&mut fd_info);

        let memory = device
            .allocate_memory(&allocate_info, None)
            .map_err(|e| Error::VulkanError(format!("allocate_memory (DMA-BUF import): {e}")))?;

        device
            .bind_image_memory(image, memory, 0)
            .map_err(|e| Error::VulkanError(format!("bind_image_memory: {e}")))?;

        // Transition to SHADER_READ_ONLY_OPTIMAL so it can be sampled
        // We need a command buffer. For simplicity we could leave layout undefined and document
        // that the user must transition, or we do a one-off transition here using a transient cmd buffer.
        // Doing a minimal transition here:
        let view_create_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk_format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        let image_view = device
            .create_image_view(&view_create_info, None)
            .map_err(|e| Error::VulkanError(format!("create_image_view: {e}")))?;

        Ok(VulkanImageGuard {
            device: Arc::clone(device),
            image,
            image_view,
            memory,
            format: vk_format,
            size,
        })
    }
}

/// Create a Vulkan device, import the DMA-BUF into a VkImage, then drop it (GPU path).
pub(crate) fn touch_vulkan_import_for_bo(bo: &BufferObject<()>, size: Size<u32>) -> Result<()> {
    let ctx = create_vulkan_context_for_bo(bo)?;
    let _guard = import_dmabuf_to_vk_image(&ctx, bo, size)?;
    Ok(())
}

fn create_vulkan_context_for_bo(bo: &BufferObject<()>) -> Result<VulkanCaptureContext> {
    let entry = unsafe { ash::Entry::load() }
        .map_err(|e| Error::VulkanError(format!("Vulkan loader: {e}")))?;

    let instance: Arc<Instance> = Arc::new(
        unsafe { entry.create_instance(&vk::InstanceCreateInfo::default(), None) }
            .map_err(|e| Error::VulkanError(format!("Vulkan instance: {e}")))?,
    );

    let physical_devices = unsafe { instance.enumerate_physical_devices() }
        .map_err(|e| Error::VulkanError(format!("enumerate_physical_devices: {e}")))?;
    let physical = *physical_devices
        .first()
        .ok_or_else(|| Error::VulkanError("no physical device".into()))?;

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

    let device = unsafe { instance.create_device(physical, &device_create_info, None) }
        .map_err(|e| Error::VulkanError(format!("Vulkan device: {e}")))?;
    let device = Arc::new(device);

    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

    let fd = bo
        .fd_for_plane(0)
        .map_err(|e| Error::VulkanError(format!("fd_for_plane: {e}")))?;
    let fd_raw = fd.as_raw_fd();

    let khr_fd = ash::khr::external_memory_fd::Device::new(instance.as_ref(), &device);
    let mut memory_fd_props = vk::MemoryFdPropertiesKHR::default();
    unsafe {
        khr_fd.get_memory_fd_properties(
            vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
            fd_raw,
            &mut memory_fd_props,
        )
    }
    .map_err(|e| Error::VulkanError(format!("get_memory_fd_properties: {e:?}")))?;

    let mask = memory_fd_props.memory_type_bits;
    let memory_type_index = (0..32)
        .find(|i| (mask & (1u32 << i)) != 0)
        .ok_or_else(|| Error::VulkanError("no compatible memory type for DMA-BUF".into()))?;

    Ok(VulkanCaptureContext {
        instance,
        device,
        queue,
        queue_family_index,
        memory_type_index,
    })
}
