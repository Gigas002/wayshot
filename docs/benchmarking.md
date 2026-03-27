# Benchmarking libwayshot

Benchmarks run in **GitHub Actions** on every push and PR. The Criterion HTML report is uploaded as the **criterion-report** artifact: open the Actions run → Artifacts → download and open `report/index.html`.

This document describes how to run the benchmarks and how to compare the **EGL** and **Vulkan** capture backends.

## Quick start

From the repo root:

```bash
# Image operations only (no Wayland needed) – runs anywhere
cargo bench -p libwayshot --features bench -- image_ops

# EGL capture (needs a Wayland session + DRI device)
cargo bench -p libwayshot --features "bench,egl" -- capture

# Vulkan capture (needs Wayland + DRI + Vulkan driver)
cargo bench -p libwayshot --features "bench,vulkan" -- capture

# Run every benchmark
cargo bench -p libwayshot --features "bench,egl,vulkan" --no-fail-fast --bench image_ops --bench capture
```

## What is benchmarked?

### `image_ops` (no display required)

- **`rotate_image_buffer`** – rotation/flip (Normal, 90°, 180°, 270°, Flipped) on a 1920×1080 buffer.
- **`convert_inplace`** – color conversion (Xbgr8888, Xrgb8888, 10-bit) on a 1920×1080 frame.

Use this to check image-processing performance without a compositor.

### `capture` (Wayland + GPU required)

- **`capture_egl`** – `capture_target_frame_eglimage`: full EGL path (DMA-BUF → EGLImage). Built with `egl` feature.
- **`capture_vulkan`** – `capture_target_frame_vk_image`: full Vulkan path (DMA-BUF → VkImage). Built with `vulkan` feature.

Each capture benchmark measures **time per frame** (roundtrip to compositor + buffer creation). Lower is better.

## Requirements for capture benchmarks

1. **Wayland session** – run inside Sway, Hyprland, River, etc. `WAYLAND_DISPLAY` must be set.
2. **DRI render node** – benchmarks open `/dev/dri/renderD128` (typical default GPU render node on Linux).
3. **Vulkan (Vulkan bench only)** – Mesa Vulkan driver or other Vulkan ICD. The bench creates a minimal Vulkan device with `VK_KHR_external_memory_fd` and `VK_EXT_external_memory_dma_buf`.

If connection or GPU setup fails, the capture benchmarks are skipped and a short message is printed.

## Comparing EGL vs Vulkan

1. Build with both backends and run the capture bench once:

    ```bash
    cargo bench -p libwayshot --features "egl,vulkan" -- capture --save-baseline egl-vs-vulkan
    ```

2. Open the Criterion report (printed at the end, e.g. `target/criterion/report/index.html`) and compare:
    - **`capture_egl/capture_target_frame_eglimage`**
    - **`capture_vulkan/capture_target_frame_vk_image`**

    The report shows mean time per iteration and confidence intervals.

3. Optional: compare two runs (e.g. before/after a change):

    ```bash
    cargo bench -p libwayshot --features "egl,vulkan" -- capture --baseline egl-vs-vulkan
    # ... make changes ...
    cargo bench -p libwayshot --features "egl,vulkan" -- capture --baseline egl-vs-vulkan
    ```

    Criterion will print a diff between the two baselines.

## Interpreting results

- **Time (ms)** – average time for one capture. Includes Wayland roundtrip and GPU buffer creation.
- **Difference** – when using baselines, positive % means slower, negative % means faster.
- Capture benchmarks use a small sample size and 10s measurement time; for stable numbers run multiple times or increase `--measurement-time`.
