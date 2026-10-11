# Ubuntu 24.04 / 26.04 LTS

These instructions target x86_64 (Intel/AMD 64-bit) Ubuntu Desktop. EffectCraft supports
native Wayland and X11; Ubuntu's normal desktop session supplies the display server and GPU
drivers. A server installation can use `effectcraft-cli` without a desktop.

## Install a package

Download the x86_64 `.deb` from the release you want, check it against that release's
`SHA256SUMS.txt`, then install the local file with APT so its dependencies are installed too:

```sh
sudo apt update
sudo apt install ./effectcraft-<ver>-linux-x86_64.deb
effectcraft --version
effectcraft
```

Replace `<ver>` with the downloaded version. The package installs both `effectcraft` and
`effectcraft-cli`, an application-menu launcher, icons and file associations for `.ecproj` and `.ecprojx` projects. Right-click a project and choose
**Open With EffectCraft**.

Packages built on Ubuntu 24.04 also run on 26.04. A package built on 26.04 may require a newer
glibc and must not be assumed to run on 24.04. Official release builds retain their Ubuntu 22.04
baseline (glibc 2.35); build your own distributable on the oldest Ubuntu you intend to support.

For an AppImage, make it executable and run it. If FUSE mounting is unavailable, use
`APPIMAGE_EXTRACT_AND_RUN=1 ./effectcraft-<ver>-linux-x86_64.AppImage`; this requires no FUSE
installation. The `.deb` is the simplest choice for application-menu and file associations.

## Build this checkout

Install Rust with [rustup](https://rustup.rs/). The workspace requires Rust **1.95 or newer**;
the `rustc` package in an older Ubuntu release may be too old. Then, from this checkout:

```sh
sudo apt update
sudo apt install --no-install-recommends build-essential pkg-config cmake git curl \
  libasound2-dev libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libx11-dev libxrandr-dev libxi-dev \
  libgl-dev libegl-dev libegl1 libegl-mesa0 libgl1-mesa-dri libvulkan1 mesa-vulkan-drivers \
  xdg-desktop-portal xdg-desktop-portal-gtk fonts-dejavu-core
rustup update stable
cargo build --release --locked -p effectcraft -p effectcraft-cli
./target/release/effectcraft
```

On a machine with little RAM, add `CARGO_BUILD_JOBS=2` before the Cargo command. Builds can use
several gigabytes of disk. The desktop uses CPAL/ALSA for audio; Ubuntu 24.04/26.04 supply the runtime as
`libasound2t64`. Media codecs are pure Rust through the pinned FilmCraft crates. FFmpeg is an
optional external test oracle, not an installed app dependency.

To make an installable package, install the pinned nFPM version used in
[release.yml](../.github/workflows/release.yml) from [nFPM](https://nfpm.goreleaser.com/install/),
then run:

```sh
packaging/linux/package.sh --skip-build --formats "deb tar"
sudo apt install ./dist/release/effectcraft-*-linux-x86_64.deb
```

`CARGO_TARGET_DIR` is supported by both Cargo and the packaging script. The tarball contains
`bin/` and `share/`; it can be unpacked anywhere and run as `bin/effectcraft`.

## Graphics hardware

The priority workstation targets are **NVIDIA RTX PRO 4000 Blackwell** and
**RTX PRO 2000 Blackwell**. GeForce RTX 30/40/50-series cards and Intel integrated graphics
are also targets. These are driver/setup targets; the automated checks use Mesa software
rendering and do not certify these physical GPUs.

| Hardware | Ubuntu graphics stack |
|---|---|
| RTX PRO 4000 / 2000 Blackwell, GeForce RTX 50 series | Ubuntu's recommended NVIDIA desktop driver, with the **open kernel modules** required by Blackwell |
| GeForce RTX 30 / 40 series | Ubuntu's recommended NVIDIA desktop driver |
| Intel integrated graphics | Ubuntu's kernel, Mesa Vulkan/OpenGL drivers and `linux-firmware` |

For NVIDIA, install available Ubuntu updates, then let Ubuntu select a driver for the actual
GPU instead of pinning a driver branch:

```sh
sudo apt update
sudo apt upgrade
sudo apt install ubuntu-drivers-common
ubuntu-drivers list
sudo ubuntu-drivers install
sudo reboot
```

For Blackwell, the selected driver package must use the `-open` kernel-module flavor. After
rebooting, check `nvidia-smi`; `modinfo -F license nvidia` should report `Dual MIT/GPL` for the
open modules. If Secure Boot requests key enrollment during installation, complete it at reboot.
The [NVIDIA supported-GPU list](https://download.nvidia.com/XFree86/Linux-x86_64/595.84/README/supportedchips.html)
includes both priority workstation cards;
[NVIDIA's kernel-module documentation](https://download.nvidia.com/XFree86/Linux-x86_64/580.95.05/README/kernel_open.html)
explains the Blackwell requirement. See also
[Ubuntu's desktop driver instructions](https://ubuntu.com/desktop/docs/en/latest/how-to/graphics/install-nvidia-drivers/).

For Intel, use Ubuntu's maintained kernel and Mesa packages:

```sh
sudo apt update
sudo apt upgrade
sudo apt install libvulkan1 mesa-vulkan-drivers libegl1 libegl-mesa0 libgl1 libgl1-mesa-dri linux-firmware
```

On Ubuntu 24.04 with a recent Intel integrated GPU, install the current
[HWE kernel](https://ubuntu.com/kernel/docs/reference/hwe-kernels/) and reboot:
`sudo apt install --install-recommends linux-generic-hwe-24.04`. Ubuntu 26.04 supplies a newer
kernel and graphics stack directly. See [Mesa's supported systems](https://docs.mesa3d.org/systems.html)
and its [Intel runtime dependencies](https://docs.mesa3d.org/drivers/anv.html#runtime-dependencies),
plus [Intel's Ubuntu compatibility guidance](https://dgpu-docs.intel.com/installation-guides/installing-packages-from-the-intel-ppa.html).

EffectCraft uses wgpu for window presentation and GPU compositing/effects (Vulkan on Linux;
OpenGL is a window fallback with CPU compositing). CPU rendering remains available when the
compute adapter cannot meet the required capabilities. See **Help → System Compatibility
Report** for the compositor adapter and warnings; `RUST_LOG=info effectcraft` also reports the
window adapter during startup. On hybrid systems, verify the selected adapter after changing
OS graphics preferences.

The CLI renders on the CPU by default; `effectcraft-cli render-frame --project project.ecproj
--gpu --out frame.png` requests the GPU compositor. This does **not** enable NVIDIA NVENC/NVDEC,
Intel Quick Sync or VA-API codec acceleration: this checkout uses pure-Rust software codecs.
A Vulkan loader and vendor/Mesa driver are sufficient; CUDA and the Vulkan SDK are not required.

## Desktop troubleshooting

- **File dialogs do not open:** run inside your logged-in desktop session, with its D-Bus
  session and `xdg-desktop-portal` service. The GTK portal backend above supplies the chooser
  on Ubuntu GNOME. KDE users should use their desktop's `xdg-desktop-portal-kde` backend.
- **File drag-and-drop fails on Wayland:** the current winit version does not support these
  drops. Use File → Import, or run `env -u WAYLAND_DISPLAY effectcraft` through XWayland.
- **No audio device:** use the normal logged-in PipeWire/PulseAudio/ALSA desktop session and
  select the output under Preferences → Audio. Headless checks do not verify audio playback.
- **A graphics driver fails:** try `WGPU_BACKEND=gl effectcraft`. On Mesa, the software fallback
  `LIBGL_ALWAYS_SOFTWARE=1 WGPU_BACKEND=gl effectcraft` can help diagnose GPU problems.
- **No window on SSH or Ubuntu Server:** the GUI needs a display session. Use
  `effectcraft-cli render-frame --project project.ecproj --out frame.png` for headless rendering.

For startup diagnostics, run `RUST_LOG=info effectcraft` from a terminal. **Help → Enable
Logging** writes `Logs/EffectCraft Log.txt` beneath `~/.config/effectcraft` (or the configured
`$XDG_CONFIG_HOME/effectcraft`); **Help → Reveal Logging File** opens its location. The Startup
& Repair preference can also select OpenGL, and a launch that never draws a frame makes the
next launch try that fallback. `WGPU_BACKEND` overrides this preference.

## Automated Ubuntu checks

The [Ubuntu workflow](../.github/workflows/ubuntu.yml) builds and installs a `.deb` in Ubuntu
24.04 and 26.04 x86_64 containers. It checks launcher/MIME registration and resource permissions,
creates a small composition, renders PNG and a short H.264 MP4 through the CLI, imports and
decodes that MP4, then drives a
real window under Xvfb/Mesa: edit, save, reopen and screenshot.

Run the same check in a desktop session with `packaging/linux/smoke.sh`, or headlessly with
`xvfb-run -a dbus-run-session -- packaging/linux/smoke.sh`. The headless form needs `jq`, `xvfb`,
`xauth` and `dbus-x11`. Use a fresh `SMOKE_OUT` directory for each run.

On 11 October 2026, the locally built `.deb` passed installed checks on Ubuntu 24.04 and 26.04
as a different, unprivileged user: all four X11/Xvfb and native Wayland/Weston runs passed with
the default Vulkan llvmpipe adapter. Each run exercised PNG rendering, a three-frame H.264
encode/import/decode, a desktop edit, save/reopen and a screenshot. Launcher/MIME registration,
linked libraries and public resource permissions also passed. The validation build used
release optimization with LTO disabled and 16 codegen units; its measured glibc floor was 2.39.
The tarball's permissions were checked after packaging under a private `umask 077`.

Software-rendered checks do not certify the named physical GPUs, native GNOME/KDE desktop
integration, audio playback, tablet input, clipboard interoperability or interactive portal dialogs.
