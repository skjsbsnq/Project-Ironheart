#include "trace_common.h"

#include <d3dx9.h>

namespace {

FARPROC proc(const char *name) {
    return trace::get_system_proc("d3dx9_43.dll", name);
}

template <typename T>
T proc_as(const char *name) {
    return reinterpret_cast<T>(proc(name));
}

} // namespace

extern "C" __declspec(dllexport) HRESULT WINAPI D3DXCompileShader(
    const char *src_data,
    UINT data_len,
    const D3DXMACRO *defines,
    ID3DXInclude *include,
    const char *function_name,
    const char *profile,
    DWORD flags,
    ID3DXBuffer **shader,
    ID3DXBuffer **error_messages,
    ID3DXConstantTable **constant_table) {
    using Fn = HRESULT(WINAPI *)(const char *, UINT, const D3DXMACRO *, ID3DXInclude *, const char *, const char *, DWORD, ID3DXBuffer **, ID3DXBuffer **, ID3DXConstantTable **);
    auto fn = proc_as<Fn>("D3DXCompileShader");
    if (!fn) {
        return E_FAIL;
    }

    HRESULT hr = fn(src_data, data_len, defines, include, function_name, profile, flags, shader, error_messages, constant_table);
    SIZE_T shader_size = 0;
    std::string shader_hash;
    if (SUCCEEDED(hr) && shader && *shader) {
        shader_size = (*shader)->GetBufferSize();
        shader_hash = trace::hash_bytes((*shader)->GetBufferPointer(), shader_size);
    }

    std::string event = "{";
    event += "\"event\":\"D3DXCompileShader\",";
    event += "\"hr\":\"" + trace::hr_hex(hr) + "\",";
    event += "\"entrypoint\":\"" + trace::json_escape(function_name ? function_name : "") + "\",";
    event += "\"target\":\"" + trace::json_escape(profile ? profile : "") + "\",";
    event += "\"source_hash\":\"" + trace::hash_bytes(src_data, data_len) + "\",";
    event += "\"source_size\":" + std::to_string(data_len) + ",";
    event += "\"shader_hash\":\"" + shader_hash + "\",";
    event += "\"shader_size\":" + std::to_string(static_cast<unsigned long long>(shader_size)) + ",";
    event += "\"flags\":" + std::to_string(flags);
    event += "}";
    trace::append_json_object(trace::trace_file_path(L"shader_compiles.json"), event);

    if (SUCCEEDED(hr) && shader && *shader) {
        trace::record_shader_bytecode(
            "D3DXCompileShader",
            "",
            (*shader)->GetBufferPointer(),
            shader_size,
            "",
            function_name ? function_name : "",
            profile ? profile : "");
    }

    return hr;
}

extern "C" __declspec(dllexport) HRESULT WINAPI D3DXLoadSurfaceFromSurface(
    IDirect3DSurface9 *destsurface,
    const PALETTEENTRY *destpalette,
    const RECT *destrect,
    IDirect3DSurface9 *srcsurface,
    const PALETTEENTRY *srcpalette,
    const RECT *srcrect,
    DWORD filter,
    D3DCOLOR colorkey) {
    using Fn = HRESULT(WINAPI *)(IDirect3DSurface9 *, const PALETTEENTRY *, const RECT *, IDirect3DSurface9 *, const PALETTEENTRY *, const RECT *, DWORD, D3DCOLOR);
    auto fn = proc_as<Fn>("D3DXLoadSurfaceFromSurface");
    return fn ? fn(destsurface, destpalette, destrect, srcsurface, srcpalette, srcrect, filter, colorkey) : E_FAIL;
}

extern "C" __declspec(dllexport) HRESULT WINAPI D3DXCreateLine(IDirect3DDevice9 *device, ID3DXLine **line) {
    using Fn = HRESULT(WINAPI *)(IDirect3DDevice9 *, ID3DXLine **);
    auto fn = proc_as<Fn>("D3DXCreateLine");
    return fn ? fn(device, line) : E_FAIL;
}

extern "C" __declspec(dllexport) HRESULT WINAPI D3DXSaveTextureToFileInMemory(
    ID3DXBuffer **destbuffer,
    D3DXIMAGE_FILEFORMAT destformat,
    IDirect3DBaseTexture9 *srctexture,
    const PALETTEENTRY *srcpalette) {
    using Fn = HRESULT(WINAPI *)(ID3DXBuffer **, D3DXIMAGE_FILEFORMAT, IDirect3DBaseTexture9 *, const PALETTEENTRY *);
    auto fn = proc_as<Fn>("D3DXSaveTextureToFileInMemory");
    return fn ? fn(destbuffer, destformat, srctexture, srcpalette) : E_FAIL;
}

extern "C" __declspec(dllexport) HRESULT WINAPI D3DXCreateCubeTexture(
    IDirect3DDevice9 *device,
    UINT size,
    UINT miplevels,
    DWORD usage,
    D3DFORMAT format,
    D3DPOOL pool,
    IDirect3DCubeTexture9 **cube) {
    using Fn = HRESULT(WINAPI *)(IDirect3DDevice9 *, UINT, UINT, DWORD, D3DFORMAT, D3DPOOL, IDirect3DCubeTexture9 **);
    auto fn = proc_as<Fn>("D3DXCreateCubeTexture");
    return fn ? fn(device, size, miplevels, usage, format, pool, cube) : E_FAIL;
}

extern "C" __declspec(dllexport) HRESULT WINAPI D3DXCreateTexture(
    IDirect3DDevice9 *device,
    UINT width,
    UINT height,
    UINT miplevels,
    DWORD usage,
    D3DFORMAT format,
    D3DPOOL pool,
    IDirect3DTexture9 **texture) {
    using Fn = HRESULT(WINAPI *)(IDirect3DDevice9 *, UINT, UINT, UINT, DWORD, D3DFORMAT, D3DPOOL, IDirect3DTexture9 **);
    auto fn = proc_as<Fn>("D3DXCreateTexture");
    return fn ? fn(device, width, height, miplevels, usage, format, pool, texture) : E_FAIL;
}

extern "C" __declspec(dllexport) HRESULT WINAPI D3DXSaveSurfaceToFileInMemory(
    ID3DXBuffer **destbuffer,
    D3DXIMAGE_FILEFORMAT destformat,
    IDirect3DSurface9 *srcsurface,
    const PALETTEENTRY *srcpalette,
    const RECT *srcrect) {
    using Fn = HRESULT(WINAPI *)(ID3DXBuffer **, D3DXIMAGE_FILEFORMAT, IDirect3DSurface9 *, const PALETTEENTRY *, const RECT *);
    auto fn = proc_as<Fn>("D3DXSaveSurfaceToFileInMemory");
    return fn ? fn(destbuffer, destformat, srcsurface, srcpalette, srcrect) : E_FAIL;
}

extern "C" __declspec(dllexport) HRESULT WINAPI D3DXLoadSurfaceFromMemory(
    IDirect3DSurface9 *dst_surface,
    const PALETTEENTRY *dst_palette,
    const RECT *dst_rect,
    const void *src_memory,
    D3DFORMAT src_format,
    UINT src_pitch,
    const PALETTEENTRY *src_palette,
    const RECT *src_rect,
    DWORD filter,
    D3DCOLOR color_key) {
    using Fn = HRESULT(WINAPI *)(IDirect3DSurface9 *, const PALETTEENTRY *, const RECT *, const void *, D3DFORMAT, UINT, const PALETTEENTRY *, const RECT *, DWORD, D3DCOLOR);
    auto fn = proc_as<Fn>("D3DXLoadSurfaceFromMemory");
    return fn ? fn(dst_surface, dst_palette, dst_rect, src_memory, src_format, src_pitch, src_palette, src_rect, filter, color_key) : E_FAIL;
}

BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID reserved) {
    (void)reserved;
    if (reason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(instance);
    }
    return TRUE;
}
