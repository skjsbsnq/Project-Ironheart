#include "trace_common.h"

#include <dxgi1_3.h>

#include <cstdint>
#include <string>
#include <unordered_map>

using CreateDXGIFactory1Fn = HRESULT(WINAPI *)(REFIID, void **);
using CreateDXGIFactory2Fn = HRESULT(WINAPI *)(UINT, REFIID, void **);

using CreateSwapChainFn = HRESULT(STDMETHODCALLTYPE *)(
    IDXGIFactory *,
    IUnknown *,
    DXGI_SWAP_CHAIN_DESC *,
    IDXGISwapChain **);

using CreateSwapChainForHwndFn = HRESULT(STDMETHODCALLTYPE *)(
    IDXGIFactory2 *,
    IUnknown *,
    HWND,
    const DXGI_SWAP_CHAIN_DESC1 *,
    const DXGI_SWAP_CHAIN_FULLSCREEN_DESC *,
    IDXGIOutput *,
    IDXGISwapChain1 **);

using PresentFn = HRESULT(STDMETHODCALLTYPE *)(IDXGISwapChain *, UINT, UINT);

namespace {

constexpr size_t kFactoryCreateSwapChainSlot = 10;
constexpr size_t kFactory2CreateSwapChainForHwndSlot = 15;
constexpr size_t kSwapChainPresentSlot = 8;

SRWLOCK g_patch_lock = SRWLOCK_INIT;
SRWLOCK g_state_lock = SRWLOCK_INIT;

CreateSwapChainFn g_real_create_swap_chain = nullptr;
CreateSwapChainForHwndFn g_real_create_swap_chain_for_hwnd = nullptr;
PresentFn g_real_present = nullptr;
volatile LONG64 g_event_seq = 0;
volatile LONG64 g_present_seq = 0;

std::unordered_map<void *, uint64_t> g_swapchain_ids;
volatile LONG64 g_swapchain_id = 0;

HRESULT STDMETHODCALLTYPE traced_create_swap_chain(
    IDXGIFactory *factory,
    IUnknown *device,
    DXGI_SWAP_CHAIN_DESC *desc,
    IDXGISwapChain **swapchain);

HRESULT STDMETHODCALLTYPE traced_create_swap_chain_for_hwnd(
    IDXGIFactory2 *factory,
    IUnknown *device,
    HWND hwnd,
    const DXGI_SWAP_CHAIN_DESC1 *desc,
    const DXGI_SWAP_CHAIN_FULLSCREEN_DESC *fullscreen_desc,
    IDXGIOutput *restrict_output,
    IDXGISwapChain1 **swapchain);

HRESULT STDMETHODCALLTYPE traced_present(IDXGISwapChain *swapchain, UINT sync_interval, UINT flags);

CreateDXGIFactory1Fn real_create_dxgi_factory1() {
    static CreateDXGIFactory1Fn fn =
        reinterpret_cast<CreateDXGIFactory1Fn>(trace::get_system_proc("dxgi.dll", "CreateDXGIFactory1"));
    return fn;
}

CreateDXGIFactory2Fn real_create_dxgi_factory2() {
    static CreateDXGIFactory2Fn fn =
        reinterpret_cast<CreateDXGIFactory2Fn>(trace::get_system_proc("dxgi.dll", "CreateDXGIFactory2"));
    return fn;
}

std::wstring runtime_trace_path() {
    return trace::trace_file_path(L"logs\\d3d11_trace.jsonl");
}

uint64_t next_seq() {
    return static_cast<uint64_t>(InterlockedIncrement64(&g_event_seq));
}

uint64_t next_swapchain_id() {
    return static_cast<uint64_t>(InterlockedIncrement64(&g_swapchain_id));
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

uint64_t swapchain_id_for_ptr(void *swapchain) {
    if (!swapchain) {
        return 0;
    }
    auto it = g_swapchain_ids.find(swapchain);
    if (it != g_swapchain_ids.end()) {
        return it->second;
    }
    uint64_t id = next_swapchain_id();
    g_swapchain_ids[swapchain] = id;
    return id;
}

void patch_swapchain_vtable(IDXGISwapChain *swapchain) {
    if (!swapchain) {
        return;
    }
    AcquireSRWLockExclusive(&g_patch_lock);
    auto **vtable = *reinterpret_cast<void ***>(swapchain);
    if (vtable) {
        patch_slot(vtable, kSwapChainPresentSlot, reinterpret_cast<void *>(&traced_present), reinterpret_cast<void **>(&g_real_present));
    }
    ReleaseSRWLockExclusive(&g_patch_lock);
}

void patch_factory_vtable(void *factory) {
    if (!factory) {
        return;
    }
    AcquireSRWLockExclusive(&g_patch_lock);
    auto **vtable = *reinterpret_cast<void ***>(factory);
    if (vtable) {
        patch_slot(vtable, kFactoryCreateSwapChainSlot, reinterpret_cast<void *>(&traced_create_swap_chain), reinterpret_cast<void **>(&g_real_create_swap_chain));
        patch_slot(vtable, kFactory2CreateSwapChainForHwndSlot, reinterpret_cast<void *>(&traced_create_swap_chain_for_hwnd), reinterpret_cast<void **>(&g_real_create_swap_chain_for_hwnd));
    }
    ReleaseSRWLockExclusive(&g_patch_lock);
}

std::string swapchain_desc_json(const DXGI_SWAP_CHAIN_DESC &desc) {
    std::string out = "{";
    out += "\"width\":" + std::to_string(desc.BufferDesc.Width) + ",";
    out += "\"height\":" + std::to_string(desc.BufferDesc.Height) + ",";
    out += "\"format_id\":" + std::to_string(static_cast<unsigned>(desc.BufferDesc.Format)) + ",";
    out += "\"refresh_numerator\":" + std::to_string(desc.BufferDesc.RefreshRate.Numerator) + ",";
    out += "\"refresh_denominator\":" + std::to_string(desc.BufferDesc.RefreshRate.Denominator) + ",";
    out += "\"sample_count\":" + std::to_string(desc.SampleDesc.Count) + ",";
    out += "\"buffer_usage\":" + std::to_string(desc.BufferUsage) + ",";
    out += "\"buffer_count\":" + std::to_string(desc.BufferCount) + ",";
    out += "\"windowed\":";
    out += desc.Windowed ? "true" : "false";
    out += ",\"swap_effect\":" + std::to_string(static_cast<unsigned>(desc.SwapEffect));
    out += "}";
    return out;
}

std::string swapchain_desc1_json(const DXGI_SWAP_CHAIN_DESC1 &desc) {
    std::string out = "{";
    out += "\"width\":" + std::to_string(desc.Width) + ",";
    out += "\"height\":" + std::to_string(desc.Height) + ",";
    out += "\"format_id\":" + std::to_string(static_cast<unsigned>(desc.Format)) + ",";
    out += "\"sample_count\":" + std::to_string(desc.SampleDesc.Count) + ",";
    out += "\"buffer_usage\":" + std::to_string(desc.BufferUsage) + ",";
    out += "\"buffer_count\":" + std::to_string(desc.BufferCount) + ",";
    out += "\"scaling\":" + std::to_string(static_cast<unsigned>(desc.Scaling)) + ",";
    out += "\"swap_effect\":" + std::to_string(static_cast<unsigned>(desc.SwapEffect)) + ",";
    out += "\"alpha_mode\":" + std::to_string(static_cast<unsigned>(desc.AlphaMode)) + ",";
    out += "\"flags\":" + std::to_string(desc.Flags);
    out += "}";
    return out;
}

HRESULT STDMETHODCALLTYPE traced_create_swap_chain(
    IDXGIFactory *factory,
    IUnknown *device,
    DXGI_SWAP_CHAIN_DESC *desc,
    IDXGISwapChain **swapchain) {
    HRESULT hr = g_real_create_swap_chain(factory, device, desc, swapchain);
    uint64_t id = 0;
    if (SUCCEEDED(hr) && swapchain && *swapchain) {
        patch_swapchain_vtable(*swapchain);
        AcquireSRWLockExclusive(&g_state_lock);
        id = swapchain_id_for_ptr(*swapchain);
        ReleaseSRWLockExclusive(&g_state_lock);
    }
    std::string out = event_prefix("IDXGIFactory::CreateSwapChain");
    out += ",\"factory\":\"" + trace::ptr_hex(factory) + "\"";
    out += ",\"device\":\"" + trace::ptr_hex(device) + "\"";
    out += ",\"swapchain\":\"" + trace::ptr_hex(swapchain && *swapchain ? *swapchain : nullptr) + "\"";
    out += ",\"swapchain_id\":" + std::to_string(id);
    out += ",\"desc\":";
    out += desc ? swapchain_desc_json(*desc) : "null";
    out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
    log_event(out);
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_create_swap_chain_for_hwnd(
    IDXGIFactory2 *factory,
    IUnknown *device,
    HWND hwnd,
    const DXGI_SWAP_CHAIN_DESC1 *desc,
    const DXGI_SWAP_CHAIN_FULLSCREEN_DESC *fullscreen_desc,
    IDXGIOutput *restrict_output,
    IDXGISwapChain1 **swapchain) {
    HRESULT hr = g_real_create_swap_chain_for_hwnd(factory, device, hwnd, desc, fullscreen_desc, restrict_output, swapchain);
    uint64_t id = 0;
    if (SUCCEEDED(hr) && swapchain && *swapchain) {
        patch_swapchain_vtable(*swapchain);
        AcquireSRWLockExclusive(&g_state_lock);
        id = swapchain_id_for_ptr(*swapchain);
        ReleaseSRWLockExclusive(&g_state_lock);
    }
    std::string out = event_prefix("IDXGIFactory2::CreateSwapChainForHwnd");
    out += ",\"factory\":\"" + trace::ptr_hex(factory) + "\"";
    out += ",\"device\":\"" + trace::ptr_hex(device) + "\"";
    out += ",\"hwnd\":\"" + trace::ptr_hex(hwnd) + "\"";
    out += ",\"swapchain\":\"" + trace::ptr_hex(swapchain && *swapchain ? *swapchain : nullptr) + "\"";
    out += ",\"swapchain_id\":" + std::to_string(id);
    out += ",\"desc\":";
    out += desc ? swapchain_desc1_json(*desc) : "null";
    out += ",\"fullscreen_desc\":";
    out += fullscreen_desc ? "true" : "false";
    out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
    log_event(out);
    return hr;
}

HRESULT STDMETHODCALLTYPE traced_present(IDXGISwapChain *swapchain, UINT sync_interval, UINT flags) {
    HRESULT hr = g_real_present(swapchain, sync_interval, flags);
    uint64_t id = 0;
    AcquireSRWLockExclusive(&g_state_lock);
    id = swapchain_id_for_ptr(swapchain);
    ReleaseSRWLockExclusive(&g_state_lock);
    uint64_t frame = static_cast<uint64_t>(InterlockedIncrement64(&g_present_seq));
    std::string out = event_prefix("IDXGISwapChain::Present");
    out += ",\"swapchain\":\"" + trace::ptr_hex(swapchain) + "\"";
    out += ",\"swapchain_id\":" + std::to_string(id);
    out += ",\"frame\":" + std::to_string(frame);
    out += ",\"sync_interval\":" + std::to_string(sync_interval);
    out += ",\"flags\":" + std::to_string(flags);
    out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
    log_event(out);
    return hr;
}

} // namespace

extern "C" __declspec(dllexport) HRESULT WINAPI CreateDXGIFactory1(REFIID riid, void **factory) {
    auto fn = real_create_dxgi_factory1();
    if (!fn) {
        return E_FAIL;
    }
    HRESULT hr = fn(riid, factory);
    if (SUCCEEDED(hr) && factory && *factory) {
        patch_factory_vtable(*factory);
    }
    std::string out = event_prefix("CreateDXGIFactory1");
    out += ",\"factory\":\"" + trace::ptr_hex(factory && *factory ? *factory : nullptr) + "\"";
    out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
    log_event(out);
    return hr;
}

extern "C" __declspec(dllexport) HRESULT WINAPI CreateDXGIFactory2(UINT flags, REFIID riid, void **factory) {
    auto fn = real_create_dxgi_factory2();
    if (!fn) {
        return E_FAIL;
    }
    HRESULT hr = fn(flags, riid, factory);
    if (SUCCEEDED(hr) && factory && *factory) {
        patch_factory_vtable(*factory);
    }
    std::string out = event_prefix("CreateDXGIFactory2");
    out += ",\"flags\":" + std::to_string(flags);
    out += ",\"factory\":\"" + trace::ptr_hex(factory && *factory ? *factory : nullptr) + "\"";
    out += ",\"hr\":\"" + trace::hr_hex(hr) + "\"}";
    log_event(out);
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
