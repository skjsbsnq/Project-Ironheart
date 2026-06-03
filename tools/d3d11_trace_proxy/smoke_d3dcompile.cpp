#include <windows.h>

#include <cstdio>

using D3DCompileFn = HRESULT(WINAPI *)(
    const void *,
    SIZE_T,
    const char *,
    const void *,
    void *,
    const char *,
    const char *,
    UINT,
    UINT,
    void **,
    void **);

struct BlobLike {
    void **vtable;
};

using ReleaseFn = ULONG(STDMETHODCALLTYPE *)(void *);

int main() {
    HMODULE compiler = LoadLibraryW(L"d3dcompiler_47.dll");
    if (!compiler) {
        std::printf("LoadLibrary failed: %lu\n", GetLastError());
        return 1;
    }

    auto compile = reinterpret_cast<D3DCompileFn>(GetProcAddress(compiler, "D3DCompile"));
    if (!compile) {
        std::printf("GetProcAddress failed\n");
        return 1;
    }

    const char *src =
        "Texture2D DiffuseMap : register(t0);\n"
        "SamplerState DiffuseSampler : register(s0);\n"
        "cbuffer Globals : register(b0) { float4 Tint; };\n"
        "float4 main(float4 pos : SV_Position, float2 uv : TEXCOORD0) : SV_Target {\n"
        "  return DiffuseMap.Sample(DiffuseSampler, uv) * Tint;\n"
        "}\n";

    void *shader = nullptr;
    void *errors = nullptr;
    HRESULT hr = compile(src, strlen(src), "smoke_pdxmap.shader", nullptr, nullptr, "main", "ps_5_0", 0, 0, &shader, &errors);
    std::printf("D3DCompile hr=0x%08lx\n", static_cast<unsigned long>(hr));

    if (shader) {
        auto *blob = reinterpret_cast<BlobLike *>(shader);
        auto release = reinterpret_cast<ReleaseFn>(blob->vtable[2]);
        release(shader);
    }
    if (errors) {
        auto *blob = reinterpret_cast<BlobLike *>(errors);
        auto release = reinterpret_cast<ReleaseFn>(blob->vtable[2]);
        release(errors);
    }

    return SUCCEEDED(hr) ? 0 : 2;
}

