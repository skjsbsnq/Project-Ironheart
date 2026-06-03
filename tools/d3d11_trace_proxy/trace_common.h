#pragma once

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif

#include <windows.h>

#include <string>

namespace trace {

std::wstring trace_file_path(const wchar_t *name);
FARPROC get_system_proc(const char *dll_name, const char *proc_name);

std::string json_escape(const std::string &value);
std::string json_escape(const char *value);
std::string hr_hex(HRESULT hr);
std::string hash_bytes(const void *data, SIZE_T len);
std::string ptr_hex(const void *ptr);
unsigned long process_id();
unsigned long long tick_ms();

void append_json_object(const std::wstring &path, const std::string &object_json);
void append_json_line(const std::wstring &path, const std::string &object_json);
void flush_trace_files();
void record_shader_bytecode(
    const char *event,
    const char *stage,
    const void *bytecode,
    SIZE_T bytecode_len,
    const char *filename,
    const char *entrypoint,
    const char *target);

} // namespace trace
