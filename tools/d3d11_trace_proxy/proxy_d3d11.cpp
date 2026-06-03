#include "trace_common.h"

#include <d3d11.h>

#include <algorithm>
#include <cstdint>
#include <iomanip>
#include <sstream>
#include <string>
#include <unordered_map>
#include <vector>

using D3D11CreateDeviceFn = HRESULT(WINAPI *)(
    IDXGIAdapter *,
    D3D_DRIVER_TYPE,
    HMODULE,
    UINT,
    const D3D_FEATURE_LEVEL *,
    UINT,
    UINT,
    ID3D11Device **,
    D3D_FEATURE_LEVEL *,
    ID3D11DeviceContext **);

using CreateTexture2DFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11Device *,
    const D3D11_TEXTURE2D_DESC *,
    const D3D11_SUBRESOURCE_DATA *,
    ID3D11Texture2D **);

using CreateShaderResourceViewFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11Device *,
    ID3D11Resource *,
    const D3D11_SHADER_RESOURCE_VIEW_DESC *,
    ID3D11ShaderResourceView **);

using CreateRenderTargetViewFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11Device *,
    ID3D11Resource *,
    const D3D11_RENDER_TARGET_VIEW_DESC *,
    ID3D11RenderTargetView **);

using CreateDepthStencilViewFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11Device *,
    ID3D11Resource *,
    const D3D11_DEPTH_STENCIL_VIEW_DESC *,
    ID3D11DepthStencilView **);

using CreateVertexShaderFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11Device *,
    const void *,
    SIZE_T,
    ID3D11ClassLinkage *,
    ID3D11VertexShader **);

using CreatePixelShaderFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11Device *,
    const void *,
    SIZE_T,
    ID3D11ClassLinkage *,
    ID3D11PixelShader **);

using CreateBufferFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11Device *,
    const D3D11_BUFFER_DESC *,
    const D3D11_SUBRESOURCE_DATA *,
    ID3D11Buffer **);

using CreateSamplerStateFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11Device *,
    const D3D11_SAMPLER_DESC *,
    ID3D11SamplerState **);

using CreateBlendStateFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11Device *,
    const D3D11_BLEND_DESC *,
    ID3D11BlendState **);

using CreateDepthStencilStateFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11Device *,
    const D3D11_DEPTH_STENCIL_DESC *,
    ID3D11DepthStencilState **);

using CreateRasterizerStateFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11Device *,
    const D3D11_RASTERIZER_DESC *,
    ID3D11RasterizerState **);

using GetImmediateContextFn = void(STDMETHODCALLTYPE *)(ID3D11Device *, ID3D11DeviceContext **);

using VSSetConstantBuffersFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    UINT,
    UINT,
    ID3D11Buffer *const *);

using PSSetConstantBuffersFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    UINT,
    UINT,
    ID3D11Buffer *const *);

using VSSetShaderFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    ID3D11VertexShader *,
    ID3D11ClassInstance *const *,
    UINT);

using PSSetShaderResourcesFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    UINT,
    UINT,
    ID3D11ShaderResourceView *const *);

using PSSetSamplersFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    UINT,
    UINT,
    ID3D11SamplerState *const *);

using PSSetShaderFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    ID3D11PixelShader *,
    ID3D11ClassInstance *const *,
    UINT);

using OMSetRenderTargetsFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    UINT,
    ID3D11RenderTargetView *const *,
    ID3D11DepthStencilView *);

using OMSetRenderTargetsAndUnorderedAccessViewsFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    UINT,
    ID3D11RenderTargetView *const *,
    ID3D11DepthStencilView *,
    UINT,
    UINT,
    ID3D11UnorderedAccessView *const *,
    const UINT *);

using OMSetBlendStateFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    ID3D11BlendState *,
    const FLOAT[4],
    UINT);

using OMSetDepthStencilStateFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    ID3D11DepthStencilState *,
    UINT);

using RSSetStateFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    ID3D11RasterizerState *);

using DrawIndexedFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *, UINT, UINT, INT);
using DrawFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *, UINT, UINT);
using DrawIndexedInstancedFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *, UINT, UINT, UINT, INT, UINT);
using DrawInstancedFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *, UINT, UINT, UINT, UINT);
using DrawAutoFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *);
using DrawIndexedInstancedIndirectFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *, ID3D11Buffer *, UINT);
using DrawInstancedIndirectFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *, ID3D11Buffer *, UINT);
using MapFn = HRESULT(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    ID3D11Resource *,
    UINT,
    D3D11_MAP,
    UINT,
    D3D11_MAPPED_SUBRESOURCE *);
using UnmapFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *, ID3D11Resource *, UINT);
using CopySubresourceRegionFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    ID3D11Resource *,
    UINT,
    UINT,
    UINT,
    UINT,
    ID3D11Resource *,
    UINT,
    const D3D11_BOX *);
using CopyResourceFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *, ID3D11Resource *, ID3D11Resource *);
using UpdateSubresourceFn = void(STDMETHODCALLTYPE *)(
    ID3D11DeviceContext *,
    ID3D11Resource *,
    UINT,
    const D3D11_BOX *,
    const void *,
    UINT,
    UINT);
using ClearRenderTargetViewFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *, ID3D11RenderTargetView *, const FLOAT[4]);
using ClearDepthStencilViewFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *, ID3D11DepthStencilView *, UINT, FLOAT, UINT8);
using ResolveSubresourceFn = void(STDMETHODCALLTYPE *)(ID3D11DeviceContext *, ID3D11Resource *, UINT, ID3D11Resource *, UINT, DXGI_FORMAT);

namespace {

constexpr size_t kCreateTexture2DSlot = 5;
constexpr size_t kCreateShaderResourceViewSlot = 7;
constexpr size_t kCreateRenderTargetViewSlot = 9;
constexpr size_t kCreateDepthStencilViewSlot = 10;
constexpr size_t kCreateVertexShaderSlot = 12;
constexpr size_t kCreatePixelShaderSlot = 15;
constexpr size_t kCreateBufferSlot = 3;
constexpr size_t kCreateBlendStateSlot = 20;
constexpr size_t kCreateDepthStencilStateSlot = 21;
constexpr size_t kCreateRasterizerStateSlot = 22;
constexpr size_t kCreateSamplerStateSlot = 23;
constexpr size_t kGetImmediateContextSlot = 40;

constexpr size_t kVSSetConstantBuffersSlot = 7;
constexpr size_t kPSSetShaderResourcesSlot = 8;
constexpr size_t kPSSetShaderSlot = 9;
constexpr size_t kPSSetSamplersSlot = 10;
constexpr size_t kVSSetShaderSlot = 11;
constexpr size_t kDrawIndexedSlot = 12;
constexpr size_t kDrawSlot = 13;
constexpr size_t kMapSlot = 14;
constexpr size_t kUnmapSlot = 15;
constexpr size_t kPSSetConstantBuffersSlot = 16;
constexpr size_t kDrawIndexedInstancedSlot = 20;
constexpr size_t kDrawInstancedSlot = 21;
constexpr size_t kOMSetRenderTargetsSlot = 33;
constexpr size_t kOMSetRenderTargetsAndUnorderedAccessViewsSlot = 34;
constexpr size_t kOMSetBlendStateSlot = 35;
constexpr size_t kOMSetDepthStencilStateSlot = 36;
constexpr size_t kDrawAutoSlot = 38;
constexpr size_t kDrawIndexedInstancedIndirectSlot = 39;
constexpr size_t kDrawInstancedIndirectSlot = 40;
constexpr size_t kRSSetStateSlot = 43;
constexpr size_t kCopySubresourceRegionSlot = 46;
constexpr size_t kCopyResourceSlot = 47;
constexpr size_t kUpdateSubresourceSlot = 48;
constexpr size_t kClearRenderTargetViewSlot = 50;
constexpr size_t kClearDepthStencilViewSlot = 53;
constexpr size_t kResolveSubresourceSlot = 57;

SRWLOCK g_patch_lock = SRWLOCK_INIT;
SRWLOCK g_state_lock = SRWLOCK_INIT;

CreateTexture2DFn g_real_create_texture2d = nullptr;
CreateShaderResourceViewFn g_real_create_srv = nullptr;
CreateRenderTargetViewFn g_real_create_rtv = nullptr;
CreateDepthStencilViewFn g_real_create_dsv = nullptr;
CreateVertexShaderFn g_real_create_vertex_shader = nullptr;
CreatePixelShaderFn g_real_create_pixel_shader = nullptr;
CreateBufferFn g_real_create_buffer = nullptr;
CreateSamplerStateFn g_real_create_sampler_state = nullptr;
CreateBlendStateFn g_real_create_blend_state = nullptr;
CreateDepthStencilStateFn g_real_create_depth_stencil_state = nullptr;
CreateRasterizerStateFn g_real_create_rasterizer_state = nullptr;
GetImmediateContextFn g_real_get_immediate_context = nullptr;

VSSetConstantBuffersFn g_real_vs_set_constant_buffers = nullptr;
PSSetConstantBuffersFn g_real_ps_set_constant_buffers = nullptr;
VSSetShaderFn g_real_vs_set_shader = nullptr;
PSSetShaderResourcesFn g_real_ps_set_shader_resources = nullptr;
PSSetSamplersFn g_real_ps_set_samplers = nullptr;
PSSetShaderFn g_real_ps_set_shader = nullptr;
OMSetRenderTargetsFn g_real_om_set_render_targets = nullptr;
OMSetRenderTargetsAndUnorderedAccessViewsFn g_real_om_set_render_targets_and_uavs = nullptr;
OMSetBlendStateFn g_real_om_set_blend_state = nullptr;
OMSetDepthStencilStateFn g_real_om_set_depth_stencil_state = nullptr;
RSSetStateFn g_real_rs_set_state = nullptr;
DrawIndexedFn g_real_draw_indexed = nullptr;
DrawFn g_real_draw = nullptr;
DrawIndexedInstancedFn g_real_draw_indexed_instanced = nullptr;
DrawInstancedFn g_real_draw_instanced = nullptr;
DrawAutoFn g_real_draw_auto = nullptr;
DrawIndexedInstancedIndirectFn g_real_draw_indexed_instanced_indirect = nullptr;
DrawInstancedIndirectFn g_real_draw_instanced_indirect = nullptr;
MapFn g_real_map = nullptr;
UnmapFn g_real_unmap = nullptr;
CopySubresourceRegionFn g_real_copy_subresource_region = nullptr;
CopyResourceFn g_real_copy_resource = nullptr;
UpdateSubresourceFn g_real_update_subresource = nullptr;
ClearRenderTargetViewFn g_real_clear_rtv = nullptr;
ClearDepthStencilViewFn g_real_clear_dsv = nullptr;
ResolveSubresourceFn g_real_resolve_subresource = nullptr;

volatile LONG64 g_event_seq = 0;
volatile LONG64 g_texture_id = 0;
volatile LONG64 g_view_id = 0;
volatile LONG64 g_buffer_id = 0;
volatile LONG64 g_state_object_id = 0;

struct ShaderInfo {
    std::string stage;
    std::string hash;
};

struct TextureInfo {
    uint64_t id = 0;
    D3D11_TEXTURE2D_DESC desc = {};
};

struct BufferInfo {
    uint64_t id = 0;
    D3D11_BUFFER_DESC desc = {};
};

struct MappedInfo {
    void *resource = nullptr;
    UINT subresource = 0;
    void *data = nullptr;
    UINT row_pitch = 0;
    UINT depth_pitch = 0;
    size_t estimated_size = 0;
    UINT map_type = 0;
};

struct ViewInfo {
    uint64_t id = 0;
    std::string kind;
    void *resource = nullptr;
    uint64_t texture_id = 0;
    DXGI_FORMAT format = DXGI_FORMAT_UNKNOWN;
    UINT dimension = 0;
};

struct StateObjectInfo {
    uint64_t id = 0;
    std::string kind;
};

struct ContextState {
    std::string vs_hash;
    std::string ps_hash;
    std::unordered_map<UINT, std::string> vs_cbuffers;
    std::unordered_map<UINT, std::string> ps_cbuffers;
    std::unordered_map<UINT, std::string> ps_srvs;
    std::unordered_map<UINT, std::string> ps_samplers;
    std::vector<std::string> rtvs;
    std::string dsv;
    std::string blend_state;
    std::string depth_stencil_state;
    std::string rasterizer_state;
    UINT stencil_ref = 0;
    UINT blend_sample_mask = 0xffffffffu;
    FLOAT blend_factor[4] = {0.0f, 0.0f, 0.0f, 0.0f};
};

std::unordered_map<void *, ShaderInfo> g_shaders;
std::unordered_map<void *, TextureInfo> g_textures;
std::unordered_map<void *, BufferInfo> g_buffers;
std::unordered_map<void *, ViewInfo> g_views;
std::unordered_map<void *, StateObjectInfo> g_state_objects;
std::unordered_map<void *, ContextState> g_contexts;
std::unordered_map<std::string, MappedInfo> g_mapped_resources;

D3D11CreateDeviceFn real_d3d11_create_device() {
    static D3D11CreateDeviceFn fn =
        reinterpret_cast<D3D11CreateDeviceFn>(trace::get_system_proc("d3d11.dll", "D3D11CreateDevice"));
    return fn;
}

std::wstring runtime_trace_path() {
    return trace::trace_file_path(L"logs\\d3d11_trace.jsonl");
}

uint64_t next_seq() {
    return static_cast<uint64_t>(InterlockedIncrement64(&g_event_seq));
}

uint64_t next_texture_id() {
    return static_cast<uint64_t>(InterlockedIncrement64(&g_texture_id));
}

uint64_t next_view_id() {
    return static_cast<uint64_t>(InterlockedIncrement64(&g_view_id));
}

uint64_t next_buffer_id() {
    return static_cast<uint64_t>(InterlockedIncrement64(&g_buffer_id));
}

uint64_t next_state_object_id() {
    return static_cast<uint64_t>(InterlockedIncrement64(&g_state_object_id));
}

std::string event_prefix(const char *event) {
    std::string out = "{";
    out += "\"seq\":" + std::to_string(next_seq()) + ",";
    out += "\"pid\":" + std::to_string(trace::process_id()) + ",";
    out += "\"tick_ms\":" + std::to_string(trace::tick_ms()) + ",";
    out += "\"event\":\"" + trace::json_escape(event) + "\"";
    return out;
}

void log_event(const std::string &object_json) {
    trace::append_json_line(runtime_trace_path(), object_json);
}

std::string format_name(DXGI_FORMAT format) {
    switch (format) {
    case DXGI_FORMAT_R32G32B32A32_FLOAT:
        return "R32G32B32A32_FLOAT";
    case DXGI_FORMAT_R16G16B16A16_FLOAT:
        return "R16G16B16A16_FLOAT";
    case DXGI_FORMAT_R8G8B8A8_UNORM:
        return "R8G8B8A8_UNORM";
    case DXGI_FORMAT_R8G8B8A8_UNORM_SRGB:
        return "R8G8B8A8_UNORM_SRGB";
    case DXGI_FORMAT_B8G8R8A8_UNORM:
        return "B8G8R8A8_UNORM";
    case DXGI_FORMAT_B8G8R8A8_UNORM_SRGB:
        return "B8G8R8A8_UNORM_SRGB";
    case DXGI_FORMAT_R8_UNORM:
        return "R8_UNORM";
    case DXGI_FORMAT_R16_FLOAT:
        return "R16_FLOAT";
    case DXGI_FORMAT_R32_FLOAT:
        return "R32_FLOAT";
    case DXGI_FORMAT_D24_UNORM_S8_UINT:
        return "D24_UNORM_S8_UINT";
    case DXGI_FORMAT_D32_FLOAT:
        return "D32_FLOAT";
    case DXGI_FORMAT_BC1_UNORM:
        return "BC1_UNORM";
    case DXGI_FORMAT_BC1_UNORM_SRGB:
        return "BC1_UNORM_SRGB";
    case DXGI_FORMAT_BC3_UNORM:
        return "BC3_UNORM";
    case DXGI_FORMAT_BC3_UNORM_SRGB:
        return "BC3_UNORM_SRGB";
    case DXGI_FORMAT_BC5_UNORM:
        return "BC5_UNORM";
    default:
        return "DXGI_FORMAT_" + std::to_string(static_cast<unsigned>(format));
    }
}

std::string bind_flags_json(UINT flags) {
    struct FlagName {
        UINT flag;
        const char *name;
    };
    static const FlagName names[] = {
        {D3D11_BIND_VERTEX_BUFFER, "VERTEX_BUFFER"},
        {D3D11_BIND_INDEX_BUFFER, "INDEX_BUFFER"},
        {D3D11_BIND_CONSTANT_BUFFER, "CONSTANT_BUFFER"},
        {D3D11_BIND_SHADER_RESOURCE, "SHADER_RESOURCE"},
        {D3D11_BIND_STREAM_OUTPUT, "STREAM_OUTPUT"},
        {D3D11_BIND_RENDER_TARGET, "RENDER_TARGET"},
        {D3D11_BIND_DEPTH_STENCIL, "DEPTH_STENCIL"},
        {D3D11_BIND_UNORDERED_ACCESS, "UNORDERED_ACCESS"},
        {D3D11_BIND_DECODER, "DECODER"},
        {D3D11_BIND_VIDEO_ENCODER, "VIDEO_ENCODER"},
    };

    std::string out = "[";
    bool first = true;
    for (const auto &entry : names) {
        if ((flags & entry.flag) == 0) {
            continue;
        }
        if (!first) {
            out += ",";
        }
        first = false;
        out += "\"";
        out += entry.name;
        out += "\"";
    }
    out += "]";
    return out;
}

std::string buffer_desc_json(const D3D11_BUFFER_DESC &desc) {
    std::string out = "{";
    out += "\"byte_width\":" + std::to_string(desc.ByteWidth) + ",";
    out += "\"usage\":" + std::to_string(static_cast<unsigned>(desc.Usage)) + ",";
    out += "\"bind_flags\":" + std::to_string(desc.BindFlags) + ",";
    out += "\"bind_flag_names\":" + bind_flags_json(desc.BindFlags) + ",";
    out += "\"cpu_access_flags\":" + std::to_string(desc.CPUAccessFlags) + ",";
    out += "\"misc_flags\":" + std::to_string(desc.MiscFlags) + ",";
    out += "\"structure_byte_stride\":" + std::to_string(desc.StructureByteStride);
    out += "}";
    return out;
}

std::string texture_desc_json(const D3D11_TEXTURE2D_DESC &desc) {
    std::string out = "{";
    out += "\"width\":" + std::to_string(desc.Width) + ",";
    out += "\"height\":" + std::to_string(desc.Height) + ",";
    out += "\"mip_levels\":" + std::to_string(desc.MipLevels) + ",";
    out += "\"array_size\":" + std::to_string(desc.ArraySize) + ",";
    out += "\"format\":\"" + format_name(desc.Format) + "\",";
    out += "\"format_id\":" + std::to_string(static_cast<unsigned>(desc.Format)) + ",";
    out += "\"sample_count\":" + std::to_string(desc.SampleDesc.Count) + ",";
    out += "\"sample_quality\":" + std::to_string(desc.SampleDesc.Quality) + ",";
    out += "\"usage\":" + std::to_string(static_cast<unsigned>(desc.Usage)) + ",";
    out += "\"bind_flags\":" + std::to_string(desc.BindFlags) + ",";
    out += "\"bind_flag_names\":" + bind_flags_json(desc.BindFlags) + ",";
    out += "\"cpu_access_flags\":" + std::to_string(desc.CPUAccessFlags) + ",";
    out += "\"misc_flags\":" + std::to_string(desc.MiscFlags);
    out += "}";
    return out;
}

std::string sampler_desc_json(const D3D11_SAMPLER_DESC &desc) {
    std::string out = "{";
    out += "\"filter\":" + std::to_string(static_cast<unsigned>(desc.Filter)) + ",";
    out += "\"address_u\":" + std::to_string(static_cast<unsigned>(desc.AddressU)) + ",";
    out += "\"address_v\":" + std::to_string(static_cast<unsigned>(desc.AddressV)) + ",";
    out += "\"address_w\":" + std::to_string(static_cast<unsigned>(desc.AddressW)) + ",";
    out += "\"mip_lod_bias\":" + std::to_string(desc.MipLODBias) + ",";
    out += "\"max_anisotropy\":" + std::to_string(desc.MaxAnisotropy) + ",";
    out += "\"comparison_func\":" + std::to_string(static_cast<unsigned>(desc.ComparisonFunc)) + ",";
    out += "\"border_color\":[" + std::to_string(desc.BorderColor[0]) + "," + std::to_string(desc.BorderColor[1]) + "," + std::to_string(desc.BorderColor[2]) + "," + std::to_string(desc.BorderColor[3]) + "],";
    out += "\"min_lod\":" + std::to_string(desc.MinLOD) + ",";
    out += "\"max_lod\":" + std::to_string(desc.MaxLOD);
    out += "}";
    return out;
}

std::string blend_desc_json(const D3D11_BLEND_DESC &desc) {
    std::string out = "{";
    out += "\"alpha_to_coverage_enable\":";
    out += desc.AlphaToCoverageEnable ? "true" : "false";
    out += ",\"independent_blend_enable\":";
    out += desc.IndependentBlendEnable ? "true" : "false";
    out += ",\"render_targets\":[";
    for (UINT i = 0; i < 8; ++i) {
        if (i) {
            out += ",";
        }
        const auto &rt = desc.RenderTarget[i];
        out += "{";
        out += "\"blend_enable\":";
        out += rt.BlendEnable ? "true" : "false";
        out += ",\"src_blend\":" + std::to_string(static_cast<unsigned>(rt.SrcBlend));
        out += ",\"dest_blend\":" + std::to_string(static_cast<unsigned>(rt.DestBlend));
        out += ",\"blend_op\":" + std::to_string(static_cast<unsigned>(rt.BlendOp));
        out += ",\"src_blend_alpha\":" + std::to_string(static_cast<unsigned>(rt.SrcBlendAlpha));
        out += ",\"dest_blend_alpha\":" + std::to_string(static_cast<unsigned>(rt.DestBlendAlpha));
        out += ",\"blend_op_alpha\":" + std::to_string(static_cast<unsigned>(rt.BlendOpAlpha));
        out += ",\"render_target_write_mask\":" + std::to_string(static_cast<unsigned>(rt.RenderTargetWriteMask));
        out += "}";
    }
    out += "]";
    return out;
}

std::string depth_stencil_desc_json(const D3D11_DEPTH_STENCIL_DESC &desc) {
    std::string out = "{";
    out += "\"depth_enable\":";
    out += desc.DepthEnable ? "true" : "false";
    out += ",\"depth_write_mask\":" + std::to_string(static_cast<unsigned>(desc.DepthWriteMask));
    out += ",\"depth_func\":" + std::to_string(static_cast<unsigned>(desc.DepthFunc));
    out += ",\"stencil_enable\":";
    out += desc.StencilEnable ? "true" : "false";
    out += ",\"stencil_read_mask\":" + std::to_string(static_cast<unsigned>(desc.StencilReadMask));
    out += ",\"stencil_write_mask\":" + std::to_string(static_cast<unsigned>(desc.StencilWriteMask));
    out += ",\"front_fail_op\":" + std::to_string(static_cast<unsigned>(desc.FrontFace.StencilFailOp));
    out += ",\"front_depth_fail_op\":" + std::to_string(static_cast<unsigned>(desc.FrontFace.StencilDepthFailOp));
    out += ",\"front_pass_op\":" + std::to_string(static_cast<unsigned>(desc.FrontFace.StencilPassOp));
    out += ",\"front_func\":" + std::to_string(static_cast<unsigned>(desc.FrontFace.StencilFunc));
    out += ",\"back_fail_op\":" + std::to_string(static_cast<unsigned>(desc.BackFace.StencilFailOp));
    out += ",\"back_depth_fail_op\":" + std::to_string(static_cast<unsigned>(desc.BackFace.StencilDepthFailOp));
    out += ",\"back_pass_op\":" + std::to_string(static_cast<unsigned>(desc.BackFace.StencilPassOp));
    out += ",\"back_func\":" + std::to_string(static_cast<unsigned>(desc.BackFace.StencilFunc));
    out += "}";
    return out;
}

std::string rasterizer_desc_json(const D3D11_RASTERIZER_DESC &desc) {
    std::string out = "{";
    out += "\"fill_mode\":" + std::to_string(static_cast<unsigned>(desc.FillMode)) + ",";
    out += "\"cull_mode\":" + std::to_string(static_cast<unsigned>(desc.CullMode)) + ",";
    out += "\"front_counter_clockwise\":";
    out += desc.FrontCounterClockwise ? "true" : "false";
    out += ",\"depth_bias\":" + std::to_string(desc.DepthBias);
    out += ",\"depth_bias_clamp\":" + std::to_string(desc.DepthBiasClamp);
    out += ",\"slope_scaled_depth_bias\":" + std::to_string(desc.SlopeScaledDepthBias);
    out += ",\"depth_clip_enable\":";
    out += desc.DepthClipEnable ? "true" : "false";
    out += ",\"scissor_enable\":";
    out += desc.ScissorEnable ? "true" : "false";
    out += ",\"multisample_enable\":";
    out += desc.MultisampleEnable ? "true" : "false";
    out += ",\"antialiased_line_enable\":";
    out += desc.AntialiasedLineEnable ? "true" : "false";
    out += "}";
    return out;
}

uint64_t texture_id_for_resource(void *resource) {
    auto it = g_textures.find(resource);
    return it == g_textures.end() ? 0 : it->second.id;
}

uint64_t buffer_id_for_ptr(void *buffer) {
    auto it = g_buffers.find(buffer);
    return it == g_buffers.end() ? 0 : it->second.id;
}

const BufferInfo *buffer_info_for_ptr(void *buffer) {
    auto it = g_buffers.find(buffer);
    return it == g_buffers.end() ? nullptr : &it->second;
}

const TextureInfo *texture_info_for_ptr(void *texture) {
    auto it = g_textures.find(texture);
    return it == g_textures.end() ? nullptr : &it->second;
}

bool is_constant_buffer_resource(void *resource) {
    const auto *buffer = buffer_info_for_ptr(resource);
    return buffer && (buffer->desc.BindFlags & D3D11_BIND_CONSTANT_BUFFER) != 0;
}

std::string mapped_key(void *context, void *resource, UINT subresource) {
    return trace::ptr_hex(context) + "|" + trace::ptr_hex(resource) + "|" + std::to_string(subresource);
}

std::string hex_prefix(const void *data, size_t len, size_t cap = 4096) {
    if (!data || len == 0) {
        return "";
    }
    const auto *bytes = static_cast<const uint8_t *>(data);
    const size_t n = std::min(len, cap);
    std::ostringstream ss;
    ss << std::hex << std::setfill('0');
    for (size_t i = 0; i < n; ++i) {
        ss << std::setw(2) << static_cast<unsigned>(bytes[i]);
    }
    return ss.str();
}

std::string box_json(const D3D11_BOX *box) {
    if (!box) {
        return "null";
    }
    std::string out = "{";
    out += "\"left\":" + std::to_string(box->left);
    out += ",\"top\":" + std::to_string(box->top);
    out += ",\"front\":" + std::to_string(box->front);
    out += ",\"right\":" + std::to_string(box->right);
    out += ",\"bottom\":" + std::to_string(box->bottom);
    out += ",\"back\":" + std::to_string(box->back);
    out += "}";
    return out;
}

size_t bytes_per_pixel_estimate(DXGI_FORMAT format) {
    switch (format) {
    case DXGI_FORMAT_R32G32B32A32_FLOAT:
        return 16;
    case DXGI_FORMAT_R16G16B16A16_FLOAT:
    case DXGI_FORMAT_R32G32_FLOAT:
        return 8;
    case DXGI_FORMAT_R8G8B8A8_UNORM:
    case DXGI_FORMAT_R8G8B8A8_UNORM_SRGB:
    case DXGI_FORMAT_B8G8R8A8_UNORM:
    case DXGI_FORMAT_B8G8R8A8_UNORM_SRGB:
    case DXGI_FORMAT_R32_FLOAT:
    case DXGI_FORMAT_D24_UNORM_S8_UINT:
        return 4;
    case DXGI_FORMAT_R16_FLOAT:
        return 2;
    case DXGI_FORMAT_R8_UNORM:
        return 1;
    default:
        return 4;
    }
}

size_t estimate_texture_update_size(const D3D11_TEXTURE2D_DESC &desc, UINT row_pitch, const D3D11_BOX *box) {
    UINT width = desc.Width;
    UINT height = desc.Height;
    UINT depth = 1;
    if (box) {
        width = box->right > box->left ? box->right - box->left : 0;
        height = box->bottom > box->top ? box->bottom - box->top : 0;
        depth = box->back > box->front ? box->back - box->front : 1;
    }
    if (width == 0 || height == 0 || depth == 0) {
        return 0;
    }
    const size_t row = row_pitch ? row_pitch : static_cast<size_t>(width) * bytes_per_pixel_estimate(desc.Format);
    return row * static_cast<size_t>(height) * static_cast<size_t>(depth);
}

size_t estimate_resource_data_size(void *resource, UINT row_pitch, const D3D11_BOX *box) {
    if (const auto *buffer = buffer_info_for_ptr(resource)) {
        return buffer->desc.ByteWidth;
    }
    if (const auto *texture = texture_info_for_ptr(resource)) {
        return estimate_texture_update_size(texture->desc, row_pitch, box);
    }
    return 0;
}

std::string resource_ref_json(void *resource) {
    std::string out = "{";
    out += "\"ptr\":\"" + trace::ptr_hex(resource) + "\"";
    if (const auto *buffer = buffer_info_for_ptr(resource)) {
        out += ",\"kind\":\"buffer\",\"buffer_id\":" + std::to_string(buffer->id);
        out += ",\"byte_width\":" + std::to_string(buffer->desc.ByteWidth);
        out += ",\"bind_flags\":" + std::to_string(buffer->desc.BindFlags);
    } else if (const auto *texture = texture_info_for_ptr(resource)) {
        out += ",\"kind\":\"texture2d\",\"texture_id\":" + std::to_string(texture->id);
        out += ",\"width\":" + std::to_string(texture->desc.Width);
        out += ",\"height\":" + std::to_string(texture->desc.Height);
        out += ",\"format\":\"" + format_name(texture->desc.Format) + "\"";
    } else {
        out += ",\"kind\":\"unknown\"";
    }
    out += "}";
    return out;
}

std::string data_digest_json(const void *data, size_t len, bool include_prefix) {
    const size_t kMaxHashBytes = 4u * 1024u * 1024u;
    const size_t hash_len = std::min(len, kMaxHashBytes);
    std::string out = "{";
    out += "\"size\":" + std::to_string(len);
    out += ",\"hashed_size\":" + std::to_string(hash_len);
    out += ",\"hash\":\"" + trace::json_escape(trace::hash_bytes(data, hash_len)) + "\"";
    if (include_prefix) {
        out += ",\"prefix_hex\":\"" + hex_prefix(data, len, 4096) + "\"";
    }
    out += "}";
    return out;
}

std::string buffer_ref_for_ptr(void *ptr) {
    if (!ptr) {
        return "";
    }
    auto it = g_buffers.find(ptr);
    if (it == g_buffers.end()) {
        return trace::ptr_hex(ptr);
    }
    return std::to_string(it->second.id);
}

std::string state_object_ref_for_ptr(void *ptr) {
    if (!ptr) {
        return "";
    }
    auto it = g_state_objects.find(ptr);
    if (it == g_state_objects.end()) {
        return trace::ptr_hex(ptr);
    }
    return std::to_string(it->second.id);
}

std::string view_json_for_ptr(void *ptr) {
    if (!ptr) {
        return "{\"ptr\":\"0x0\",\"id\":0}";
    }
    auto it = g_views.find(ptr);
    if (it == g_views.end()) {
        return "{\"ptr\":\"" + trace::ptr_hex(ptr) + "\",\"id\":0}";
    }
    const auto &view = it->second;
    std::string out = "{";
    out += "\"ptr\":\"" + trace::ptr_hex(ptr) + "\",";
    out += "\"id\":" + std::to_string(view.id) + ",";
    out += "\"kind\":\"" + view.kind + "\",";
    out += "\"resource\":\"" + trace::ptr_hex(view.resource) + "\",";
    out += "\"texture_id\":" + std::to_string(view.texture_id);
    out += "}";
    return out;
}

std::string view_ref_for_ptr(void *ptr) {
    auto it = g_views.find(ptr);
    if (it == g_views.end()) {
        return trace::ptr_hex(ptr);
    }
    return std::to_string(it->second.id);
}

std::string slot_map_json(const std::unordered_map<UINT, std::string> &map) {
    std::string out = "[";
    bool first = true;
    std::vector<UINT> slots;
    slots.reserve(map.size());
    for (const auto &entry : map) {
        slots.push_back(entry.first);
    }
    std::sort(slots.begin(), slots.end());
    for (UINT slot : slots) {
        const auto it = map.find(slot);
        if (it == map.end() || it->second.empty()) {
            continue;
        }
        if (!first) {
            out += ",";
        }
        first = false;
        out += "{\"slot\":" + std::to_string(slot) + ",\"value\":\"" + trace::json_escape(it->second) + "\"}";
    }
    out += "]";
    return out;
}

void patch_context_vtable(ID3D11DeviceContext *context);

void patch_slot(void **vtable, size_t slot, void *replacement, void **real_storage) {
    if (!vtable || !replacement || !real_storage) {
        return;
    }
    if (!*real_storage && vtable[slot] != replacement) {
        *real_storage = vtable[slot];
    }
    if (!*real_storage || vtable[slot] == replacement) {
        return;
    }
    DWORD old_protect = 0;
    if (VirtualProtect(&vtable[slot], sizeof(void *), PAGE_EXECUTE_READWRITE, &old_protect)) {
        vtable[slot] = replacement;
        VirtualProtect(&vtable[slot], sizeof(void *), old_protect, &old_protect);
    }
}

void log_context_state_draw(ID3D11DeviceContext *context, const char *kind, const std::string &payload) {
    ContextState state;
    AcquireSRWLockShared(&g_state_lock);
    auto it = g_contexts.find(context);
    if (it != g_contexts.end()) {
        state = it->second;
    }
    ReleaseSRWLockShared(&g_state_lock);

    std::string out = event_prefix(kind);
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\"";
    out += payload;
    out += ",\"vs_hash\":\"" + trace::json_escape(state.vs_hash) + "\"";
    out += ",\"ps_hash\":\"" + trace::json_escape(state.ps_hash) + "\"";
    out += ",\"rtvs\":[";
    for (size_t i = 0; i < state.rtvs.size(); ++i) {
        if (i) {
            out += ",";
        }
        out += "\"" + trace::json_escape(state.rtvs[i]) + "\"";
    }
    out += "]";
    out += ",\"dsv\":\"" + trace::json_escape(state.dsv) + "\"";
    out += ",\"ps_srvs\":[";
    bool first = true;
    std::vector<UINT> slots;
    slots.reserve(state.ps_srvs.size());
    for (const auto &entry : state.ps_srvs) {
        slots.push_back(entry.first);
    }
    std::sort(slots.begin(), slots.end());
    for (UINT slot : slots) {
        const auto srv = state.ps_srvs.find(slot);
        if (srv == state.ps_srvs.end() || srv->second.empty()) {
            continue;
        }
        if (!first) {
            out += ",";
        }
        first = false;
        out += "{\"slot\":" + std::to_string(slot) + ",\"view\":\"" + trace::json_escape(srv->second) + "\"}";
    }
    out += "]";
    out += ",\"vs_cbuffers\":" + slot_map_json(state.vs_cbuffers);
    out += ",\"ps_cbuffers\":" + slot_map_json(state.ps_cbuffers);
    out += ",\"ps_samplers\":" + slot_map_json(state.ps_samplers);
    out += ",\"blend_state\":\"" + trace::json_escape(state.blend_state) + "\"";
    out += ",\"depth_stencil_state\":\"" + trace::json_escape(state.depth_stencil_state) + "\"";
    out += ",\"rasterizer_state\":\"" + trace::json_escape(state.rasterizer_state) + "\"";
    out += ",\"stencil_ref\":" + std::to_string(state.stencil_ref);
    out += ",\"blend_sample_mask\":" + std::to_string(state.blend_sample_mask);
    out += ",\"blend_factor\":[" + std::to_string(state.blend_factor[0]) + "," + std::to_string(state.blend_factor[1]) + "," + std::to_string(state.blend_factor[2]) + "," + std::to_string(state.blend_factor[3]) + "]";
    out += "}";
    log_event(out);
}

HRESULT STDMETHODCALLTYPE traced_create_texture2d(
    ID3D11Device *device,
    const D3D11_TEXTURE2D_DESC *desc,
    const D3D11_SUBRESOURCE_DATA *initial_data,
    ID3D11Texture2D **texture) {
    HRESULT hr = g_real_create_texture2d(device, desc, initial_data, texture);
    if (SUCCEEDED(hr) && texture && *texture && desc) {
        TextureInfo info;
        info.id = next_texture_id();
        info.desc = *desc;
        AcquireSRWLockExclusive(&g_state_lock);
        g_textures[*texture] = info;
        ReleaseSRWLockExclusive(&g_state_lock);

        std::string out = event_prefix("ID3D11Device::CreateTexture2D");
        out += ",\"device\":\"" + trace::ptr_hex(device) + "\"";
        out += ",\"texture\":\"" + trace::ptr_hex(*texture) + "\"";
        out += ",\"texture_id\":" + std::to_string(info.id);
        out += ",\"initial_data\":";
        out += initial_data ? "true" : "false";
        if (initial_data && initial_data->pSysMem) {
            out += ",\"sys_mem_pitch\":" + std::to_string(initial_data->SysMemPitch);
            out += ",\"sys_mem_slice_pitch\":" + std::to_string(initial_data->SysMemSlicePitch);
        }
        out += ",\"desc\":" + texture_desc_json(*desc);
        out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
        log_event(out);
    }
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_create_buffer(
    ID3D11Device *device,
    const D3D11_BUFFER_DESC *desc,
    const D3D11_SUBRESOURCE_DATA *initial_data,
    ID3D11Buffer **buffer) {
    HRESULT hr = g_real_create_buffer(device, desc, initial_data, buffer);
    if (SUCCEEDED(hr) && buffer && *buffer && desc) {
        BufferInfo info;
        info.id = next_buffer_id();
        info.desc = *desc;
        AcquireSRWLockExclusive(&g_state_lock);
        g_buffers[*buffer] = info;
        ReleaseSRWLockExclusive(&g_state_lock);

        std::string out = event_prefix("ID3D11Device::CreateBuffer");
        out += ",\"device\":\"" + trace::ptr_hex(device) + "\"";
        out += ",\"buffer\":\"" + trace::ptr_hex(*buffer) + "\"";
        out += ",\"buffer_id\":" + std::to_string(info.id);
        out += ",\"initial_data\":";
        out += initial_data ? "true" : "false";
        if (initial_data && initial_data->pSysMem && (desc->BindFlags & D3D11_BIND_CONSTANT_BUFFER) != 0) {
            out += ",\"initial_digest\":" + data_digest_json(initial_data->pSysMem, desc->ByteWidth, true);
        }
        out += ",\"desc\":" + buffer_desc_json(*desc);
        out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
        log_event(out);
    }
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_create_srv(
    ID3D11Device *device,
    ID3D11Resource *resource,
    const D3D11_SHADER_RESOURCE_VIEW_DESC *desc,
    ID3D11ShaderResourceView **view) {
    HRESULT hr = g_real_create_srv(device, resource, desc, view);
    if (SUCCEEDED(hr) && view && *view) {
        D3D11_SHADER_RESOURCE_VIEW_DESC actual = {};
        (*view)->GetDesc(&actual);
        ViewInfo info;
        info.id = next_view_id();
        info.kind = "SRV";
        info.resource = resource;
        info.format = actual.Format;
        info.dimension = static_cast<UINT>(actual.ViewDimension);
        AcquireSRWLockExclusive(&g_state_lock);
        info.texture_id = texture_id_for_resource(resource);
        g_views[*view] = info;
        ReleaseSRWLockExclusive(&g_state_lock);

        std::string out = event_prefix("ID3D11Device::CreateShaderResourceView");
        out += ",\"device\":\"" + trace::ptr_hex(device) + "\"";
        out += ",\"view\":\"" + trace::ptr_hex(*view) + "\"";
        out += ",\"view_id\":" + std::to_string(info.id);
        out += ",\"resource\":\"" + trace::ptr_hex(resource) + "\"";
        out += ",\"texture_id\":" + std::to_string(info.texture_id);
        out += ",\"format\":\"" + format_name(actual.Format) + "\"";
        out += ",\"format_id\":" + std::to_string(static_cast<unsigned>(actual.Format));
        out += ",\"view_dimension\":" + std::to_string(static_cast<unsigned>(actual.ViewDimension));
        out += ",\"explicit_desc\":";
        out += desc ? "true" : "false";
        out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
        log_event(out);
    }
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_create_rtv(
    ID3D11Device *device,
    ID3D11Resource *resource,
    const D3D11_RENDER_TARGET_VIEW_DESC *desc,
    ID3D11RenderTargetView **view) {
    HRESULT hr = g_real_create_rtv(device, resource, desc, view);
    if (SUCCEEDED(hr) && view && *view) {
        D3D11_RENDER_TARGET_VIEW_DESC actual = {};
        (*view)->GetDesc(&actual);
        ViewInfo info;
        info.id = next_view_id();
        info.kind = "RTV";
        info.resource = resource;
        info.format = actual.Format;
        info.dimension = static_cast<UINT>(actual.ViewDimension);
        AcquireSRWLockExclusive(&g_state_lock);
        info.texture_id = texture_id_for_resource(resource);
        g_views[*view] = info;
        ReleaseSRWLockExclusive(&g_state_lock);

        std::string out = event_prefix("ID3D11Device::CreateRenderTargetView");
        out += ",\"device\":\"" + trace::ptr_hex(device) + "\"";
        out += ",\"view\":\"" + trace::ptr_hex(*view) + "\"";
        out += ",\"view_id\":" + std::to_string(info.id);
        out += ",\"resource\":\"" + trace::ptr_hex(resource) + "\"";
        out += ",\"texture_id\":" + std::to_string(info.texture_id);
        out += ",\"format\":\"" + format_name(actual.Format) + "\"";
        out += ",\"format_id\":" + std::to_string(static_cast<unsigned>(actual.Format));
        out += ",\"view_dimension\":" + std::to_string(static_cast<unsigned>(actual.ViewDimension));
        out += ",\"explicit_desc\":";
        out += desc ? "true" : "false";
        out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
        log_event(out);
    }
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_create_dsv(
    ID3D11Device *device,
    ID3D11Resource *resource,
    const D3D11_DEPTH_STENCIL_VIEW_DESC *desc,
    ID3D11DepthStencilView **view) {
    HRESULT hr = g_real_create_dsv(device, resource, desc, view);
    if (SUCCEEDED(hr) && view && *view) {
        D3D11_DEPTH_STENCIL_VIEW_DESC actual = {};
        (*view)->GetDesc(&actual);
        ViewInfo info;
        info.id = next_view_id();
        info.kind = "DSV";
        info.resource = resource;
        info.format = actual.Format;
        info.dimension = static_cast<UINT>(actual.ViewDimension);
        AcquireSRWLockExclusive(&g_state_lock);
        info.texture_id = texture_id_for_resource(resource);
        g_views[*view] = info;
        ReleaseSRWLockExclusive(&g_state_lock);

        std::string out = event_prefix("ID3D11Device::CreateDepthStencilView");
        out += ",\"device\":\"" + trace::ptr_hex(device) + "\"";
        out += ",\"view\":\"" + trace::ptr_hex(*view) + "\"";
        out += ",\"view_id\":" + std::to_string(info.id);
        out += ",\"resource\":\"" + trace::ptr_hex(resource) + "\"";
        out += ",\"texture_id\":" + std::to_string(info.texture_id);
        out += ",\"format\":\"" + format_name(actual.Format) + "\"";
        out += ",\"format_id\":" + std::to_string(static_cast<unsigned>(actual.Format));
        out += ",\"view_dimension\":" + std::to_string(static_cast<unsigned>(actual.ViewDimension));
        out += ",\"explicit_desc\":";
        out += desc ? "true" : "false";
        out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
        log_event(out);
    }
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_create_vertex_shader(
    ID3D11Device *device,
    const void *bytecode,
    SIZE_T bytecode_len,
    ID3D11ClassLinkage *class_linkage,
    ID3D11VertexShader **vertex_shader) {
    std::string hash = trace::hash_bytes(bytecode, bytecode_len);
    trace::record_shader_bytecode("ID3D11Device::CreateVertexShader", "vertex", bytecode, bytecode_len, "", "", "");
    HRESULT hr = g_real_create_vertex_shader(device, bytecode, bytecode_len, class_linkage, vertex_shader);
    if (SUCCEEDED(hr) && vertex_shader && *vertex_shader) {
        AcquireSRWLockExclusive(&g_state_lock);
        g_shaders[*vertex_shader] = ShaderInfo{"vertex", hash};
        ReleaseSRWLockExclusive(&g_state_lock);
        std::string out = event_prefix("ID3D11Device::CreateVertexShader");
        out += ",\"shader\":\"" + trace::ptr_hex(*vertex_shader) + "\",\"stage\":\"vertex\",\"bytecode_hash\":\"" + hash + "\",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
        log_event(out);
    }
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_create_pixel_shader(
    ID3D11Device *device,
    const void *bytecode,
    SIZE_T bytecode_len,
    ID3D11ClassLinkage *class_linkage,
    ID3D11PixelShader **pixel_shader) {
    std::string hash = trace::hash_bytes(bytecode, bytecode_len);
    trace::record_shader_bytecode("ID3D11Device::CreatePixelShader", "pixel", bytecode, bytecode_len, "", "", "");
    HRESULT hr = g_real_create_pixel_shader(device, bytecode, bytecode_len, class_linkage, pixel_shader);
    if (SUCCEEDED(hr) && pixel_shader && *pixel_shader) {
        AcquireSRWLockExclusive(&g_state_lock);
        g_shaders[*pixel_shader] = ShaderInfo{"pixel", hash};
        ReleaseSRWLockExclusive(&g_state_lock);
        std::string out = event_prefix("ID3D11Device::CreatePixelShader");
        out += ",\"shader\":\"" + trace::ptr_hex(*pixel_shader) + "\",\"stage\":\"pixel\",\"bytecode_hash\":\"" + hash + "\",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
        log_event(out);
    }
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_create_sampler_state(
    ID3D11Device *device,
    const D3D11_SAMPLER_DESC *desc,
    ID3D11SamplerState **sampler) {
    HRESULT hr = g_real_create_sampler_state(device, desc, sampler);
    if (SUCCEEDED(hr) && sampler && *sampler && desc) {
        StateObjectInfo info;
        info.id = next_state_object_id();
        info.kind = "SamplerState";
        AcquireSRWLockExclusive(&g_state_lock);
        g_state_objects[*sampler] = info;
        ReleaseSRWLockExclusive(&g_state_lock);

        std::string out = event_prefix("ID3D11Device::CreateSamplerState");
        out += ",\"device\":\"" + trace::ptr_hex(device) + "\"";
        out += ",\"state\":\"" + trace::ptr_hex(*sampler) + "\"";
        out += ",\"state_id\":" + std::to_string(info.id);
        out += ",\"kind\":\"SamplerState\"";
        out += ",\"desc\":" + sampler_desc_json(*desc);
        out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
        log_event(out);
    }
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_create_blend_state(
    ID3D11Device *device,
    const D3D11_BLEND_DESC *desc,
    ID3D11BlendState **state) {
    HRESULT hr = g_real_create_blend_state(device, desc, state);
    if (SUCCEEDED(hr) && state && *state && desc) {
        StateObjectInfo info;
        info.id = next_state_object_id();
        info.kind = "BlendState";
        AcquireSRWLockExclusive(&g_state_lock);
        g_state_objects[*state] = info;
        ReleaseSRWLockExclusive(&g_state_lock);

        std::string out = event_prefix("ID3D11Device::CreateBlendState");
        out += ",\"device\":\"" + trace::ptr_hex(device) + "\"";
        out += ",\"state\":\"" + trace::ptr_hex(*state) + "\"";
        out += ",\"state_id\":" + std::to_string(info.id);
        out += ",\"kind\":\"BlendState\"";
        out += ",\"desc\":" + blend_desc_json(*desc);
        out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
        log_event(out);
    }
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_create_depth_stencil_state(
    ID3D11Device *device,
    const D3D11_DEPTH_STENCIL_DESC *desc,
    ID3D11DepthStencilState **state) {
    HRESULT hr = g_real_create_depth_stencil_state(device, desc, state);
    if (SUCCEEDED(hr) && state && *state && desc) {
        StateObjectInfo info;
        info.id = next_state_object_id();
        info.kind = "DepthStencilState";
        AcquireSRWLockExclusive(&g_state_lock);
        g_state_objects[*state] = info;
        ReleaseSRWLockExclusive(&g_state_lock);

        std::string out = event_prefix("ID3D11Device::CreateDepthStencilState");
        out += ",\"device\":\"" + trace::ptr_hex(device) + "\"";
        out += ",\"state\":\"" + trace::ptr_hex(*state) + "\"";
        out += ",\"state_id\":" + std::to_string(info.id);
        out += ",\"kind\":\"DepthStencilState\"";
        out += ",\"desc\":" + depth_stencil_desc_json(*desc);
        out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
        log_event(out);
    }
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_create_rasterizer_state(
    ID3D11Device *device,
    const D3D11_RASTERIZER_DESC *desc,
    ID3D11RasterizerState **state) {
    HRESULT hr = g_real_create_rasterizer_state(device, desc, state);
    if (SUCCEEDED(hr) && state && *state && desc) {
        StateObjectInfo info;
        info.id = next_state_object_id();
        info.kind = "RasterizerState";
        AcquireSRWLockExclusive(&g_state_lock);
        g_state_objects[*state] = info;
        ReleaseSRWLockExclusive(&g_state_lock);

        std::string out = event_prefix("ID3D11Device::CreateRasterizerState");
        out += ",\"device\":\"" + trace::ptr_hex(device) + "\"";
        out += ",\"state\":\"" + trace::ptr_hex(*state) + "\"";
        out += ",\"state_id\":" + std::to_string(info.id);
        out += ",\"kind\":\"RasterizerState\"";
        out += ",\"desc\":" + rasterizer_desc_json(*desc);
        out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
        log_event(out);
    }
    return hr;
}

void STDMETHODCALLTYPE traced_get_immediate_context(ID3D11Device *device, ID3D11DeviceContext **context) {
    g_real_get_immediate_context(device, context);
    if (context && *context) {
        patch_context_vtable(*context);
        std::string out = event_prefix("ID3D11Device::GetImmediateContext");
        out += ",\"device\":\"" + trace::ptr_hex(device) + "\",\"context\":\"" + trace::ptr_hex(*context) + "\"}";
        log_event(out);
    }
}

std::string shader_hash_for_ptr(void *shader) {
    if (!shader) {
        return "";
    }
    auto it = g_shaders.find(shader);
    return it == g_shaders.end() ? "" : it->second.hash;
}

void STDMETHODCALLTYPE traced_vs_set_shader(
    ID3D11DeviceContext *context,
    ID3D11VertexShader *shader,
    ID3D11ClassInstance *const *class_instances,
    UINT class_instances_count) {
    g_real_vs_set_shader(context, shader, class_instances, class_instances_count);
    std::string hash;
    AcquireSRWLockExclusive(&g_state_lock);
    hash = shader_hash_for_ptr(shader);
    g_contexts[context].vs_hash = hash;
    ReleaseSRWLockExclusive(&g_state_lock);

    std::string out = event_prefix("ID3D11DeviceContext::VSSetShader");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\",\"shader\":\"" + trace::ptr_hex(shader) + "\",\"bytecode_hash\":\"" + trace::json_escape(hash) + "\"}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_ps_set_shader(
    ID3D11DeviceContext *context,
    ID3D11PixelShader *shader,
    ID3D11ClassInstance *const *class_instances,
    UINT class_instances_count) {
    g_real_ps_set_shader(context, shader, class_instances, class_instances_count);
    std::string hash;
    AcquireSRWLockExclusive(&g_state_lock);
    hash = shader_hash_for_ptr(shader);
    g_contexts[context].ps_hash = hash;
    ReleaseSRWLockExclusive(&g_state_lock);

    std::string out = event_prefix("ID3D11DeviceContext::PSSetShader");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\",\"shader\":\"" + trace::ptr_hex(shader) + "\",\"bytecode_hash\":\"" + trace::json_escape(hash) + "\"}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_ps_set_shader_resources(
    ID3D11DeviceContext *context,
    UINT start_slot,
    UINT view_count,
    ID3D11ShaderResourceView *const *views) {
    g_real_ps_set_shader_resources(context, start_slot, view_count, views);

    std::vector<std::pair<UINT, std::string>> changed;
    AcquireSRWLockExclusive(&g_state_lock);
    auto &state = g_contexts[context];
    for (UINT i = 0; i < view_count; ++i) {
        UINT slot = start_slot + i;
        void *ptr = views ? views[i] : nullptr;
        std::string ref = ptr ? view_ref_for_ptr(ptr) : "";
        if (ref.empty()) {
            state.ps_srvs.erase(slot);
        } else {
            state.ps_srvs[slot] = ref;
        }
        changed.push_back({slot, ref});
    }
    ReleaseSRWLockExclusive(&g_state_lock);

    std::string out = event_prefix("ID3D11DeviceContext::PSSetShaderResources");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\",\"start_slot\":" + std::to_string(start_slot) + ",\"count\":" + std::to_string(view_count);
    out += ",\"views\":[";
    for (size_t i = 0; i < changed.size(); ++i) {
        if (i) {
            out += ",";
        }
        out += "{\"slot\":" + std::to_string(changed[i].first) + ",\"view\":\"" + trace::json_escape(changed[i].second) + "\"}";
    }
    out += "]}";
    log_event(out);
}

void log_buffer_bind_event(
    const char *event_name,
    ID3D11DeviceContext *context,
    UINT start_slot,
    UINT buffer_count,
    const std::vector<std::pair<UINT, std::string>> &changed) {
    std::string out = event_prefix(event_name);
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\",\"start_slot\":" + std::to_string(start_slot) + ",\"count\":" + std::to_string(buffer_count);
    out += ",\"buffers\":[";
    for (size_t i = 0; i < changed.size(); ++i) {
        if (i) {
            out += ",";
        }
        out += "{\"slot\":" + std::to_string(changed[i].first) + ",\"buffer\":\"" + trace::json_escape(changed[i].second) + "\"}";
    }
    out += "]}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_vs_set_constant_buffers(
    ID3D11DeviceContext *context,
    UINT start_slot,
    UINT buffer_count,
    ID3D11Buffer *const *buffers) {
    g_real_vs_set_constant_buffers(context, start_slot, buffer_count, buffers);

    std::vector<std::pair<UINT, std::string>> changed;
    AcquireSRWLockExclusive(&g_state_lock);
    auto &state = g_contexts[context];
    for (UINT i = 0; i < buffer_count; ++i) {
        UINT slot = start_slot + i;
        void *ptr = buffers ? buffers[i] : nullptr;
        std::string ref = ptr ? buffer_ref_for_ptr(ptr) : "";
        if (ref.empty()) {
            state.vs_cbuffers.erase(slot);
        } else {
            state.vs_cbuffers[slot] = ref;
        }
        changed.push_back({slot, ref});
    }
    ReleaseSRWLockExclusive(&g_state_lock);
    log_buffer_bind_event("ID3D11DeviceContext::VSSetConstantBuffers", context, start_slot, buffer_count, changed);
}

void STDMETHODCALLTYPE traced_ps_set_constant_buffers(
    ID3D11DeviceContext *context,
    UINT start_slot,
    UINT buffer_count,
    ID3D11Buffer *const *buffers) {
    g_real_ps_set_constant_buffers(context, start_slot, buffer_count, buffers);

    std::vector<std::pair<UINT, std::string>> changed;
    AcquireSRWLockExclusive(&g_state_lock);
    auto &state = g_contexts[context];
    for (UINT i = 0; i < buffer_count; ++i) {
        UINT slot = start_slot + i;
        void *ptr = buffers ? buffers[i] : nullptr;
        std::string ref = ptr ? buffer_ref_for_ptr(ptr) : "";
        if (ref.empty()) {
            state.ps_cbuffers.erase(slot);
        } else {
            state.ps_cbuffers[slot] = ref;
        }
        changed.push_back({slot, ref});
    }
    ReleaseSRWLockExclusive(&g_state_lock);
    log_buffer_bind_event("ID3D11DeviceContext::PSSetConstantBuffers", context, start_slot, buffer_count, changed);
}

void STDMETHODCALLTYPE traced_ps_set_samplers(
    ID3D11DeviceContext *context,
    UINT start_slot,
    UINT sampler_count,
    ID3D11SamplerState *const *samplers) {
    g_real_ps_set_samplers(context, start_slot, sampler_count, samplers);

    std::vector<std::pair<UINT, std::string>> changed;
    AcquireSRWLockExclusive(&g_state_lock);
    auto &state = g_contexts[context];
    for (UINT i = 0; i < sampler_count; ++i) {
        UINT slot = start_slot + i;
        void *ptr = samplers ? samplers[i] : nullptr;
        std::string ref = ptr ? state_object_ref_for_ptr(ptr) : "";
        if (ref.empty()) {
            state.ps_samplers.erase(slot);
        } else {
            state.ps_samplers[slot] = ref;
        }
        changed.push_back({slot, ref});
    }
    ReleaseSRWLockExclusive(&g_state_lock);

    std::string out = event_prefix("ID3D11DeviceContext::PSSetSamplers");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\",\"start_slot\":" + std::to_string(start_slot) + ",\"count\":" + std::to_string(sampler_count);
    out += ",\"samplers\":[";
    for (size_t i = 0; i < changed.size(); ++i) {
        if (i) {
            out += ",";
        }
        out += "{\"slot\":" + std::to_string(changed[i].first) + ",\"sampler\":\"" + trace::json_escape(changed[i].second) + "\"}";
    }
    out += "]}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_om_set_render_targets(
    ID3D11DeviceContext *context,
    UINT view_count,
    ID3D11RenderTargetView *const *rtvs,
    ID3D11DepthStencilView *dsv) {
    g_real_om_set_render_targets(context, view_count, rtvs, dsv);

    std::vector<std::string> rtv_refs;
    rtv_refs.reserve(view_count);
    std::string dsv_ref;
    AcquireSRWLockExclusive(&g_state_lock);
    auto &state = g_contexts[context];
    state.rtvs.clear();
    for (UINT i = 0; i < view_count; ++i) {
        void *ptr = rtvs ? rtvs[i] : nullptr;
        std::string ref = ptr ? view_ref_for_ptr(ptr) : "";
        state.rtvs.push_back(ref);
        rtv_refs.push_back(ref);
    }
    dsv_ref = dsv ? view_ref_for_ptr(dsv) : "";
    state.dsv = dsv_ref;
    ReleaseSRWLockExclusive(&g_state_lock);

    std::string out = event_prefix("ID3D11DeviceContext::OMSetRenderTargets");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\",\"count\":" + std::to_string(view_count);
    out += ",\"rtvs\":[";
    for (size_t i = 0; i < rtv_refs.size(); ++i) {
        if (i) {
            out += ",";
        }
        out += "\"" + trace::json_escape(rtv_refs[i]) + "\"";
    }
    out += "],\"dsv\":\"" + trace::json_escape(dsv_ref) + "\"}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_om_set_render_targets_and_uavs(
    ID3D11DeviceContext *context,
    UINT view_count,
    ID3D11RenderTargetView *const *rtvs,
    ID3D11DepthStencilView *dsv,
    UINT uav_start_slot,
    UINT uav_count,
    ID3D11UnorderedAccessView *const *uavs,
    const UINT *initial_counts) {
    g_real_om_set_render_targets_and_uavs(context, view_count, rtvs, dsv, uav_start_slot, uav_count, uavs, initial_counts);

    std::vector<std::string> rtv_refs;
    std::string dsv_ref;
    const bool keep_rt_and_dsv = view_count == D3D11_KEEP_RENDER_TARGETS_AND_DEPTH_STENCIL;

    AcquireSRWLockExclusive(&g_state_lock);
    auto &state = g_contexts[context];
    if (!keep_rt_and_dsv) {
        state.rtvs.clear();
        rtv_refs.reserve(view_count);
        for (UINT i = 0; i < view_count; ++i) {
            void *ptr = rtvs ? rtvs[i] : nullptr;
            std::string ref = ptr ? view_ref_for_ptr(ptr) : "";
            state.rtvs.push_back(ref);
            rtv_refs.push_back(ref);
        }
        dsv_ref = dsv ? view_ref_for_ptr(dsv) : "";
        state.dsv = dsv_ref;
    } else {
        rtv_refs = state.rtvs;
        dsv_ref = state.dsv;
    }
    ReleaseSRWLockExclusive(&g_state_lock);

    std::string out = event_prefix("ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\"";
    out += ",\"count\":" + std::to_string(view_count);
    out += ",\"keep_rt_and_dsv\":";
    out += keep_rt_and_dsv ? "true" : "false";
    out += ",\"rtvs\":[";
    for (size_t i = 0; i < rtv_refs.size(); ++i) {
        if (i) {
            out += ",";
        }
        out += "\"" + trace::json_escape(rtv_refs[i]) + "\"";
    }
    out += "],\"dsv\":\"" + trace::json_escape(dsv_ref) + "\"";
    out += ",\"uav_start_slot\":" + std::to_string(uav_start_slot);
    out += ",\"uav_count\":" + std::to_string(uav_count);
    out += ",\"uavs\":[";
    const bool keep_uavs = uav_start_slot == D3D11_KEEP_UNORDERED_ACCESS_VIEWS;
    if (!keep_uavs) {
        for (UINT i = 0; i < uav_count; ++i) {
            if (i) {
                out += ",";
            }
            out += "\"";
            out += trace::ptr_hex(uavs ? uavs[i] : nullptr);
            out += "\"";
        }
    }
    out += "],\"keep_uavs\":";
    out += keep_uavs ? "true" : "false";
    out += ",\"has_initial_counts\":";
    out += initial_counts ? "true" : "false";
    out += "}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_om_set_blend_state(
    ID3D11DeviceContext *context,
    ID3D11BlendState *blend_state,
    const FLOAT blend_factor[4],
    UINT sample_mask) {
    g_real_om_set_blend_state(context, blend_state, blend_factor, sample_mask);

    std::string ref = blend_state ? state_object_ref_for_ptr(blend_state) : "";
    FLOAT factor[4] = {0.0f, 0.0f, 0.0f, 0.0f};
    if (blend_factor) {
        factor[0] = blend_factor[0];
        factor[1] = blend_factor[1];
        factor[2] = blend_factor[2];
        factor[3] = blend_factor[3];
    }
    AcquireSRWLockExclusive(&g_state_lock);
    auto &state = g_contexts[context];
    state.blend_state = ref;
    state.blend_sample_mask = sample_mask;
    for (int i = 0; i < 4; ++i) {
        state.blend_factor[i] = factor[i];
    }
    ReleaseSRWLockExclusive(&g_state_lock);

    std::string out = event_prefix("ID3D11DeviceContext::OMSetBlendState");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\",\"state\":\"" + trace::json_escape(ref) + "\"";
    out += ",\"sample_mask\":" + std::to_string(sample_mask);
    out += ",\"blend_factor\":[" + std::to_string(factor[0]) + "," + std::to_string(factor[1]) + "," + std::to_string(factor[2]) + "," + std::to_string(factor[3]) + "]}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_om_set_depth_stencil_state(
    ID3D11DeviceContext *context,
    ID3D11DepthStencilState *depth_stencil_state,
    UINT stencil_ref) {
    g_real_om_set_depth_stencil_state(context, depth_stencil_state, stencil_ref);

    std::string ref = depth_stencil_state ? state_object_ref_for_ptr(depth_stencil_state) : "";
    AcquireSRWLockExclusive(&g_state_lock);
    auto &state = g_contexts[context];
    state.depth_stencil_state = ref;
    state.stencil_ref = stencil_ref;
    ReleaseSRWLockExclusive(&g_state_lock);

    std::string out = event_prefix("ID3D11DeviceContext::OMSetDepthStencilState");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\",\"state\":\"" + trace::json_escape(ref) + "\"";
    out += ",\"stencil_ref\":" + std::to_string(stencil_ref) + "}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_rs_set_state(
    ID3D11DeviceContext *context,
    ID3D11RasterizerState *rasterizer_state) {
    g_real_rs_set_state(context, rasterizer_state);

    std::string ref = rasterizer_state ? state_object_ref_for_ptr(rasterizer_state) : "";
    AcquireSRWLockExclusive(&g_state_lock);
    g_contexts[context].rasterizer_state = ref;
    ReleaseSRWLockExclusive(&g_state_lock);

    std::string out = event_prefix("ID3D11DeviceContext::RSSetState");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\",\"state\":\"" + trace::json_escape(ref) + "\"}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_draw_indexed(ID3D11DeviceContext *context, UINT index_count, UINT start_index, INT base_vertex) {
    g_real_draw_indexed(context, index_count, start_index, base_vertex);
    log_context_state_draw(
        context,
        "ID3D11DeviceContext::DrawIndexed",
        ",\"index_count\":" + std::to_string(index_count) + ",\"start_index\":" + std::to_string(start_index) + ",\"base_vertex\":" + std::to_string(base_vertex));
}

void STDMETHODCALLTYPE traced_draw(ID3D11DeviceContext *context, UINT vertex_count, UINT start_vertex) {
    g_real_draw(context, vertex_count, start_vertex);
    log_context_state_draw(
        context,
        "ID3D11DeviceContext::Draw",
        ",\"vertex_count\":" + std::to_string(vertex_count) + ",\"start_vertex\":" + std::to_string(start_vertex));
}

void STDMETHODCALLTYPE traced_draw_indexed_instanced(
    ID3D11DeviceContext *context,
    UINT index_count_per_instance,
    UINT instance_count,
    UINT start_index,
    INT base_vertex,
    UINT start_instance) {
    g_real_draw_indexed_instanced(context, index_count_per_instance, instance_count, start_index, base_vertex, start_instance);
    log_context_state_draw(
        context,
        "ID3D11DeviceContext::DrawIndexedInstanced",
        ",\"index_count_per_instance\":" + std::to_string(index_count_per_instance) +
            ",\"instance_count\":" + std::to_string(instance_count) +
            ",\"start_index\":" + std::to_string(start_index) +
            ",\"base_vertex\":" + std::to_string(base_vertex) +
            ",\"start_instance\":" + std::to_string(start_instance));
}

void STDMETHODCALLTYPE traced_draw_instanced(
    ID3D11DeviceContext *context,
    UINT vertex_count_per_instance,
    UINT instance_count,
    UINT start_vertex,
    UINT start_instance) {
    g_real_draw_instanced(context, vertex_count_per_instance, instance_count, start_vertex, start_instance);
    log_context_state_draw(
        context,
        "ID3D11DeviceContext::DrawInstanced",
        ",\"vertex_count_per_instance\":" + std::to_string(vertex_count_per_instance) +
            ",\"instance_count\":" + std::to_string(instance_count) +
            ",\"start_vertex\":" + std::to_string(start_vertex) +
            ",\"start_instance\":" + std::to_string(start_instance));
}

void STDMETHODCALLTYPE traced_draw_auto(ID3D11DeviceContext *context) {
    g_real_draw_auto(context);
    log_context_state_draw(context, "ID3D11DeviceContext::DrawAuto", "");
}

void STDMETHODCALLTYPE traced_draw_indexed_instanced_indirect(
    ID3D11DeviceContext *context,
    ID3D11Buffer *buffer_for_args,
    UINT aligned_byte_offset_for_args) {
    g_real_draw_indexed_instanced_indirect(context, buffer_for_args, aligned_byte_offset_for_args);
    log_context_state_draw(
        context,
        "ID3D11DeviceContext::DrawIndexedInstancedIndirect",
        ",\"args_buffer\":\"" + trace::ptr_hex(buffer_for_args) +
            "\",\"aligned_byte_offset_for_args\":" + std::to_string(aligned_byte_offset_for_args));
}

void STDMETHODCALLTYPE traced_draw_instanced_indirect(
    ID3D11DeviceContext *context,
    ID3D11Buffer *buffer_for_args,
    UINT aligned_byte_offset_for_args) {
    g_real_draw_instanced_indirect(context, buffer_for_args, aligned_byte_offset_for_args);
    log_context_state_draw(
        context,
        "ID3D11DeviceContext::DrawInstancedIndirect",
        ",\"args_buffer\":\"" + trace::ptr_hex(buffer_for_args) +
            "\",\"aligned_byte_offset_for_args\":" + std::to_string(aligned_byte_offset_for_args));
}

HRESULT STDMETHODCALLTYPE traced_map(
    ID3D11DeviceContext *context,
    ID3D11Resource *resource,
    UINT subresource,
    D3D11_MAP map_type,
    UINT map_flags,
    D3D11_MAPPED_SUBRESOURCE *mapped_resource) {
    HRESULT hr = g_real_map(context, resource, subresource, map_type, map_flags, mapped_resource);

    size_t estimated_size = 0;
    if (SUCCEEDED(hr) && mapped_resource && mapped_resource->pData) {
        AcquireSRWLockShared(&g_state_lock);
        estimated_size = is_constant_buffer_resource(resource) ? estimate_resource_data_size(resource, mapped_resource->RowPitch, nullptr) : 0;
        ReleaseSRWLockShared(&g_state_lock);

        MappedInfo info;
        info.resource = resource;
        info.subresource = subresource;
        info.data = mapped_resource->pData;
        info.row_pitch = mapped_resource->RowPitch;
        info.depth_pitch = mapped_resource->DepthPitch;
        info.estimated_size = estimated_size;
        info.map_type = static_cast<UINT>(map_type);

        AcquireSRWLockExclusive(&g_state_lock);
        g_mapped_resources[mapped_key(context, resource, subresource)] = info;
        ReleaseSRWLockExclusive(&g_state_lock);
    }

    std::string out = event_prefix("ID3D11DeviceContext::Map");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\"";
    out += ",\"resource\":" + resource_ref_json(resource);
    out += ",\"subresource\":" + std::to_string(subresource);
    out += ",\"map_type\":" + std::to_string(static_cast<UINT>(map_type));
    out += ",\"map_flags\":" + std::to_string(map_flags);
    out += ",\"row_pitch\":";
    out += (SUCCEEDED(hr) && mapped_resource) ? std::to_string(mapped_resource->RowPitch) : "0";
    out += ",\"depth_pitch\":";
    out += (SUCCEEDED(hr) && mapped_resource) ? std::to_string(mapped_resource->DepthPitch) : "0";
    out += ",\"estimated_size\":" + std::to_string(estimated_size);
    out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
    log_event(out);
    return hr;
}

void STDMETHODCALLTYPE traced_unmap(ID3D11DeviceContext *context, ID3D11Resource *resource, UINT subresource) {
    MappedInfo info;
    bool had_info = false;
    const std::string key = mapped_key(context, resource, subresource);
    AcquireSRWLockExclusive(&g_state_lock);
    auto it = g_mapped_resources.find(key);
    if (it != g_mapped_resources.end()) {
        info = it->second;
        had_info = true;
        g_mapped_resources.erase(it);
    }
    ReleaseSRWLockExclusive(&g_state_lock);

    std::string digest = "{}";
    if (had_info && info.data && info.estimated_size > 0 && is_constant_buffer_resource(resource)) {
        digest = data_digest_json(info.data, info.estimated_size, true);
    }

    g_real_unmap(context, resource, subresource);

    std::string out = event_prefix("ID3D11DeviceContext::Unmap");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\"";
    out += ",\"resource\":" + resource_ref_json(resource);
    out += ",\"subresource\":" + std::to_string(subresource);
    out += ",\"had_map\":";
    out += had_info ? "true" : "false";
    out += ",\"map_type\":" + std::to_string(had_info ? info.map_type : 0);
    out += ",\"row_pitch\":" + std::to_string(had_info ? info.row_pitch : 0);
    out += ",\"depth_pitch\":" + std::to_string(had_info ? info.depth_pitch : 0);
    out += ",\"digest\":" + digest;
    out += "}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_update_subresource(
    ID3D11DeviceContext *context,
    ID3D11Resource *dst_resource,
    UINT dst_subresource,
    const D3D11_BOX *dst_box,
    const void *src_data,
    UINT src_row_pitch,
    UINT src_depth_pitch) {
    size_t estimated_size = 0;
    bool should_digest = false;
    AcquireSRWLockShared(&g_state_lock);
    should_digest = is_constant_buffer_resource(dst_resource);
    estimated_size = should_digest ? estimate_resource_data_size(dst_resource, src_row_pitch, dst_box) : 0;
    ReleaseSRWLockShared(&g_state_lock);

    std::string digest = src_data && estimated_size > 0 && should_digest ? data_digest_json(src_data, estimated_size, true) : "{}";
    g_real_update_subresource(context, dst_resource, dst_subresource, dst_box, src_data, src_row_pitch, src_depth_pitch);

    std::string out = event_prefix("ID3D11DeviceContext::UpdateSubresource");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\"";
    out += ",\"dst\":" + resource_ref_json(dst_resource);
    out += ",\"dst_subresource\":" + std::to_string(dst_subresource);
    out += ",\"dst_box\":" + box_json(dst_box);
    out += ",\"src_row_pitch\":" + std::to_string(src_row_pitch);
    out += ",\"src_depth_pitch\":" + std::to_string(src_depth_pitch);
    out += ",\"digest\":" + digest;
    out += "}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_copy_subresource_region(
    ID3D11DeviceContext *context,
    ID3D11Resource *dst_resource,
    UINT dst_subresource,
    UINT dst_x,
    UINT dst_y,
    UINT dst_z,
    ID3D11Resource *src_resource,
    UINT src_subresource,
    const D3D11_BOX *src_box) {
    g_real_copy_subresource_region(context, dst_resource, dst_subresource, dst_x, dst_y, dst_z, src_resource, src_subresource, src_box);

    std::string out = event_prefix("ID3D11DeviceContext::CopySubresourceRegion");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\"";
    out += ",\"dst\":" + resource_ref_json(dst_resource);
    out += ",\"dst_subresource\":" + std::to_string(dst_subresource);
    out += ",\"dst_x\":" + std::to_string(dst_x);
    out += ",\"dst_y\":" + std::to_string(dst_y);
    out += ",\"dst_z\":" + std::to_string(dst_z);
    out += ",\"src\":" + resource_ref_json(src_resource);
    out += ",\"src_subresource\":" + std::to_string(src_subresource);
    out += ",\"src_box\":" + box_json(src_box);
    out += "}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_copy_resource(ID3D11DeviceContext *context, ID3D11Resource *dst_resource, ID3D11Resource *src_resource) {
    g_real_copy_resource(context, dst_resource, src_resource);

    std::string out = event_prefix("ID3D11DeviceContext::CopyResource");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\"";
    out += ",\"dst\":" + resource_ref_json(dst_resource);
    out += ",\"src\":" + resource_ref_json(src_resource);
    out += "}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_clear_rtv(ID3D11DeviceContext *context, ID3D11RenderTargetView *view, const FLOAT color_rgba[4]) {
    FLOAT c[4] = {0, 0, 0, 0};
    if (color_rgba) {
        c[0] = color_rgba[0];
        c[1] = color_rgba[1];
        c[2] = color_rgba[2];
        c[3] = color_rgba[3];
    }
    g_real_clear_rtv(context, view, color_rgba);

    std::string out = event_prefix("ID3D11DeviceContext::ClearRenderTargetView");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\"";
    out += ",\"view\":" + view_json_for_ptr(view);
    out += ",\"color\":[" + std::to_string(c[0]) + "," + std::to_string(c[1]) + "," + std::to_string(c[2]) + "," + std::to_string(c[3]) + "]";
    out += "}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_clear_dsv(
    ID3D11DeviceContext *context,
    ID3D11DepthStencilView *view,
    UINT clear_flags,
    FLOAT depth,
    UINT8 stencil) {
    g_real_clear_dsv(context, view, clear_flags, depth, stencil);

    std::string out = event_prefix("ID3D11DeviceContext::ClearDepthStencilView");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\"";
    out += ",\"view\":" + view_json_for_ptr(view);
    out += ",\"clear_flags\":" + std::to_string(clear_flags);
    out += ",\"depth\":" + std::to_string(depth);
    out += ",\"stencil\":" + std::to_string(static_cast<unsigned>(stencil));
    out += "}";
    log_event(out);
}

void STDMETHODCALLTYPE traced_resolve_subresource(
    ID3D11DeviceContext *context,
    ID3D11Resource *dst_resource,
    UINT dst_subresource,
    ID3D11Resource *src_resource,
    UINT src_subresource,
    DXGI_FORMAT format) {
    g_real_resolve_subresource(context, dst_resource, dst_subresource, src_resource, src_subresource, format);

    std::string out = event_prefix("ID3D11DeviceContext::ResolveSubresource");
    out += ",\"context\":\"" + trace::ptr_hex(context) + "\"";
    out += ",\"dst\":" + resource_ref_json(dst_resource);
    out += ",\"dst_subresource\":" + std::to_string(dst_subresource);
    out += ",\"src\":" + resource_ref_json(src_resource);
    out += ",\"src_subresource\":" + std::to_string(src_subresource);
    out += ",\"format\":\"" + format_name(format) + "\"";
    out += ",\"format_id\":" + std::to_string(static_cast<unsigned>(format));
    out += "}";
    log_event(out);
}

void patch_context_vtable(ID3D11DeviceContext *context) {
    if (!context) {
        return;
    }
    AcquireSRWLockExclusive(&g_patch_lock);
    auto **vtable = *reinterpret_cast<void ***>(context);
    if (vtable) {
        patch_slot(vtable, kVSSetConstantBuffersSlot, reinterpret_cast<void *>(&traced_vs_set_constant_buffers), reinterpret_cast<void **>(&g_real_vs_set_constant_buffers));
        patch_slot(vtable, kPSSetShaderResourcesSlot, reinterpret_cast<void *>(&traced_ps_set_shader_resources), reinterpret_cast<void **>(&g_real_ps_set_shader_resources));
        patch_slot(vtable, kPSSetShaderSlot, reinterpret_cast<void *>(&traced_ps_set_shader), reinterpret_cast<void **>(&g_real_ps_set_shader));
        patch_slot(vtable, kPSSetSamplersSlot, reinterpret_cast<void *>(&traced_ps_set_samplers), reinterpret_cast<void **>(&g_real_ps_set_samplers));
        patch_slot(vtable, kVSSetShaderSlot, reinterpret_cast<void *>(&traced_vs_set_shader), reinterpret_cast<void **>(&g_real_vs_set_shader));
        patch_slot(vtable, kMapSlot, reinterpret_cast<void *>(&traced_map), reinterpret_cast<void **>(&g_real_map));
        patch_slot(vtable, kUnmapSlot, reinterpret_cast<void *>(&traced_unmap), reinterpret_cast<void **>(&g_real_unmap));
        patch_slot(vtable, kPSSetConstantBuffersSlot, reinterpret_cast<void *>(&traced_ps_set_constant_buffers), reinterpret_cast<void **>(&g_real_ps_set_constant_buffers));
        patch_slot(vtable, kDrawIndexedSlot, reinterpret_cast<void *>(&traced_draw_indexed), reinterpret_cast<void **>(&g_real_draw_indexed));
        patch_slot(vtable, kDrawSlot, reinterpret_cast<void *>(&traced_draw), reinterpret_cast<void **>(&g_real_draw));
        patch_slot(vtable, kDrawIndexedInstancedSlot, reinterpret_cast<void *>(&traced_draw_indexed_instanced), reinterpret_cast<void **>(&g_real_draw_indexed_instanced));
        patch_slot(vtable, kDrawInstancedSlot, reinterpret_cast<void *>(&traced_draw_instanced), reinterpret_cast<void **>(&g_real_draw_instanced));
        patch_slot(vtable, kOMSetRenderTargetsSlot, reinterpret_cast<void *>(&traced_om_set_render_targets), reinterpret_cast<void **>(&g_real_om_set_render_targets));
        patch_slot(vtable, kOMSetRenderTargetsAndUnorderedAccessViewsSlot, reinterpret_cast<void *>(&traced_om_set_render_targets_and_uavs), reinterpret_cast<void **>(&g_real_om_set_render_targets_and_uavs));
        patch_slot(vtable, kOMSetBlendStateSlot, reinterpret_cast<void *>(&traced_om_set_blend_state), reinterpret_cast<void **>(&g_real_om_set_blend_state));
        patch_slot(vtable, kOMSetDepthStencilStateSlot, reinterpret_cast<void *>(&traced_om_set_depth_stencil_state), reinterpret_cast<void **>(&g_real_om_set_depth_stencil_state));
        patch_slot(vtable, kDrawAutoSlot, reinterpret_cast<void *>(&traced_draw_auto), reinterpret_cast<void **>(&g_real_draw_auto));
        patch_slot(vtable, kDrawIndexedInstancedIndirectSlot, reinterpret_cast<void *>(&traced_draw_indexed_instanced_indirect), reinterpret_cast<void **>(&g_real_draw_indexed_instanced_indirect));
        patch_slot(vtable, kDrawInstancedIndirectSlot, reinterpret_cast<void *>(&traced_draw_instanced_indirect), reinterpret_cast<void **>(&g_real_draw_instanced_indirect));
        patch_slot(vtable, kRSSetStateSlot, reinterpret_cast<void *>(&traced_rs_set_state), reinterpret_cast<void **>(&g_real_rs_set_state));
        patch_slot(vtable, kCopySubresourceRegionSlot, reinterpret_cast<void *>(&traced_copy_subresource_region), reinterpret_cast<void **>(&g_real_copy_subresource_region));
        patch_slot(vtable, kCopyResourceSlot, reinterpret_cast<void *>(&traced_copy_resource), reinterpret_cast<void **>(&g_real_copy_resource));
        patch_slot(vtable, kUpdateSubresourceSlot, reinterpret_cast<void *>(&traced_update_subresource), reinterpret_cast<void **>(&g_real_update_subresource));
        patch_slot(vtable, kClearRenderTargetViewSlot, reinterpret_cast<void *>(&traced_clear_rtv), reinterpret_cast<void **>(&g_real_clear_rtv));
        patch_slot(vtable, kClearDepthStencilViewSlot, reinterpret_cast<void *>(&traced_clear_dsv), reinterpret_cast<void **>(&g_real_clear_dsv));
        patch_slot(vtable, kResolveSubresourceSlot, reinterpret_cast<void *>(&traced_resolve_subresource), reinterpret_cast<void **>(&g_real_resolve_subresource));
    }
    ReleaseSRWLockExclusive(&g_patch_lock);
}

void patch_device_vtable(ID3D11Device *device) {
    if (!device) {
        return;
    }

    AcquireSRWLockExclusive(&g_patch_lock);
    auto **vtable = *reinterpret_cast<void ***>(device);
    if (vtable) {
        patch_slot(vtable, kCreateBufferSlot, reinterpret_cast<void *>(&traced_create_buffer), reinterpret_cast<void **>(&g_real_create_buffer));
        patch_slot(vtable, kCreateTexture2DSlot, reinterpret_cast<void *>(&traced_create_texture2d), reinterpret_cast<void **>(&g_real_create_texture2d));
        patch_slot(vtable, kCreateShaderResourceViewSlot, reinterpret_cast<void *>(&traced_create_srv), reinterpret_cast<void **>(&g_real_create_srv));
        patch_slot(vtable, kCreateRenderTargetViewSlot, reinterpret_cast<void *>(&traced_create_rtv), reinterpret_cast<void **>(&g_real_create_rtv));
        patch_slot(vtable, kCreateDepthStencilViewSlot, reinterpret_cast<void *>(&traced_create_dsv), reinterpret_cast<void **>(&g_real_create_dsv));
        patch_slot(vtable, kCreateVertexShaderSlot, reinterpret_cast<void *>(&traced_create_vertex_shader), reinterpret_cast<void **>(&g_real_create_vertex_shader));
        patch_slot(vtable, kCreatePixelShaderSlot, reinterpret_cast<void *>(&traced_create_pixel_shader), reinterpret_cast<void **>(&g_real_create_pixel_shader));
        patch_slot(vtable, kCreateBlendStateSlot, reinterpret_cast<void *>(&traced_create_blend_state), reinterpret_cast<void **>(&g_real_create_blend_state));
        patch_slot(vtable, kCreateDepthStencilStateSlot, reinterpret_cast<void *>(&traced_create_depth_stencil_state), reinterpret_cast<void **>(&g_real_create_depth_stencil_state));
        patch_slot(vtable, kCreateRasterizerStateSlot, reinterpret_cast<void *>(&traced_create_rasterizer_state), reinterpret_cast<void **>(&g_real_create_rasterizer_state));
        patch_slot(vtable, kCreateSamplerStateSlot, reinterpret_cast<void *>(&traced_create_sampler_state), reinterpret_cast<void **>(&g_real_create_sampler_state));
        patch_slot(vtable, kGetImmediateContextSlot, reinterpret_cast<void *>(&traced_get_immediate_context), reinterpret_cast<void **>(&g_real_get_immediate_context));
    }
    ReleaseSRWLockExclusive(&g_patch_lock);
}

} // namespace

extern "C" __declspec(dllexport) HRESULT WINAPI D3D11CreateDevice(
    IDXGIAdapter *adapter,
    D3D_DRIVER_TYPE driver_type,
    HMODULE software,
    UINT flags,
    const D3D_FEATURE_LEVEL *feature_levels,
    UINT feature_levels_count,
    UINT sdk_version,
    ID3D11Device **device,
    D3D_FEATURE_LEVEL *feature_level,
    ID3D11DeviceContext **immediate_context) {
    auto fn = real_d3d11_create_device();
    if (!fn) {
        return E_FAIL;
    }

    HRESULT hr = fn(
        adapter,
        driver_type,
        software,
        flags,
        feature_levels,
        feature_levels_count,
        sdk_version,
        device,
        feature_level,
        immediate_context);

    trace::append_json_object(
        trace::trace_file_path(L"shader_compiles.json"),
        std::string("{\"event\":\"D3D11CreateDevice\",\"hr\":\"") + trace::hr_hex(hr) + "\"}");

    std::string out = event_prefix("D3D11CreateDevice");
    out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"";
    out += ",\"device\":\"" + trace::ptr_hex(device && *device ? *device : nullptr) + "\"";
    out += ",\"immediate_context\":\"" + trace::ptr_hex(immediate_context && *immediate_context ? *immediate_context : nullptr) + "\"";
    out += ",\"flags\":" + std::to_string(flags);
    out += "}";
    log_event(out);

    if (SUCCEEDED(hr) && device && *device) {
        patch_device_vtable(*device);
    }
    if (SUCCEEDED(hr) && immediate_context && *immediate_context) {
        patch_context_vtable(*immediate_context);
    }

    return hr;
}

BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID reserved) {
    (void)instance;
    (void)reserved;
    if (reason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(instance);
    } else if (reason == DLL_PROCESS_DETACH) {
        trace::flush_trace_files();
    }
    return TRUE;
}
