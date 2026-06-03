#include "trace_common.h"

#include <d3dcompiler.h>

using D3DCompileFn = HRESULT(WINAPI *)(
    const void *,
    SIZE_T,
    const char *,
    const D3D_SHADER_MACRO *,
    ID3DInclude *,
    const char *,
    const char *,
    UINT,
    UINT,
    ID3DBlob **,
    ID3DBlob **);

namespace {

D3DCompileFn real_d3d_compile() {
    static D3DCompileFn fn =
        reinterpret_cast<D3DCompileFn>(trace::get_system_proc("d3dcompiler_47.dll", "D3DCompile"));
    return fn;
}

std::string defines_to_json(const D3D_SHADER_MACRO *defines) {
    if (!defines) {
        return "[]";
    }

    std::string out = "[";
    bool first = true;
    for (const D3D_SHADER_MACRO *it = defines; it->Name; ++it) {
        if (!first) {
            out += ",";
        }
        first = false;
        out += "{\"name\":\"";
        out += trace::json_escape(it->Name);
        out += "\",\"definition\":\"";
        out += trace::json_escape(it->Definition ? it->Definition : "");
        out += "\"}";
    }
    out += "]";
    return out;
}

} // namespace

extern "C" __declspec(dllexport) HRESULT WINAPI D3DCompile(
    const void *data,
    SIZE_T data_size,
    const char *filename,
    const D3D_SHADER_MACRO *defines,
    ID3DInclude *include,
    const char *entrypoint,
    const char *target,
    UINT sflags,
    UINT eflags,
    ID3DBlob **shader,
    ID3DBlob **error_messages) {
    auto fn = real_d3d_compile();
    if (!fn) {
        return E_FAIL;
    }

    HRESULT hr = fn(data, data_size, filename, defines, include, entrypoint, target, sflags, eflags, shader, error_messages);

    std::string source_hash = trace::hash_bytes(data, data_size);
    std::string shader_hash;
    SIZE_T shader_size = 0;
    if (SUCCEEDED(hr) && shader && *shader) {
        shader_size = (*shader)->GetBufferSize();
        shader_hash = trace::hash_bytes((*shader)->GetBufferPointer(), shader_size);
    }

    std::string event = "{";
    event += "\"event\":\"D3DCompile\",";
    event += "\"hr\":\"" + trace::hr_hex(hr) + "\",";
    event += "\"filename\":\"" + trace::json_escape(filename ? filename : "") + "\",";
    event += "\"entrypoint\":\"" + trace::json_escape(entrypoint ? entrypoint : "") + "\",";
    event += "\"target\":\"" + trace::json_escape(target ? target : "") + "\",";
    event += "\"source_hash\":\"" + source_hash + "\",";
    event += "\"source_size\":" + std::to_string(static_cast<unsigned long long>(data_size)) + ",";
    event += "\"shader_hash\":\"" + shader_hash + "\",";
    event += "\"shader_size\":" + std::to_string(static_cast<unsigned long long>(shader_size)) + ",";
    event += "\"sflags\":" + std::to_string(sflags) + ",";
    event += "\"eflags\":" + std::to_string(eflags) + ",";
    event += "\"defines\":" + defines_to_json(defines);
    event += "}";
    trace::append_json_object(trace::trace_file_path(L"shader_compiles.json"), event);

    if (SUCCEEDED(hr) && shader && *shader) {
        trace::record_shader_bytecode(
            "D3DCompile",
            "",
            (*shader)->GetBufferPointer(),
            shader_size,
            filename ? filename : "",
            entrypoint ? entrypoint : "",
            target ? target : "");
    }

    return hr;
}

BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID reserved) {
    (void)reserved;
    if (reason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(instance);
    }
    return TRUE;
}

