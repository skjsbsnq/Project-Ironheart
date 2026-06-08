# D3D11 Trace Proxy

Proxy DLLs for extracting vanilla HOI4 D3D11 shader bindings, runtime targets,
and pass graph state.

Build with MinGW g++:

```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1
```

Build outputs:

```text
tools/d3d11_trace_proxy/d3d11.dll
tools/d3d11_trace_proxy/dxgi.dll
tools/d3d11_trace_proxy/d3dcompiler_47.dll
tools/d3d11_trace_proxy/d3dx9_43.dll
```

Shader extraction hooks:

```text
D3D11CreateDevice
ID3D11Device::CreateVertexShader
ID3D11Device::CreatePixelShader
D3DCompile
D3DXCompileShader
```

Runtime target/pass graph hooks:

```text
ID3D11Device::CreateTexture2D
ID3D11Device::CreateShaderResourceView
ID3D11Device::CreateRenderTargetView
ID3D11Device::CreateDepthStencilView
ID3D11Device::GetImmediateContext
ID3D11DeviceContext::VSSetShader
ID3D11DeviceContext::PSSetShader
ID3D11DeviceContext::PSSetShaderResources
ID3D11DeviceContext::OMSetRenderTargets
ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews
ID3D11DeviceContext::Draw
ID3D11DeviceContext::DrawIndexed
ID3D11DeviceContext::DrawInstanced
ID3D11DeviceContext::DrawIndexedInstanced
ID3D11DeviceContext::DrawAuto
ID3D11DeviceContext::DrawIndexedInstancedIndirect
ID3D11DeviceContext::DrawInstancedIndirect
IDXGIFactory::CreateSwapChain
IDXGIFactory2::CreateSwapChainForHwnd
IDXGISwapChain::Present
```

The `d3d11.dll` proxy forwards `D3D11CreateDevice` to the system DLL, then
patches the returned `ID3D11Device` and immediate context vtable slots. The
`dxgi.dll` proxy forwards factory creation, patches swapchain creation, and
records `Present` frames.

Run by copying the DLLs next to `hoi4.exe`, or use the capture helper. The
helper deploys the proxy DLLs, launches HOI4, then restores the game directory:

```powershell
powershell -ExecutionPolicy Bypass -File .\capture_hoi4_shaders.ps1 -TimeoutSeconds 120
```

Set this environment variable before launching HOI4 if the current working
directory is not this repository root:

```powershell
$env:HOI4_TRACE_DIR = "C:\Users\19180\Documents\999\b1\tools\vanilla_trace"
```

Trace outputs:

```text
tools/vanilla_trace/shader_compiles.json
tools/vanilla_trace/shader_bindings.json
tools/vanilla_trace/logs/d3d11_trace.jsonl
tools/vanilla_trace/runtime_targets.json
tools/vanilla_trace/render_passes.json
```

Summarize the raw JSONL log after capture:

```powershell
powershell -ExecutionPolicy Bypass -File ..\vanilla_trace\summarize_d3d11_trace.ps1 -TraceDir ..\vanilla_trace
```

Known limitation: in the current HOI4 capture, explicit draw hooks only caught
early startup `Draw` calls. The summarized pass graph therefore also includes a
state-flow reconstruction based on render target, shader, and SRV state changes.

Do not commit captured vanilla shader bytecode, textures, binaries, or other
private game payloads.
