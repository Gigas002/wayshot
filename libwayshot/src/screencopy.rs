use std::{
    ffi::CString,
    os::fd::OwnedFd,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "dmabuf")]
use gbm::BufferObject;
use image::{ColorType, DynamicImage, EncodableLayout, RgbImage, RgbaImage};
use memmap2::MmapMut;
use rustix::{
    fs::{self, SealFlags},
    io, shm,
};
use wayland_client::protocol::{
    wl_buffer::WlBuffer, wl_output, wl_shm::Format, wl_shm_pool::WlShmPool,
};

use crate::{
    Error, Result,
    convert::create_converter,
    region::{LogicalRegion, Size},
};

/// Runs its closure when dropped: destroys protocol objects (frames,
/// sessions, sources) on every return path, errors included.
/// wayland-client never destroys a proxy on drop.
pub(crate) struct OnDrop<F: FnMut()>(pub(crate) F);

impl<F: FnMut()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        (self.0)()
    }
}

pub struct FrameGuard {
    pub buffer: WlBuffer,
    pub shm_pool: WlShmPool,
    pub size: Size,
    pub transform: Option<wl_output::Transform>,
}

impl Drop for FrameGuard {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.shm_pool.destroy();
    }
}

#[cfg(feature = "dmabuf")]
pub struct DMAFrameGuard {
    pub buffer: WlBuffer,
}
#[cfg(feature = "dmabuf")]
impl Drop for DMAFrameGuard {
    fn drop(&mut self) {
        self.buffer.destroy();
    }
}

/// Type of frame supported by the compositor. For now we only support Argb8888, Xrgb8888, and
/// Xbgr8888.
///
/// See `zwlr_screencopy_frame_v1::Event::Buffer` as it's retrieved from there.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct FrameFormat {
    pub format: Format,
    /// Size of the frame in pixels. This will always be in "landscape" so a
    /// portrait 1080x1920 frame will be 1920x1080 and will need to be rotated!
    pub size: Size,
    /// Stride is the number of bytes between the start of a row and the start of the next row.
    pub stride: u32,
}

/// Type of DMABUF frame supported by the compositor
///
/// See `zwlr_screencopy_frame_v1::Event::linux_dmabuf` as it's retrieved from there.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct DMAFrameFormat {
    pub format: u32,
    /// Size of the frame in pixels. This will always be in "landscape" so a
    /// portrait 1080x1920 frame will be 1920x1080 and will need to be rotated!
    pub size: Size,
}

impl FrameFormat {
    /// Returns the size of the frame in bytes, which is the stride * height.
    pub fn byte_size(&self) -> u64 {
        self.stride as u64 * self.size.height as u64
    }
}

#[derive(Debug)]
pub enum FrameData {
    Mmap(MmapMut),
    #[cfg(feature = "dmabuf")]
    GBMBo(BufferObject<()>),
}
/// The copied frame comprising of the FrameFormat, ColorType (Rgba8), and a memory backed shm
/// file that holds the image data in it.
#[derive(Debug)]
pub struct FrameCopy {
    pub frame_format: FrameFormat,
    pub frame_color_type: ColorType,
    pub frame_data: FrameData,
    pub transform: wl_output::Transform,
    /// Logical region with the transform already applied.
    pub logical_region: LogicalRegion,
    pub physical_size: Size,
    pub(crate) color_converted: bool,
    /// Part of the (rotated) frame to keep, when the compositor returned a
    /// whole output for a region (ext-image-copy has no region capture).
    pub(crate) crop: Option<crate::region::Region>,
}

impl FrameCopy {
    pub(crate) fn convert_color_inplace(&mut self) -> Result<ColorType, Error> {
        if self.color_converted {
            return Ok(self.frame_color_type);
        }
        let frame_color_type = match create_converter(self.frame_format.format) {
            Some(converter) => {
                #[cfg_attr(not(feature = "dmabuf"), allow(irrefutable_let_patterns))]
                let FrameData::Mmap(raw) = &mut self.frame_data else {
                    return Err(Error::InvalidColor);
                };
                converter.convert_inplace(raw)
            }
            _ => {
                tracing::error!("Unsupported buffer format: {:?}", self.frame_format.format);
                tracing::error!(
                    "You can send a feature request for the above format to the mailing list for wayshot over at https://sr.ht/~shinyzenith/wayshot."
                );
                return Err(Error::NoSupportedBufferFormat);
            }
        };

        self.frame_color_type = frame_color_type;
        self.color_converted = true;
        Ok(frame_color_type)
    }

    pub(crate) fn get_image(&mut self) -> Result<DynamicImage, Error> {
        self.convert_color_inplace()?;
        let image: DynamicImage = (&*self).try_into()?;
        Ok(image)
    }
}

impl TryFrom<&FrameCopy> for DynamicImage {
    type Error = Error;

    fn try_from(value: &FrameCopy) -> Result<Self> {
        value.allocate_image()
    }
}

impl FrameCopy {
    /// Build the image: rows are read `stride` bytes apart (a compositor may
    /// pad them), and pixels are placed according to the output transform.
    fn allocate_image(&self) -> Result<DynamicImage> {
        let bpp = match self.frame_color_type {
            ColorType::Rgb8 => 3,
            ColorType::Rgba8 => 4,
            _ => return Err(Error::InvalidColor),
        };
        let Size { width, height } = self.frame_format.size;
        let (w, h) = (width as usize, height as usize);
        let stride = self.frame_format.stride as usize;
        let row = w * bpp;
        let data = self.data();
        if h > 0 && (stride < row || data.len() < stride * (h - 1) + row) {
            return Err(Error::BufferTooSmall);
        }

        if self.transform == wl_output::Transform::Normal
            && let Some(crop) = self.crop
        {
            // Copy only the kept rectangle (a small region of a 4K output).
            let (x, y) = (crop.position.x as usize, crop.position.y as usize);
            let (cw, ch) = (crop.size.width as usize, crop.size.height as usize);
            if x + cw > w || y + ch > h {
                return Err(Error::BufferTooSmall);
            }
            let mut pixels = vec![0u8; cw * ch * bpp];
            for (v, out) in pixels.chunks_exact_mut(cw * bpp).enumerate() {
                let start = (y + v) * stride + x * bpp;
                out.copy_from_slice(&data[start..start + cw * bpp]);
            }
            return Self::image_from_pixels(bpp, crop.size.width, crop.size.height, pixels);
        }

        let (image_width, image_height) = self.image_shape();
        let mut pixels = vec![0u8; w * h * bpp];
        if self.transform == wl_output::Transform::Normal {
            for (v, out) in pixels.chunks_exact_mut(row).enumerate() {
                out.copy_from_slice(&data[v * stride..v * stride + row]);
            }
        } else {
            for v in 0..h {
                let source_row = &data[v * stride..v * stride + row];
                for (u, pixel) in source_row.chunks_exact(bpp).enumerate() {
                    let (x, y) = self.pixel_position(u as u32, v as u32);
                    let at = (y as usize * image_width as usize + x as usize) * bpp;
                    pixels[at..at + bpp].copy_from_slice(pixel);
                }
            }
        }

        let image = Self::image_from_pixels(bpp, image_width, image_height, pixels)?;
        Ok(match self.crop {
            Some(crop) => image.crop_imm(
                crop.position.x as u32,
                crop.position.y as u32,
                crop.size.width,
                crop.size.height,
            ),
            None => image,
        })
    }

    fn image_from_pixels(
        bpp: usize,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Result<DynamicImage> {
        let image = match bpp {
            3 => RgbImage::from_raw(width, height, pixels).map(DynamicImage::from),
            _ => RgbaImage::from_raw(width, height, pixels).map(DynamicImage::from),
        };
        image.ok_or(Error::BufferTooSmall)
    }

    fn data(&self) -> &[u8] {
        match &self.frame_data {
            FrameData::Mmap(frame_mmap) => frame_mmap.as_bytes(),
            #[cfg(feature = "dmabuf")]
            FrameData::GBMBo(_) => unimplemented!("it is still not used, and todo"),
        }
    }

    /// Where the frame's pixel (u, v) goes in the image.
    fn pixel_position(&self, u: u32, v: u32) -> (u32, u32) {
        let Size { width, height } = self.frame_format.size;
        match self.transform {
            wl_output::Transform::Normal => (u, v),
            wl_output::Transform::_90 => (height - v - 1, u),
            wl_output::Transform::_180 => (width - u - 1, height - v - 1),
            wl_output::Transform::_270 => (v, width - u - 1),
            wl_output::Transform::Flipped => (width - u - 1, v),
            wl_output::Transform::Flipped90 => (v, u),
            wl_output::Transform::Flipped180 => (u, height - v - 1),
            wl_output::Transform::Flipped270 => (height - v - 1, width - u - 1),
            _ => unreachable!(),
        }
    }

    fn image_shape(&self) -> (u32, u32) {
        match self.transform {
            wl_output::Transform::Normal
            | wl_output::Transform::_180
            | wl_output::Transform::Flipped
            | wl_output::Transform::Flipped180 => {
                (self.frame_format.size.width, self.frame_format.size.height)
            }
            _ => (self.frame_format.size.height, self.frame_format.size.width),
        }
    }
}

fn get_mem_file_handle() -> String {
    format!(
        "/libwayshot-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|time| time.subsec_nanos().to_string())
            .unwrap_or("unknown".into())
    )
}

/// Return a RawFd to a shm file. We use memfd create on linux and shm_open for BSD support.
/// You don't need to mess around with this function, it is only used by
/// capture_output_frame.
pub fn create_shm_fd() -> std::io::Result<OwnedFd> {
    // Only try memfd on linux and freebsd.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    loop {
        // Create a file that closes on successful execution and seal it's operations.
        match fs::memfd_create(
            CString::new("libwayshot")?.as_c_str(),
            fs::MemfdFlags::CLOEXEC | fs::MemfdFlags::ALLOW_SEALING,
        ) {
            Ok(fd) => {
                // This is only an optimization, so ignore errors.
                // F_SEAL_SRHINK = File cannot be reduced in size.
                // F_SEAL_SEAL = Prevent further calls to fcntl().
                let _ = fs::fcntl_add_seals(&fd, fs::SealFlags::SHRINK | SealFlags::SEAL);
                return Ok(fd);
            }
            Err(io::Errno::INTR) => continue,
            Err(io::Errno::NOSYS) => break,
            Err(errno) => return Err(std::io::Error::from(errno)),
        }
    }

    // Fallback to using shm_open.
    let mut mem_file_handle = get_mem_file_handle();
    loop {
        let open_result = shm::open(
            mem_file_handle.as_str(),
            shm::OFlags::CREATE | shm::OFlags::EXCL | shm::OFlags::RDWR,
            fs::Mode::RUSR | fs::Mode::WUSR,
        );
        // O_CREAT = Create file if does not exist.
        // O_EXCL = Error if create and file exists.
        // O_RDWR = Open for reading and writing.
        // O_CLOEXEC = Close on successful execution.
        // S_IRUSR = Set user read permission bit .
        // S_IWUSR = Set user write permission bit.
        match open_result {
            Ok(fd) => match shm::unlink(mem_file_handle.as_str()) {
                Ok(_) => return Ok(fd),
                Err(errno) => return Err(std::io::Error::from(errno)),
            },
            Err(io::Errno::EXIST) => {
                // If a file with that handle exists then change the handle
                mem_file_handle = get_mem_file_handle();
                continue;
            }
            Err(io::Errno::INTR) => continue,
            Err(errno) => return Err(std::io::Error::from(errno)),
        }
    }
}
