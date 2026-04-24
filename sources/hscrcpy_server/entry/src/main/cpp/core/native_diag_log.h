#ifndef HSCRCPY_SERVER_CORE_NATIVE_DIAG_LOG_H
#define HSCRCPY_SERVER_CORE_NATIVE_DIAG_LOG_H

#include <chrono>
#include <cstdint>
#include <cstdarg>
#include <cstdio>
#include <hilog/log.h>
#include <mutex>

namespace hscrcpy {
namespace core {
namespace diag {

enum class DiagLogLevel {
    kInfo,
    kWarn,
    kError
};

inline int64_t NowSteadyTimeUs()
{
    const auto now = std::chrono::steady_clock::now();
    return std::chrono::duration_cast<std::chrono::microseconds>(now.time_since_epoch()).count();
}

inline void LogStructuredV(
    DiagLogLevel level, const char *subsystem, const char *operation, const char *format, va_list args)
{
    static std::mutex g_log_mutex;
    static constexpr uint32_t kLogDomain = 0x3200;
    static constexpr const char *kLogTag = "hscrcpyDiag";
    std::lock_guard<std::mutex> lock(g_log_mutex);

    char message[1024];
    std::vsnprintf(message, sizeof(message), format, args);

    const ::LogLevel hilog_level = level == DiagLogLevel::kWarn ? LOG_WARN :
        (level == DiagLogLevel::kError ? LOG_ERROR : LOG_INFO);

    OH_LOG_Print(
        LOG_APP,
        hilog_level,
        kLogDomain,
        kLogTag,
        "subsystem=%{public}s operation=%{public}s %{public}s",
        subsystem == nullptr ? "unknown" : subsystem,
        operation == nullptr ? "unknown" : operation,
        message);
}

inline void LogStructured(DiagLogLevel level, const char *subsystem, const char *operation, const char *format, ...)
{
    va_list args;
    va_start(args, format);
    LogStructuredV(level, subsystem, operation, format, args);
    va_end(args);
}

inline void Info(const char *subsystem, const char *operation, const char *format, ...)
{
    va_list args;
    va_start(args, format);
    LogStructuredV(DiagLogLevel::kInfo, subsystem, operation, format, args);
    va_end(args);
}

inline void Warn(const char *subsystem, const char *operation, const char *format, ...)
{
    va_list args;
    va_start(args, format);
    LogStructuredV(DiagLogLevel::kWarn, subsystem, operation, format, args);
    va_end(args);
}

inline void Error(const char *subsystem, const char *operation, const char *format, ...)
{
    va_list args;
    va_start(args, format);
    LogStructuredV(DiagLogLevel::kError, subsystem, operation, format, args);
    va_end(args);
}

}  // namespace diag
}  // namespace core
}  // namespace hscrcpy

#endif
