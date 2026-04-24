#include "core/native_diag_log.h"
#include "capture/screen_capture_authorization_probe.h"

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstring>
#include <string_view>
#include <string>
#include <thread>
#include <vector>

#include <sys/types.h>
#include <unistd.h>

#if defined(__linux__)
#include <sys/prctl.h>
#endif

namespace {

using hscrcpy::core::diag::Error;
using hscrcpy::core::diag::Info;
using hscrcpy::core::diag::Warn;

constexpr const char *kSubsystem = "poc/uitest_extension";

struct PocConfig {
    int hold_seconds = 30;
    int heartbeat_ms = 1000;
    std::string process_name = "hscrcpy_poc";
    bool probe_screen_capture_auth = false;
    bool probe_screen_capture_each_heartbeat = false;
    std::vector<std::string> argv;
};

PocConfig g_config;
std::atomic<bool> g_initialized{false};

bool IsReasonableArgCount(int argc)
{
    return argc >= 0 && argc <= 128;
}

int ParsePositiveInt(const std::string &value, int fallback)
{
    if (value.empty()) {
        return fallback;
    }

    try {
        const int parsed = std::stoi(value);
        return parsed > 0 ? parsed : fallback;
    } catch (...) {
        return fallback;
    }
}

bool ParseBool(const std::string &value, bool fallback)
{
    if (value.empty()) {
        return fallback;
    }

    if (value == "1" || value == "true" || value == "TRUE" || value == "yes") {
        return true;
    }

    if (value == "0" || value == "false" || value == "FALSE" || value == "no") {
        return false;
    }

    return fallback;
}

bool IsFlag(const std::string &arg, std::string_view name)
{
    return arg == name;
}

void ApplyOption(const std::string &key, const std::string &value)
{
    if (key == "--hold-seconds") {
        g_config.hold_seconds = ParsePositiveInt(value, g_config.hold_seconds);
        return;
    }

    if (key == "--heartbeat-ms") {
        g_config.heartbeat_ms = ParsePositiveInt(value, g_config.heartbeat_ms);
        return;
    }

    if (key == "--process-name" && !value.empty()) {
        g_config.process_name = value.substr(0, 15);
        return;
    }

    if (key == "--probe-screen-capture-auth") {
        g_config.probe_screen_capture_auth = ParseBool(value, true);
        return;
    }

    if (key == "--probe-screen-capture-each-heartbeat") {
        g_config.probe_screen_capture_each_heartbeat = ParseBool(value, true);
        return;
    }
}

void ParseArgs(int argc, const char *argv[])
{
    if (!IsReasonableArgCount(argc)) {
        Warn(kSubsystem, "parse_args", "skip unreasonable argc=%{public}d", argc);
        return;
    }

    for (int index = 0; index < argc; ++index) {
        const char *raw = argv == nullptr ? nullptr : argv[index];
        if (raw == nullptr) {
            g_config.argv.emplace_back("<null>");
            continue;
        }

        const std::string arg(raw);
        g_config.argv.push_back(arg);
        if (IsFlag(arg, "--probe-screen-capture-auth") || IsFlag(arg, "--probe-screen-capture-each-heartbeat")) {
            const bool has_value = index + 1 < argc && argv[index + 1] != nullptr && argv[index + 1][0] != '-';
            ApplyOption(arg, has_value ? argv[index + 1] : "1");
            continue;
        }

        if (index + 1 < argc &&
            (arg == "--hold-seconds" || arg == "--heartbeat-ms" || arg == "--process-name")) {
            const char *value = argv[index + 1];
            ApplyOption(arg, value == nullptr ? "" : value);
        }
    }
}

void SetProcessName()
{
#if defined(__linux__)
    prctl(PR_SET_NAME, g_config.process_name.c_str(), 0, 0, 0);
#endif
}

void LogRuntimeIdentity()
{
    Info(
        kSubsystem,
        "runtime_identity",
        "pid=%{public}d ppid=%{public}d uid=%{public}d euid=%{public}d gid=%{public}d egid=%{public}d",
        static_cast<int>(getpid()),
        static_cast<int>(getppid()),
        static_cast<int>(getuid()),
        static_cast<int>(geteuid()),
        static_cast<int>(getgid()),
        static_cast<int>(getegid()));
}

void RunScreenCaptureProbe(const char *phase, int heartbeat_count)
{
    const auto started_at = std::chrono::steady_clock::now();
    const std::string authorization_state = hscrcpy::capture::auth::ProbeVideoCaptureAuthorizationState();
    const auto elapsed_us = std::chrono::duration_cast<std::chrono::microseconds>(
        std::chrono::steady_clock::now() - started_at)
                                .count();

    Info(
        kSubsystem,
        "screen_capture_probe",
        "phase=%{public}s heartbeat_count=%{public}d authorization_state=%{public}s elapsed_us=%{public}lld",
        phase == nullptr ? "unknown" : phase,
        heartbeat_count,
        authorization_state.c_str(),
        static_cast<long long>(elapsed_us));
}

}  // namespace

extern "C" int UiTestExtension_OnInit(void *token, int argc, const char *argv[])
{
    g_config = PocConfig{};
    ParseArgs(argc, argv);

    Info(
        kSubsystem,
        "on_init",
        "token=%{public}p argc=%{public}d hold_seconds=%{public}d heartbeat_ms=%{public}d process_name=%{public}s probe_screen_capture_auth=%{public}d probe_each_heartbeat=%{public}d",
        token,
        argc,
        g_config.hold_seconds,
        g_config.heartbeat_ms,
        g_config.process_name.c_str(),
        g_config.probe_screen_capture_auth ? 1 : 0,
        g_config.probe_screen_capture_each_heartbeat ? 1 : 0);

    for (size_t index = 0; index < g_config.argv.size(); ++index) {
        Info(
            kSubsystem,
            "argv",
            "index=%{public}zu value=%{public}s",
            index,
            g_config.argv[index].c_str());
    }

    g_initialized.store(true);
    return 0;
}

extern "C" int UiTestExtension_OnRun()
{
    if (!g_initialized.load()) {
        Error(kSubsystem, "on_run", "called before init");
        return -1;
    }

    SetProcessName();
    LogRuntimeIdentity();
    Info(
        kSubsystem,
        "on_run_start",
        "hold_seconds=%{public}d heartbeat_ms=%{public}d process_name=%{public}s probe_screen_capture_auth=%{public}d probe_each_heartbeat=%{public}d",
        g_config.hold_seconds,
        g_config.heartbeat_ms,
        g_config.process_name.c_str(),
        g_config.probe_screen_capture_auth ? 1 : 0,
        g_config.probe_screen_capture_each_heartbeat ? 1 : 0);

    if (g_config.probe_screen_capture_auth) {
        RunScreenCaptureProbe("startup", 0);
    }

    const auto started_at = std::chrono::steady_clock::now();
    const auto deadline = started_at + std::chrono::seconds(g_config.hold_seconds);
    const auto sleep_interval = std::chrono::milliseconds(std::max(g_config.heartbeat_ms, 1));
    int heartbeat_count = 0;

    while (std::chrono::steady_clock::now() < deadline) {
        std::this_thread::sleep_for(sleep_interval);
        ++heartbeat_count;

        const auto now = std::chrono::steady_clock::now();
        const auto elapsed_ms =
            std::chrono::duration_cast<std::chrono::milliseconds>(now - started_at).count();
        const auto remaining_ms =
            std::chrono::duration_cast<std::chrono::milliseconds>(deadline - now).count();

        Info(
            kSubsystem,
            "heartbeat",
            "count=%{public}d elapsed_ms=%{public}lld remaining_ms=%{public}lld",
            heartbeat_count,
            static_cast<long long>(elapsed_ms),
            static_cast<long long>(std::max<int64_t>(remaining_ms, 0)));

        if (g_config.probe_screen_capture_auth && g_config.probe_screen_capture_each_heartbeat) {
            RunScreenCaptureProbe("heartbeat", heartbeat_count);
        }
    }

    Info(
        kSubsystem,
        "on_run_finish",
        "heartbeat_count=%{public}d hold_seconds=%{public}d",
        heartbeat_count,
        g_config.hold_seconds);
    return 0;
}
