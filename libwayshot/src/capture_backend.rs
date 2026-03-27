//! How the compositor frame is received: shared-memory (default) or GPU import (EGL / Vulkan) over DMA-BUF.

/// Buffer copy path for screenshots. `Shm` is the traditional wl-shm / ext-image-copy path.
/// `Egl` / `Vulkan` use DMA-BUF capture, run the corresponding import, then read pixels from the linear GBM buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaptureBufferBackend {
    #[default]
    Shm,
    #[cfg(feature = "egl")]
    Egl,
    #[cfg(feature = "vulkan")]
    Vulkan,
}
