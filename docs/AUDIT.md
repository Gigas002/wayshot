# Audit

A whole-application audit of wayshot, libwayshot and waymirror-egl
(2026-11-18) in five parts: bugs (§2), performance (§3), security and
robustness (§4), usability (§5), and code quality, tests and project hygiene
(§6). §7 lists what was left open, and §8 is the checklist to re-run the
audit.

The fixes are the 15 commits of branch `experimental/audit` on top of d5d5d27
(`git log --reverse --stat d5d5d27..experimental/audit`), one per finding,
followed by this document. The last fix (782ecb1, PNG level 3 by default) is a
proposal: it changes the size/speed trade-off of every default screenshot, so
it is separate and can be dropped. No version or changelog was touched.

Each finding says what was found, how serious it is, and what was done:
**fixed** (with the commit), **accepted** (with the reason), or **OK**
(checked, nothing to do). Every fix is its own commit, with a regression test
where one can be written.

Severity: **high** (a wrong or missing screenshot, or a resource leak that
stops a program, in normal use), **medium** (wrong behaviour in a reachable
case, an ignored setting), **low** (hardening, polish, or needs an unusual
setup).

## 1. Method

Every source file of the three crates was read in full. Findings were then
reproduced with release builds in a private headless sway 1.12
(`WLR_BACKENDS=headless`, its own runtime directory and its own D-Bus session
bus), so the desktop's compositor, clipboard and notification daemon were never
used. sway 1.12 offers both ext-image-copy-capture (with the toplevel source)
and wlr-screencopy; libwayshot picks the former.

- **Scenes:** a 3840x2160 photo (imgvwr, full screen) and a screen of source
  text (alacritty, full screen), at 1920x1080 and 3840x2160.
- **Layouts:** rotated outputs (90° at scale 2), mixed scales (1 and 2 side by
  side), outputs offset so the layout isn't a rectangle.
- **Pixels:** captures compared with PIL against the baseline binary, against
  `-o` captures, and against crops of full captures.
- **Protocol objects** counted from `WAYLAND_DEBUG=1` (created vs destroyed).
- **File descriptors** of waymirror-egl from `/proc/<pid>/fd`, with sway on the
  gles2 renderer (AMD GPU; the NVIDIA render node fails GBM allocation in
  headless sway with EINVAL, a setup limit, not a wayshot bug).
- **Notifications** read off the private bus with `dbus-monitor`.
- **Timing:** wall time, CPU time and peak RSS from `wait4`, median of 7–15
  runs after two warm-ups.

Builds checked: default, `--no-default-features`, `--all-features`. Static
checks: `cargo clippy` at the three levels, `cargo deny check`, `typos`,
`cargo doc` with `-D warnings`, `cargo llvm-cov`.

## 2. Bugs

| Area | Finding | Status |
| ---- | ------- | ------ |
| Rotated HiDPI outputs | `OutputInfo::scale` divided the mode's height by the logical height. The mode is never rotated and the logical size is, so a portrait monitor at scale 2 got scale 1.125: `wayshot` and `-G` made a 607x1080 image of a 1080x1920 screen, cut to its top-left part. Now 1080x1920 and pixel-identical to `-o`. | Fixed, high (6a23d0b) |
| Partial `[base]` | `resolve` read a missing `[base] file` as false (the default is true): a config with only `cursor = true` made wayshot exit 0 without saving anything. | Fixed, high (73e532e) |
| Row stride ignored | Frames were read as packed `width × bpp` rows. A compositor may pad rows (its choice in wlr-screencopy); the image was then sheared and the loop ran out of bounds: the "Image index (0, 1080) out of bounds (1920, 1080)" of upstream #458 (Sway + NVIDIA), which #459 turned into an error without fixing the image. Rows are now read by stride; a test covers all eight transforms with and without padding. Not reproduced on NVIDIA hardware. | Fixed, medium (ee47585) |
| Regions on ext-image-copy | With ext-image-copy, a region capture took every output whole and composited them, and the scale was the largest of all outputs: `-G "100,100 400x300"` on a scale-1 monitor next to a scale-2 one gave 800x600, upscaled. Without the toplevel source the region was ignored outright while the frame was placed as if it weren't. Now only the outputs the region touches are captured, and their frames are cut to it. Pixel-identical to crops of full captures on one output, at scale 2 and rotated; see §7 for the upscaled half of a mixed-scale region. | Fixed, medium (fdf7d20) |
| Leaked protocol objects | wayland-client doesn't destroy a proxy on drop, and nothing destroyed the ext source, session and frame, the wlr frame, the dmabuf params, or the frames made only to read formats. 20 `screenshot_all` calls left 20 of each in the compositor; a 100-frame screencast left 102 frames. A failed copy also left its buffer and pool. Now 20/20 and 102/102. The session has to outlive the copy: wlroots stops a frame whose session is destroyed. | Fixed, medium (a0b322a) |
| EGL dma-buf fd leak | `create_egl_image_from_dmabuf` gave EGL a raw fd and never closed it (EGL doesn't take ownership). waymirror-egl went from 68 to 381 fds in 6 s (about 62 a second); at the default limit of 1024 it stopped getting frames after about 15 s and kept showing the last one, as its errors are ignored. Now steady at 9 fds while mirroring at 61 fps. | Fixed, high (f4895a8) |
| Bad `--file-name-format` | chrono panicked on an unknown directive (`%K`: "a formatting trait implementation returned an error"). Now an error naming the format, exit 1. A format producing a `/` failed with a bare "No such file or directory"; the error now names the file. | Fixed, medium (ed1a7ee) |
| `.ppm` files were PAM | image writes RGBA as PAM (P7), so every `.ppm` was a PAM file that PIL and other PPM readers refuse. Now a binary PPM (P6), pixel-identical to `grim -t ppm`. | Fixed, medium (dbc036e) |
| Silent config errors | A file with a TOML error, a misspelt key (`cursr`, `[bsae]`), an unknown value (`compression = "fastest"`, `filter = "paethh"`, `log_level = "verbose"`, a level above 9) or a missing `--config` file all fell back to the defaults without a word. These are now errors naming the file and setting, exit 1; a missing default config still means the defaults. | Fixed, medium (73e532e) |
| `[base] geometry_*_color` | Parsed, undocumented, never used. They now apply when `[geometry]` doesn't set the color. | Fixed, low (73e532e) |
| `--config` help | Named `$HOME/wayshot/config.toml`, which is never read; now `$HOME/.config/...` like the man page. | Fixed, low (73e532e) |
| Notification text | Said "saved" when nothing was (`--clipboard` with `file = false`, or `-`). Now "saved to <name>", "copied to the clipboard" or "taken". | Fixed, low (eca4c0d) |
| wlr capture hang | A wlr-screencopy capture that got `failed` before `buffer_done` (output unplugged, capture refused) dispatched forever. Now `FramecopyFailed`. From reading the code: sway uses the ext path. | Fixed, low (a0b322a) |
| Worker panics | Composite workers were joined with `flat_map`, which dropped a panicked thread's result and left a transparent hole. A panic is now re-raised. | Fixed, low (975eec0) |
| Transforms, multi-output | All eight transforms map pixels as before (unit test against the old mapping); two outputs side by side, `-o`, and layouts with gaps are pixel-identical to the baseline. | OK |

## 3. Performance

All numbers come from the private headless sway on this machine, using the
pixman renderer for captures, the release build and default settings. Time is
the median of 7–15 runs, and RSS is peak RSS.

**Before/after, photo on screen** (`suite.sh`, base = d5d5d27, after = the
whole branch including the proposal 782ecb1):

| Capture | 1080p before | 1080p after | 4K before | 4K after |
| ------- | ------------ | ----------- | --------- | -------- |
| `wayshot s.png` | 892 ms, 27 MB, 3.39 MB file | 184 ms, 31 MB, 3.05 MB | 3269 ms, 73 MB, 11.45 MB | 614 ms, 89 MB, 10.36 MB |
| `s.qoi` | 60 ms | 41 ms | 180 ms | 109 ms |
| `s.ppm` | 49 ms, 8.3 MB file | 32 ms, 6.2 MB | 133 ms | 66 ms |
| `s.jpg` | 73 ms | 55 ms | 225 ms | 155 ms |
| `-G 400x300` png | 88 ms, 25 MB | 33 ms, 20 MB | 134 ms, 73 MB | 48 ms, 43 MB |
| `-G 400x300` qoi | 36 ms | 24 ms | 83 ms | 39 ms |

**Text on screen, PNG** (most screenshots look like this):

| | 1080p | 4K |
| - | ----- | -- |
| before | 78 ms, 119 KB | 230 ms, 280 KB |
| after | 49 ms, 119 KB | 127 ms, 277 KB |
| `grim` (for reference) | 43 ms, 110 KB | 108 ms, 255 KB |

On the photo, `grim` takes 201 ms (1080p) and 973 ms (4K) and writes 2.94 MB
and 9.95 MB.

Where the time went, and what each change did:

| Area | Finding | Status |
| ---- | ------- | ------ |
| Building the image | A per-pixel loop computed a position and did a checked `put` for each pixel: 43 ms of a 113 ms 4K `screenshot_all`. Unrotated frames now copy whole rows: 113 → 77 ms. | Fixed (ee47585) |
| Compositing one frame | Even a single output was copied onto a new transparent canvas (34 ms at 4K). A frame that is the whole result is now returned as it is: 77 → 41 ms. | Fixed (975eec0) |
| Region captures | Captured and composited every output whole (above). Now only the region's outputs, and only the region's rows are copied: 4K `-G` to qoi 83 → 39 ms, RSS 73 → 43 MB. | Fixed (fdf7d20) |
| PNG alpha | Opaque captures were encoded as RGBA. Dropping a constant alpha: 1080p 885 → 638 ms, 4K 3229 → 2590 ms, files 13 % smaller, pixels identical. RSS +5–12 MB for the RGB copy. QOI was tried too: same size, slightly slower, so left as is. | Fixed (708a3bc) |
| PNG level (proposal) | The default is image's balanced level (zlib 6). See the table below; level 3 is the proposal. | Proposed (782ecb1) |
| Screencast | 62 fps (the output's refresh rate) before and after; 16.1 ms a frame. | OK |
| Color picker | Without freeze it captures a 1x1 region, so the region fix applies. | OK |

**PNG compression levels** (binary, RGB, time / size):

| | balanced (6, current) | level 3 (proposed) | level 1 | `"fast"` |
| - | --------------------- | ------------------ | ------- | -------- |
| 1080p photo | 627 ms / 2.95 MB | 179 ms / 3.05 MB | 77 ms / 3.14 MB | 38 ms / 2.86 MB |
| 1080p text | 52 ms / 115 KB | 45 ms / 119 KB | 30 ms / 152 KB | 27 ms / 266 KB |
| 4K photo | 2551 ms / 10.0 MB | 598 ms / 10.36 MB | 252 ms / 11.22 MB | 104 ms / 9.92 MB |
| 4K text | 144 ms / 268 KB | 125 ms / 277 KB | 63 ms / 391 KB | 56 ms / 617 KB |

Level 3 is 3.5–4.3× faster on photos for about 3 % more bytes, and also adds
3 % on text. Level 1 and `"fast"` are faster again, but text screenshots
(the common case) grow by 32–130 %. Without 782ecb1, a 1080p photo takes
638 ms instead of 184 ms.

## 4. Security and robustness

| Area | Finding | Status |
| ---- | ------- | ------ |
| "Open Folder" action | The default ran `sh -c "xdg-open {dir}"`: a folder with a space opened the wrong path, and `$(...)` in a folder name ran as shell code. `xdg-open` now gets the folder as one argument. A configured `action` is still a shell command, as documented; it comes from the user's own config. | Fixed, medium (eca4c0d) |
| GBM fd unwrap | `capture_target_frame_dmabuf` unwrapped `fd_for_plane`; a GBM error panicked inside the library. Now an error. | Fixed, low (f4895a8) |
| Dependencies | `cargo deny check` failed. The lockfile update moved notify-rust to zbus 6, which removed the unmaintained zvariant/zbus_names crates (RUSTSEC-2026-0370 and related). chrono (unmaintained) was replaced by jiff. `paste` (RUSTSEC-2024-0436, unmaintained) remains through image → ravif → rav1e for AVIF, with no fix upstream, and is ignored in `deny.toml` with the reason. deny now passes. | Fixed (dc670b8, ed1a7ee, 00e3f2a) |
| Shared memory | Capture buffers are memfds, mapped by wayshot and the compositor only. | OK |
| Clipboard | A forked child serves the clipboard with stdio on `/dev/null` and exits when the selection is replaced. Not exercised live (see §7). | Accepted |
| Notification child | A forked child waits for the notification action. With no notification daemon, the parent still returns at once (integration test). | OK |

## 5. Usability

| Area | Finding | Status |
| ---- | ------- | ------ |
| Config errors | Now name the file, line, setting and valid values (TOML errors list the expected keys). `man 5 wayshot` says unknown keys and values are errors. | Fixed, docs (73e532e) |
| strftime reference | The man pages and example config linked chrono's table; now jiff's. Common directives are the same; an unknown one is an error. | Fixed, docs (e0d6793) |
| PPM without alpha | PPM can't hold alpha, so transparent gaps of a non-rectangular layout now come out black in `.ppm` files. Use PNG or QOI to keep them. | Accepted |

## 6. Code quality, tests and hygiene

| Area | Finding | Status |
| ---- | ------- | ------ |
| Tests | 354 → 368, all passing in the private sway at `--all-features`. New tests cover rotated scale, stride and transforms (with crops), region-to-output selection and cropping, bad file-name formats, config errors and the shipped example config, the partial `[base]`, notification text and the folder command, alpha dropping and PPM output. The new and changed tests passed 24 parallel runs. | Fixed |
| Untestable in-process | Protocol object and fd leaks can't be observed from inside the process; they were counted with `WAYLAND_DEBUG` and `/proc` instead (§2). | Accepted |
| Coverage | 57.8 % → 58.9 % of lines, 68.2 % → 69.6 % of functions (`cargo llvm-cov --workspace --all-features`, in headless sway). | Fixed |
| libwayshot API | Additive only. `WayshotFrame::destroy()` is new, for callers that read formats from `capture_target_frame_get_state` without copying. `CaptureFrameState` and `FrameCopy` gained private fields; both already had private fields and can't be built outside the crate. | OK |
| Gates | fmt, typos, clippy at the three feature levels, tests, deny and docs pass on every commit of the branch. | OK |

## 7. Left open

- **Untested paths.** These weren't exercised:
  - freeze and the interactive selectors (`-g`, `--choose-output`, `--choose-toplevel`, the color picker's point selection), which need input;
  - the clipboard, live;
  - the wlr-screencopy path, since sway picks ext-image-copy; the wlr changes (frame destroy, early `failed`) come from reading the code;
  - the real NVIDIA stride case of #458.
- **Mixed-scale regions.** A region across a scale-1 and a scale-2 output now
  upscales only the region's part of the scale-1 output. The resize filter
  then sees different edges, and pixels differ from the baseline by at most
  3/255 at the crop border. The scale-2 half is identical.
- **RSS of full PNG/PPM captures** grows by the RGB copy (+5 MB at 1080p,
  +16 MB at 4K). Encoding straight from the RGBA buffer with a stripping writer
  would avoid it.
- **PNG default (782ecb1)** is your call; the numbers are in §3.
- **Versions and changelog** weren't touched. If the branch is taken,
  libwayshot gains an API (`WayshotFrame::destroy`), and wayshot changes
  behaviour: config errors are now fatal, `.ppm` files are PPM.
- **`paste` ignore** in `deny.toml` should go once rav1e drops `paste`.

## 8. Re-running the audit

```sh
# Gates (as in CI); the live tests need a compositor: run them in a private
# headless sway, never the desktop's (they capture the screen and freeze it)
cargo fmt --all -- --check && typos && cargo deny check
for f in --no-default-features "" --all-features; do
  cargo clippy --workspace --all-targets $f -- -D warnings
done
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
export XDG_RUNTIME_DIR=$(mktemp -d) WLR_BACKENDS=headless WLR_RENDERER=pixman
unset WAYLAND_DISPLAY DISPLAY HYPRLAND_INSTANCE_SIGNATURE
dbus-run-session -- sh -c 'sway -c /dev/null & sleep 1;
  WAYLAND_DISPLAY=wayland-1 cargo test --workspace --all-features'

# Coverage (system LLVM tools on Arch), inside the same session
LLVM_COV=/usr/bin/llvm-cov LLVM_PROFDATA=/usr/bin/llvm-profdata \
  cargo llvm-cov --workspace --all-features --summary-only

# Protocol objects: created vs destroyed per interface
WAYLAND_DEBUG=1 wayshot --silent 2> log
for i in ext_image_copy_capture_frame_v1 ext_image_copy_capture_session_v1 wl_buffer; do
  echo "$i $(grep -c "new id $i#" log) $(grep -cE " -> $i#[0-9]+\.destroy\(\)" log)"
done
```

Geometry checks, each in that session: `swaymsg 'output HEADLESS-1 scale 2
transform 90'` and `wayshot` (expect 1080x1920); then `swaymsg create_output`
with HEADLESS-2 at scale 2 beside HEADLESS-1, and `wayshot -G "100,100
400x300"` on HEADLESS-1 (expect 400x300).
