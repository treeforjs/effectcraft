# Windows graphics setup

This guide targets **Windows 11 x64**, prioritizing **NVIDIA RTX PRO 4000 Blackwell** and
**RTX PRO 2000 Blackwell**, plus GeForce RTX 30/40/50-series cards and Intel integrated graphics.
The driver guidance comes from the hardware vendors. Windows execution and these physical GPUs
have not been validated in the Ubuntu compatibility test environment.

## Choose the driver for your GPU

| GPU | Driver choice |
|---|---|
| RTX PRO 4000 / 2000 Blackwell | Current NVIDIA RTX Enterprise Production Branch for the exact desktop or laptop model and Windows 11 64-bit |
| GeForce RTX 30 / 40 / 50 series | NVIDIA Studio Driver for creative workloads; Game Ready is also an option when gaming updates are the priority |
| Intel integrated graphics | The PC maker's graphics driver first; Intel Driver & Support Assistant can identify the appropriate generic package for the processor/graphics generation |

Use the [official NVIDIA driver selector](https://www.nvidia.com/en-us/drivers/), and check the
selected download's supported-products list before installation. The verified
[RTX Enterprise list](https://www.nvidia.com/en-us/drivers/details/279889/) includes both priority
workstation cards; the [Studio list](https://www.nvidia.com/en-us/drivers/details/280101/) includes
GeForce RTX 30/40/50 families. These establish coverage, not a minimum or pinned driver version.
NVIDIA explains [Studio versus Game Ready](https://www.nvidia.com/en-us/geforce/news/ces-2022-nvidia-community-qa/).

For Intel, follow its [OEM-first recommendation](https://www.intel.com/content/www/us/en/support/articles/000058295/graphics.html)
and [driver-generation / Driver & Support Assistant guidance](https://www.intel.com/content/www/us/en/support/articles/000090440/graphics.html).
Arc/Core Ultra, earlier Iris Xe/UHD generations and older integrated graphics use different
driver branches; choose the one covering the actual processor.

Install Windows updates and the selected vendor driver, then reboot before testing EffectCraft.
On a hybrid laptop, keep both the integrated and discrete GPU drivers current. Use the normal
signed Windows driver package for the exact GPU and OS.

## Check the adapter EffectCraft uses

EffectCraft normally lets wgpu select the window adapter. Its compositor and supported effects
use GPU compute, with CPU fallbacks when the device or effect is unsupported. **Help → System
Compatibility Report** reports the compositor adapter and warnings. **Help → Enable Logging**
writes `%APPDATA%\EffectCraft\Logs\EffectCraft Log.txt`; **Help → Reveal Logging File** locates it.
Startup adapter details can also be captured by launching with `RUST_LOG=info` from a terminal.
NVIDIA users can run `nvidia-smi`; Device Manager shows installed adapters and driver versions.
Run `dxdiag` and inspect the Display/Render tabs to record the GPU and driver versions. If
Windows lists [Microsoft Basic Display Adapter](https://support.microsoft.com/en-us/windows/hardware/display-graphics/microsoft-basic-display-adapter-in-windows),
install the proper vendor or OEM driver before evaluating graphics performance.

Windows also exposes a per-app preference in **Settings → System → Display → Graphics →
EffectCraft → Options** ([Microsoft instructions](https://support.microsoft.com/en-us/windows/hardware/display-graphics/optimizations-for-windowed-games-in-windows-11)).
Save changes and restart the app, then check its reported adapter: EffectCraft has its own
adapter selection, so verify the result instead of assuming the OS preference selected it.

EffectCraft uses wgpu with Direct3D 12/Vulkan for GPU computation and window presentation.
**Preferences → Startup & Repair → Window Graphics** can choose OpenGL for the window, with
CPU compositing. A failed startup automatically tries that fallback on the next launch;
`WGPU_BACKEND` overrides the preference. See [startup settings](preferences.md).

The CLI is CPU-rendered by default; `--gpu` requests GPU compositing. It does not enable
NVIDIA NVENC/NVDEC or Intel Quick Sync codec acceleration: the media codecs in this checkout
are software implementations. CUDA and the Vulkan SDK are not installation requirements.

If the window is black or fails to start after installing the current driver, try the OpenGL
backend from PowerShell. This example uses the default x64 installation location; adjust it
for a portable or custom installation:

```powershell
$env:WGPU_BACKEND = "gl"
& "$env:ProgramFiles\EffectCraft\effectcraft.exe"
Remove-Item Env:WGPU_BACKEND
```

The override is local to that PowerShell session. Remove it after the diagnostic run so the
next launch uses normal backend selection. Keep the startup log when reporting a driver issue.

For the same hardware on Linux, see the [Ubuntu graphics guide](ubuntu.md#graphics-hardware).
