#include "trace_common.h"

#include <d3dcompiler.h>
#include <d3d11shader.h>

#include <algorithm>
#include <cstdint>
#include <fstream>
#include <iomanip>
#include <sstream>
#include <vector>

namespace trace {
namespace {

const GUID kIID_ID3D11ShaderReflection = {
    0x8d536ca1,
    0x0cca,
    0x4956,
    {0xa8, 0x37, 0x78, 0x69, 0x63, 0x75, 0x55, 0x84}};

using D3DReflectFn = HRESULT(WINAPI *)(const void *, SIZE_T, REFIID, void **);

SRWLOCK g_file_lock = SRWLOCK_INIT;
HANDLE g_jsonl_file = INVALID_HANDLE_VALUE;
std::wstring g_jsonl_path;

std::wstring widen_ascii(const char *text) {
    std::wstring out;
    if (!text) {
        return out;
    }
    while (*text) {
        out.push_back(static_cast<unsigned char>(*text));
        ++text;
    }
    return out;
}

std::wstring dirname(const std::wstring &path) {
    size_t pos = path.find_last_of(L"\\/");
    if (pos == std::wstring::npos) {
        return L".";
    }
    return path.substr(0, pos);
}

std::wstring join(const std::wstring &a, const std::wstring &b) {
    if (a.empty()) {
        return b;
    }
    wchar_t last = a.back();
    if (last == L'\\' || last == L'/') {
        return a + b;
    }
    return a + L"\\" + b;
}

bool exists_dir(const std::wstring &path) {
    DWORD attrs = GetFileAttributesW(path.c_str());
    return attrs != INVALID_FILE_ATTRIBUTES && (attrs & FILE_ATTRIBUTE_DIRECTORY);
}

void ensure_dir(const std::wstring &path) {
    if (path.empty() || exists_dir(path)) {
        return;
    }
    size_t slash = path.find_last_of(L"\\/");
    if (slash != std::wstring::npos) {
        ensure_dir(path.substr(0, slash));
    }
    CreateDirectoryW(path.c_str(), nullptr);
}

std::wstring module_dir() {
    wchar_t path[MAX_PATH] = {};
    HMODULE module = nullptr;
    if (GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            reinterpret_cast<LPCWSTR>(&module_dir),
            &module)) {
        GetModuleFileNameW(module, path, MAX_PATH);
        return dirname(path);
    }
    GetModuleFileNameW(nullptr, path, MAX_PATH);
    return dirname(path);
}

std::wstring current_dir() {
    DWORD len = GetCurrentDirectoryW(0, nullptr);
    if (len == 0) {
        return L".";
    }
    std::wstring out(len, L'\0');
    GetCurrentDirectoryW(len, out.data());
    while (!out.empty() && out.back() == L'\0') {
        out.pop_back();
    }
    return out;
}

std::wstring trace_dir() {
    wchar_t env[32768] = {};
    DWORD env_len = GetEnvironmentVariableW(L"HOI4_TRACE_DIR", env, static_cast<DWORD>(std::size(env)));
    if (env_len > 0 && env_len < std::size(env)) {
        std::wstring dir(env, env_len);
        ensure_dir(dir);
        ensure_dir(join(dir, L"logs"));
        return dir;
    }

    std::wstring cwd_trace = join(join(current_dir(), L"tools"), L"vanilla_trace");
    if (exists_dir(cwd_trace)) {
        ensure_dir(join(cwd_trace, L"logs"));
        return cwd_trace;
    }

    std::wstring dir = module_dir();
    std::wstring sibling = join(dirname(dir), L"vanilla_trace");
    if (exists_dir(sibling)) {
        ensure_dir(join(sibling, L"logs"));
        return sibling;
    }

    wchar_t local_app_data[MAX_PATH] = {};
    DWORD local_len = GetEnvironmentVariableW(L"LOCALAPPDATA", local_app_data, MAX_PATH);
    std::wstring fallback = local_len > 0 ? std::wstring(local_app_data, local_len) : current_dir();
    fallback = join(fallback, L"hoi4_vanilla_trace");
    ensure_dir(fallback);
    ensure_dir(join(fallback, L"logs"));
    return fallback;
}

std::string read_file_utf8(const std::wstring &path) {
    HANDLE file = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE) {
        return "";
    }

    LARGE_INTEGER size = {};
    if (!GetFileSizeEx(file, &size) || size.QuadPart <= 0 || size.QuadPart > 64ll * 1024ll * 1024ll) {
        CloseHandle(file);
        return "";
    }

    std::string data(static_cast<size_t>(size.QuadPart), '\0');
    DWORD read = 0;
    ReadFile(file, data.data(), static_cast<DWORD>(data.size()), &read, nullptr);
    data.resize(read);
    CloseHandle(file);
    return data;
}

void write_file_utf8(const std::wstring &path, const std::string &data) {
    ensure_dir(dirname(path));
    HANDLE file = CreateFileW(path.c_str(), GENERIC_WRITE, FILE_SHARE_READ, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE) {
        return;
    }
    DWORD written = 0;
    WriteFile(file, data.data(), static_cast<DWORD>(data.size()), &written, nullptr);
    CloseHandle(file);
}

std::string trim_right(std::string data) {
    while (!data.empty() && (data.back() == ' ' || data.back() == '\r' || data.back() == '\n' || data.back() == '\t')) {
        data.pop_back();
    }
    return data;
}

std::string shader_input_type_name(D3D_SHADER_INPUT_TYPE type) {
    switch (type) {
    case D3D_SIT_CBUFFER:
        return "cbuffer";
    case D3D_SIT_TBUFFER:
        return "tbuffer";
    case D3D_SIT_TEXTURE:
        return "texture";
    case D3D_SIT_SAMPLER:
        return "sampler";
    case D3D_SIT_UAV_RWTYPED:
        return "uav_rwtyped";
    case D3D_SIT_STRUCTURED:
        return "structured";
    case D3D_SIT_UAV_RWSTRUCTURED:
        return "uav_rwstructured";
    case D3D_SIT_BYTEADDRESS:
        return "byteaddress";
    case D3D_SIT_UAV_RWBYTEADDRESS:
        return "uav_rwbyteaddress";
    default:
        return "unknown";
    }
}

std::string srv_dimension_name(D3D_SRV_DIMENSION dim) {
    switch (dim) {
    case D3D_SRV_DIMENSION_BUFFER:
        return "buffer";
    case D3D_SRV_DIMENSION_TEXTURE1D:
        return "texture1d";
    case D3D_SRV_DIMENSION_TEXTURE1DARRAY:
        return "texture1d_array";
    case D3D_SRV_DIMENSION_TEXTURE2D:
        return "texture2d";
    case D3D_SRV_DIMENSION_TEXTURE2DARRAY:
        return "texture2d_array";
    case D3D_SRV_DIMENSION_TEXTURE2DMS:
        return "texture2d_ms";
    case D3D_SRV_DIMENSION_TEXTURE2DMSARRAY:
        return "texture2d_ms_array";
    case D3D_SRV_DIMENSION_TEXTURE3D:
        return "texture3d";
    case D3D_SRV_DIMENSION_TEXTURECUBE:
        return "texturecube";
    case D3D_SRV_DIMENSION_TEXTURECUBEARRAY:
        return "texturecube_array";
    default:
        return "unknown";
    }
}

std::string shader_variable_class_name(D3D_SHADER_VARIABLE_CLASS cls) {
    switch (cls) {
    case D3D_SVC_SCALAR:
        return "scalar";
    case D3D_SVC_VECTOR:
        return "vector";
    case D3D_SVC_MATRIX_ROWS:
        return "matrix_rows";
    case D3D_SVC_MATRIX_COLUMNS:
        return "matrix_columns";
    case D3D_SVC_OBJECT:
        return "object";
    case D3D_SVC_STRUCT:
        return "struct";
    case D3D_SVC_INTERFACE_CLASS:
        return "interface_class";
    case D3D_SVC_INTERFACE_POINTER:
        return "interface_pointer";
    default:
        return "unknown";
    }
}

std::string shader_variable_type_name(D3D_SHADER_VARIABLE_TYPE type) {
    switch (type) {
    case D3D_SVT_VOID:
        return "void";
    case D3D_SVT_BOOL:
        return "bool";
    case D3D_SVT_INT:
        return "int";
    case D3D_SVT_FLOAT:
        return "float";
    case D3D_SVT_STRING:
        return "string";
    case D3D_SVT_TEXTURE:
        return "texture";
    case D3D_SVT_TEXTURE1D:
        return "texture1d";
    case D3D_SVT_TEXTURE2D:
        return "texture2d";
    case D3D_SVT_TEXTURE3D:
        return "texture3d";
    case D3D_SVT_TEXTURECUBE:
        return "texturecube";
    case D3D_SVT_SAMPLER:
        return "sampler";
    case D3D_SVT_PIXELSHADER:
        return "pixelshader";
    case D3D_SVT_VERTEXSHADER:
        return "vertexshader";
    case D3D_SVT_UINT:
        return "uint";
    case D3D_SVT_UINT8:
        return "uint8";
    case D3D_SVT_DOUBLE:
        return "double";
    case D3D_SVT_RWTEXTURE1D:
        return "rwtexture1d";
    case D3D_SVT_RWTEXTURE2D:
        return "rwtexture2d";
    case D3D_SVT_RWTEXTURE3D:
        return "rwtexture3d";
    case D3D_SVT_BYTEADDRESS_BUFFER:
        return "byteaddress_buffer";
    case D3D_SVT_RWBYTEADDRESS_BUFFER:
        return "rwbyteaddress_buffer";
    case D3D_SVT_STRUCTURED_BUFFER:
        return "structured_buffer";
    case D3D_SVT_RWSTRUCTURED_BUFFER:
        return "rwstructured_buffer";
    default:
        return "unknown";
    }
}

std::string reflect_constant_buffers_json(ID3D11ShaderReflection *refl, const D3D11_SHADER_DESC &shader_desc) {
    std::string out = "[";
    for (UINT i = 0; i < shader_desc.ConstantBuffers; ++i) {
        ID3D11ShaderReflectionConstantBuffer *cb = refl->GetConstantBufferByIndex(i);
        if (!cb) {
            continue;
        }

        D3D11_SHADER_BUFFER_DESC cb_desc = {};
        if (FAILED(cb->GetDesc(&cb_desc))) {
            continue;
        }

        if (out.size() > 1) {
            out += ",";
        }
        out += "{";
        out += "\"index\":" + std::to_string(i);
        out += ",\"name\":\"" + json_escape(cb_desc.Name ? cb_desc.Name : "") + "\"";
        out += ",\"size\":" + std::to_string(cb_desc.Size);
        out += ",\"variables\":[";

        bool first_var = true;
        for (UINT v = 0; v < cb_desc.Variables; ++v) {
            ID3D11ShaderReflectionVariable *var = cb->GetVariableByIndex(v);
            if (!var) {
                continue;
            }

            D3D11_SHADER_VARIABLE_DESC var_desc = {};
            if (FAILED(var->GetDesc(&var_desc))) {
                continue;
            }

            D3D11_SHADER_TYPE_DESC type_desc = {};
            ID3D11ShaderReflectionType *type = var->GetType();
            if (type) {
                type->GetDesc(&type_desc);
            }

            if (!first_var) {
                out += ",";
            }
            first_var = false;
            out += "{";
            out += "\"name\":\"" + json_escape(var_desc.Name ? var_desc.Name : "") + "\"";
            out += ",\"start_offset\":" + std::to_string(var_desc.StartOffset);
            out += ",\"size\":" + std::to_string(var_desc.Size);
            out += ",\"flags\":" + std::to_string(var_desc.uFlags);
            out += ",\"default_value\":\"" + ptr_hex(var_desc.DefaultValue) + "\"";
            out += ",\"type_class\":\"" + shader_variable_class_name(type_desc.Class) + "\"";
            out += ",\"type\":\"" + shader_variable_type_name(type_desc.Type) + "\"";
            out += ",\"type_id\":" + std::to_string(static_cast<unsigned>(type_desc.Type));
            out += ",\"rows\":" + std::to_string(type_desc.Rows);
            out += ",\"columns\":" + std::to_string(type_desc.Columns);
            out += ",\"elements\":" + std::to_string(type_desc.Elements);
            out += ",\"members\":" + std::to_string(type_desc.Members);
            out += "}";
        }

        out += "]}";
    }
    out += "]";
    return out;
}

D3DReflectFn real_d3d_reflect() {
    static D3DReflectFn fn =
        reinterpret_cast<D3DReflectFn>(get_system_proc("d3dcompiler_47.dll", "D3DReflect"));
    return fn;
}

std::string reflect_bindings_json(const void *bytecode, SIZE_T bytecode_len, bool *ok) {
    *ok = false;
    auto reflect = real_d3d_reflect();
    if (!reflect || !bytecode || bytecode_len == 0) {
        return "[]";
    }

    ID3D11ShaderReflection *refl = nullptr;
    HRESULT hr = reflect(bytecode, bytecode_len, kIID_ID3D11ShaderReflection, reinterpret_cast<void **>(&refl));
    if (FAILED(hr) || !refl) {
        return "[]";
    }

    D3D11_SHADER_DESC shader_desc = {};
    if (FAILED(refl->GetDesc(&shader_desc))) {
        refl->Release();
        return "[]";
    }

    std::string out = "[";
    for (UINT i = 0; i < shader_desc.BoundResources; ++i) {
        D3D11_SHADER_INPUT_BIND_DESC bind = {};
        if (FAILED(refl->GetResourceBindingDesc(i, &bind))) {
            continue;
        }
        if (out.size() > 1) {
            out += ",";
        }
        out += "{";
        out += "\"name\":\"" + json_escape(bind.Name ? bind.Name : "") + "\",";
        out += "\"type\":\"" + shader_input_type_name(bind.Type) + "\",";
        out += "\"type_id\":" + std::to_string(static_cast<unsigned>(bind.Type)) + ",";
        out += "\"bind_point\":" + std::to_string(bind.BindPoint) + ",";
        out += "\"bind_count\":" + std::to_string(bind.BindCount) + ",";
        out += "\"dimension\":\"" + srv_dimension_name(bind.Dimension) + "\",";
        out += "\"dimension_id\":" + std::to_string(static_cast<unsigned>(bind.Dimension)) + ",";
        out += "\"return_type\":" + std::to_string(static_cast<unsigned>(bind.ReturnType)) + ",";
        out += "\"samples\":" + std::to_string(bind.NumSamples);
        out += "}";
    }
    out += "]";
    refl->Release();
    *ok = true;
    return out;
}

std::string reflect_constant_buffers_for_bytecode_json(const void *bytecode, SIZE_T bytecode_len, bool *ok) {
    *ok = false;
    auto reflect = real_d3d_reflect();
    if (!reflect || !bytecode || bytecode_len == 0) {
        return "[]";
    }

    ID3D11ShaderReflection *refl = nullptr;
    HRESULT hr = reflect(bytecode, bytecode_len, kIID_ID3D11ShaderReflection, reinterpret_cast<void **>(&refl));
    if (FAILED(hr) || !refl) {
        return "[]";
    }

    D3D11_SHADER_DESC shader_desc = {};
    if (FAILED(refl->GetDesc(&shader_desc))) {
        refl->Release();
        return "[]";
    }

    std::string out = reflect_constant_buffers_json(refl, shader_desc);
    refl->Release();
    *ok = true;
    return out;
}

} // namespace

std::wstring trace_file_path(const wchar_t *name) {
    return join(trace_dir(), name ? name : L"trace.json");
}

FARPROC get_system_proc(const char *dll_name, const char *proc_name) {
    std::wstring dll = widen_ascii(dll_name);
    wchar_t system_dir[MAX_PATH] = {};
    UINT len = GetSystemDirectoryW(system_dir, MAX_PATH);
    if (len == 0 || len >= MAX_PATH) {
        return nullptr;
    }

    std::wstring path = join(std::wstring(system_dir, len), dll);
    HMODULE module = LoadLibraryW(path.c_str());
    if (!module) {
        return nullptr;
    }
    return GetProcAddress(module, proc_name);
}

std::string json_escape(const std::string &value) {
    std::string out;
    out.reserve(value.size() + 8);
    for (unsigned char c : value) {
        switch (c) {
        case '\\':
            out += "\\\\";
            break;
        case '"':
            out += "\\\"";
            break;
        case '\b':
            out += "\\b";
            break;
        case '\f':
            out += "\\f";
            break;
        case '\n':
            out += "\\n";
            break;
        case '\r':
            out += "\\r";
            break;
        case '\t':
            out += "\\t";
            break;
        default:
            if (c < 0x20) {
                std::ostringstream ss;
                ss << "\\u" << std::hex << std::setw(4) << std::setfill('0') << static_cast<int>(c);
                out += ss.str();
            } else {
                out.push_back(static_cast<char>(c));
            }
            break;
        }
    }
    return out;
}

std::string json_escape(const char *value) {
    return json_escape(std::string(value ? value : ""));
}

std::string hr_hex(HRESULT hr) {
    std::ostringstream ss;
    ss << "0x" << std::hex << std::setw(8) << std::setfill('0') << static_cast<unsigned long>(static_cast<uint32_t>(hr));
    return ss.str();
}

std::string hash_bytes(const void *data, SIZE_T len) {
    if (!data || len == 0) {
        return "";
    }
    const auto *bytes = static_cast<const uint8_t *>(data);
    uint64_t hash = 14695981039346656037ull;
    for (SIZE_T i = 0; i < len; ++i) {
        hash ^= bytes[i];
        hash *= 1099511628211ull;
    }
    std::ostringstream ss;
    ss << "fnv1a64:" << std::hex << std::setw(16) << std::setfill('0') << hash;
    return ss.str();
}

std::string ptr_hex(const void *ptr) {
    std::ostringstream ss;
    ss << "0x" << std::hex << std::setw(sizeof(void *) * 2) << std::setfill('0')
       << reinterpret_cast<uintptr_t>(ptr);
    return ss.str();
}

unsigned long process_id() {
    return GetCurrentProcessId();
}

unsigned long long tick_ms() {
    return GetTickCount64();
}

void append_json_object(const std::wstring &path, const std::string &object_json) {
    HANDLE mutex = CreateMutexW(nullptr, FALSE, L"Local\\HOI4VanillaTraceJsonMutex");
    if (mutex) {
        WaitForSingleObject(mutex, INFINITE);
    }

    {
        AcquireSRWLockExclusive(&g_file_lock);
        std::string data = trim_right(read_file_utf8(path));
        std::string next;
        if (data.empty()) {
            next = "[\n" + object_json + "\n]\n";
        } else if (data == "[]") {
            next = "[\n" + object_json + "\n]\n";
        } else if (!data.empty() && data.back() == ']') {
            data.pop_back();
            data = trim_right(data);
            if (!data.empty() && data.back() == '[') {
                next = data + "\n" + object_json + "\n]\n";
            } else {
                next = data + ",\n" + object_json + "\n]\n";
            }
        } else {
            next = "[\n" + object_json + "\n]\n";
        }
        write_file_utf8(path, next);
        ReleaseSRWLockExclusive(&g_file_lock);
    }

    if (mutex) {
        ReleaseMutex(mutex);
        CloseHandle(mutex);
    }
}

void append_json_line(const std::wstring &path, const std::string &object_json) {
    HANDLE mutex = CreateMutexW(nullptr, FALSE, L"Local\\HOI4VanillaTraceJsonMutex");
    if (mutex) {
        WaitForSingleObject(mutex, INFINITE);
    }

    {
        AcquireSRWLockExclusive(&g_file_lock);
        ensure_dir(dirname(path));
        if (g_jsonl_file == INVALID_HANDLE_VALUE || g_jsonl_path != path) {
            if (g_jsonl_file != INVALID_HANDLE_VALUE) {
                CloseHandle(g_jsonl_file);
                g_jsonl_file = INVALID_HANDLE_VALUE;
            }
            g_jsonl_path = path;
            g_jsonl_file = CreateFileW(
                path.c_str(),
                FILE_APPEND_DATA,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                nullptr,
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                nullptr);
        }
        if (g_jsonl_file != INVALID_HANDLE_VALUE) {
            DWORD written = 0;
            WriteFile(g_jsonl_file, object_json.data(), static_cast<DWORD>(object_json.size()), &written, nullptr);
            const char newline = '\n';
            WriteFile(g_jsonl_file, &newline, 1, &written, nullptr);
        }
        ReleaseSRWLockExclusive(&g_file_lock);
    }

    if (mutex) {
        ReleaseMutex(mutex);
        CloseHandle(mutex);
    }
}

void flush_trace_files() {
    AcquireSRWLockExclusive(&g_file_lock);
    if (g_jsonl_file != INVALID_HANDLE_VALUE) {
        FlushFileBuffers(g_jsonl_file);
    }
    ReleaseSRWLockExclusive(&g_file_lock);
}

void record_shader_bytecode(
    const char *event,
    const char *stage,
    const void *bytecode,
    SIZE_T bytecode_len,
    const char *filename,
    const char *entrypoint,
    const char *target) {
    std::string hash = hash_bytes(bytecode, bytecode_len);
    bool reflected = false;
    std::string bindings = reflect_bindings_json(bytecode, bytecode_len, &reflected);
    bool cb_reflected = false;
    std::string constant_buffers = reflect_constant_buffers_for_bytecode_json(bytecode, bytecode_len, &cb_reflected);

    std::string out = "{";
    out += "\"event\":\"" + json_escape(event ? event : "") + "\",";
    out += "\"stage\":\"" + json_escape(stage ? stage : "") + "\",";
    out += "\"filename\":\"" + json_escape(filename ? filename : "") + "\",";
    out += "\"entrypoint\":\"" + json_escape(entrypoint ? entrypoint : "") + "\",";
    out += "\"target\":\"" + json_escape(target ? target : "") + "\",";
    out += "\"bytecode_hash\":\"" + hash + "\",";
    out += "\"bytecode_size\":" + std::to_string(static_cast<unsigned long long>(bytecode_len)) + ",";
    out += "\"reflection_ok\":";
    out += reflected ? "true" : "false";
    out += ",";
    out += "\"resources\":";
    out += bindings;
    out += ",";
    out += "\"constant_buffer_reflection_ok\":";
    out += cb_reflected ? "true" : "false";
    out += ",";
    out += "\"constant_buffers\":";
    out += constant_buffers;
    out += "}";

    append_json_object(trace_file_path(L"shader_bindings.json"), out);
}

} // namespace trace
