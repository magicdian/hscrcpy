#include "transport/video_channel.h"

#include <algorithm>
#include <arpa/inet.h>
#include <cerrno>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <sstream>
#include <string>
#include <sys/socket.h>
#include <thread>
#include <unistd.h>
#include <vector>

#include "capture/h264_screen_capture_source.h"
#include "core/native_diag_log.h"
#include "video/h264_video_path.h"
#include "video/jpeg_video_path.h"

namespace hscrcpy {
namespace transport {

namespace {

static constexpr uint8_t kVideoCodecTagH264 = 1;
static constexpr uint8_t kVideoCodecTagJpeg = 2;
static constexpr uint8_t kVideoCodecTagH265 = 3;
static constexpr size_t kVideoPacketHeaderLength = 14;
static constexpr int kVideoStartupWaitSeconds = 2;
static constexpr int kJpegFrameIntervalMs = 200;
static constexpr int kH264AccessUnitPollMs = 200;
static constexpr uint64_t kSendSummaryEveryPackets = 120;
static constexpr int64_t kSlowSendWarnUs = 20000;
static constexpr int64_t kBackpressureLagWarnUs = 80000;
static constexpr uint64_t kSendWarnThrottlePackets = 120;

struct SendWriteMetrics {
    int64_t write_duration_us = 0;
    uint64_t send_calls = 0;
    uint64_t partial_writes = 0;
};

struct SendDiagnostics {
    uint64_t packets = 0;
    uint64_t keyframes = 0;
    uint64_t payload_bytes = 0;
    uint64_t packet_bytes = 0;
    uint64_t slow_send_count = 0;
    uint64_t partial_send_count = 0;
    uint64_t backpressure_count = 0;
    uint64_t total_send_calls = 0;
    int64_t total_send_duration_us = 0;
    int64_t max_send_duration_us = 0;
    int64_t last_pts_us = -1;
    int64_t last_send_done_wall_us = -1;
    int64_t total_pts_step_us = 0;
    int64_t total_send_gap_us = 0;
    uint64_t pts_step_samples = 0;
    uint64_t send_gap_samples = 0;
    int64_t max_timeline_lag_us = 0;
    uint64_t last_warning_packet = 0;
};

struct VideoRuntimeState {
    std::mutex mutex;
    std::condition_variable startup_cv;
    std::thread thread;
    bool running = false;
    bool listener_bound = false;
    bool startup_complete = false;
    int listen_fd = -1;
    int client_fd = -1;
    std::string state = "stopped";
    std::string last_error;
    core::VideoTransportState transport_state = {};
};

VideoRuntimeState g_runtime;

bool ShouldEmitSendSummary(uint64_t packets)
{
    return packets != 0 && (packets % kSendSummaryEveryPackets) == 0;
}

bool ShouldEmitSendWarning(uint64_t packets, uint64_t *last_warning_packet)
{
    if (last_warning_packet == nullptr) {
        return false;
    }
    if (packets <= 3 || packets >= *last_warning_packet + kSendWarnThrottlePackets) {
        *last_warning_packet = packets;
        return true;
    }
    return false;
}

void LogSendSummary(
    const core::VideoTransportState &state, const SendDiagnostics &diag, const char *reason, size_t latest_payload_bytes)
{
    const uint64_t avg_payload_bytes = diag.packets == 0 ? 0 : (diag.payload_bytes / diag.packets);
    const uint64_t avg_send_duration_us =
        diag.packets == 0 ? 0 : static_cast<uint64_t>(diag.total_send_duration_us / static_cast<int64_t>(diag.packets));
    const uint64_t avg_send_calls = diag.packets == 0 ? 0 : (diag.total_send_calls / diag.packets);
    const uint64_t avg_pts_step_us = diag.pts_step_samples == 0 ?
        0 :
        static_cast<uint64_t>(diag.total_pts_step_us / static_cast<int64_t>(diag.pts_step_samples));
    const uint64_t avg_send_gap_us = diag.send_gap_samples == 0 ?
        0 :
        static_cast<uint64_t>(diag.total_send_gap_us / static_cast<int64_t>(diag.send_gap_samples));
    const int64_t avg_timeline_lag_us = (diag.pts_step_samples == 0 || diag.send_gap_samples == 0) ?
        0 :
        static_cast<int64_t>(avg_send_gap_us) - static_cast<int64_t>(avg_pts_step_us);
    core::diag::Info(
        "transport/video_channel",
        "send_summary",
        "session_id=%s codec=%s reason=%s packets=%llu keyframes=%llu payload_bytes_total=%llu "
        "payload_bytes_avg=%llu payload_bytes_latest=%zu packet_bytes_total=%llu send_duration_avg_us=%llu "
        "send_duration_max_us=%lld send_calls_avg=%llu slow_sends=%llu partial_sends=%llu backpressure=%llu "
        "pts_step_avg_us=%llu send_gap_avg_us=%llu timeline_lag_avg_us=%lld timeline_lag_max_us=%lld",
        state.session_id.c_str(),
        state.selected_video_codec.c_str(),
        reason,
        static_cast<unsigned long long>(diag.packets),
        static_cast<unsigned long long>(diag.keyframes),
        static_cast<unsigned long long>(diag.payload_bytes),
        static_cast<unsigned long long>(avg_payload_bytes),
        latest_payload_bytes,
        static_cast<unsigned long long>(diag.packet_bytes),
        static_cast<unsigned long long>(avg_send_duration_us),
        static_cast<long long>(diag.max_send_duration_us),
        static_cast<unsigned long long>(avg_send_calls),
        static_cast<unsigned long long>(diag.slow_send_count),
        static_cast<unsigned long long>(diag.partial_send_count),
        static_cast<unsigned long long>(diag.backpressure_count),
        static_cast<unsigned long long>(avg_pts_step_us),
        static_cast<unsigned long long>(avg_send_gap_us),
        static_cast<long long>(avg_timeline_lag_us),
        static_cast<long long>(diag.max_timeline_lag_us));
}

void LogSendAnomaly(
    const core::VideoTransportState &state,
    const SendDiagnostics &diag,
    const char *kind,
    int64_t pts_us,
    bool is_keyframe,
    size_t payload_bytes,
    size_t packet_bytes,
    const SendWriteMetrics &metrics,
    int64_t pts_step_us,
    int64_t send_gap_us,
    int64_t timeline_lag_us)
{
    core::diag::Warn(
        "transport/video_channel",
        "send_anomaly",
        "session_id=%s codec=%s kind=%s packet_idx=%llu pts_us=%lld pts_step_us=%lld send_gap_us=%lld "
        "timeline_lag_us=%lld keyframe=%d payload_bytes=%zu packet_bytes=%zu send_duration_us=%lld "
        "send_calls=%llu partial_writes=%llu slow_sends=%llu partial_sends=%llu backpressure=%llu",
        state.session_id.c_str(),
        state.selected_video_codec.c_str(),
        kind,
        static_cast<unsigned long long>(diag.packets),
        static_cast<long long>(pts_us),
        static_cast<long long>(pts_step_us),
        static_cast<long long>(send_gap_us),
        static_cast<long long>(timeline_lag_us),
        is_keyframe ? 1 : 0,
        payload_bytes,
        packet_bytes,
        static_cast<long long>(metrics.write_duration_us),
        static_cast<unsigned long long>(metrics.send_calls),
        static_cast<unsigned long long>(metrics.partial_writes),
        static_cast<unsigned long long>(diag.slow_send_count),
        static_cast<unsigned long long>(diag.partial_send_count),
        static_cast<unsigned long long>(diag.backpressure_count));
}

void RecordSendDiagnostics(
    const core::VideoTransportState &state,
    int64_t pts_us,
    bool is_keyframe,
    size_t payload_bytes,
    size_t packet_bytes,
    const SendWriteMetrics &metrics,
    SendDiagnostics *diag)
{
    if (diag == nullptr) {
        return;
    }

    ++diag->packets;
    if (is_keyframe) {
        ++diag->keyframes;
    }
    diag->payload_bytes += static_cast<uint64_t>(payload_bytes);
    diag->packet_bytes += static_cast<uint64_t>(packet_bytes);
    diag->total_send_calls += metrics.send_calls;
    diag->total_send_duration_us += metrics.write_duration_us;
    diag->max_send_duration_us = std::max(diag->max_send_duration_us, metrics.write_duration_us);

    if (metrics.write_duration_us >= kSlowSendWarnUs) {
        ++diag->slow_send_count;
    }
    if (metrics.send_calls > 1 || metrics.partial_writes > 0) {
        ++diag->partial_send_count;
    }

    int64_t pts_step_us = -1;
    if (diag->last_pts_us >= 0) {
        pts_step_us = pts_us - diag->last_pts_us;
        if (pts_step_us >= 0) {
            diag->total_pts_step_us += pts_step_us;
            ++diag->pts_step_samples;
        }
    }
    diag->last_pts_us = pts_us;

    const int64_t send_done_wall_us = core::diag::NowSteadyTimeUs();
    int64_t send_gap_us = -1;
    if (diag->last_send_done_wall_us >= 0) {
        send_gap_us = send_done_wall_us - diag->last_send_done_wall_us;
        if (send_gap_us >= 0) {
            diag->total_send_gap_us += send_gap_us;
            ++diag->send_gap_samples;
        }
    }
    diag->last_send_done_wall_us = send_done_wall_us;

    int64_t timeline_lag_us = 0;
    bool backpressure_anomaly = false;
    if (pts_step_us >= 0 && send_gap_us >= 0) {
        timeline_lag_us = send_gap_us - pts_step_us;
        if (timeline_lag_us > diag->max_timeline_lag_us) {
            diag->max_timeline_lag_us = timeline_lag_us;
        }
        if (timeline_lag_us >= kBackpressureLagWarnUs) {
            ++diag->backpressure_count;
            backpressure_anomaly = true;
        }
    }

    if (ShouldEmitSendSummary(diag->packets)) {
        LogSendSummary(state, *diag, "interval", payload_bytes);
    }

    const bool slow_anomaly = metrics.write_duration_us >= kSlowSendWarnUs;
    const bool partial_anomaly = (metrics.send_calls > 1 || metrics.partial_writes > 0);
    if ((slow_anomaly || partial_anomaly || backpressure_anomaly) &&
        ShouldEmitSendWarning(diag->packets, &diag->last_warning_packet)) {
        const char *kind = "send_anomaly";
        if (slow_anomaly) {
            kind = "slow_send";
        } else if (backpressure_anomaly) {
            kind = "backpressure_lag";
        } else if (partial_anomaly) {
            kind = "partial_write";
        }
        LogSendAnomaly(state, *diag, kind, pts_us, is_keyframe, payload_bytes, packet_bytes, metrics, pts_step_us, send_gap_us, timeline_lag_us);
    }
}

void CloseSocket(int *fd)
{
    if (fd == nullptr || *fd < 0) {
        return;
    }
    shutdown(*fd, SHUT_RDWR);
    close(*fd);
    *fd = -1;
}

std::thread TakeRuntimeThreadLocked()
{
    std::thread thread;
    thread.swap(g_runtime.thread);
    return thread;
}

bool IsRuntimeRunning()
{
    std::lock_guard<std::mutex> lock(g_runtime.mutex);
    return g_runtime.running;
}

void SetRuntimeError(const std::string &message)
{
    core::diag::Error(
        "transport/video_channel",
        "runtime_error",
        "message=%s",
        message.c_str());
    std::lock_guard<std::mutex> lock(g_runtime.mutex);
    g_runtime.running = false;
    g_runtime.listener_bound = false;
    g_runtime.state = "error";
    g_runtime.last_error = message;
    g_runtime.transport_state.active = false;
    CloseSocket(&g_runtime.client_fd);
    CloseSocket(&g_runtime.listen_fd);
    g_runtime.startup_complete = true;
    g_runtime.startup_cv.notify_all();
}

std::string JoinNotes(const std::vector<std::string> &notes)
{
    if (notes.empty()) {
        return "video transport rejected the runtime request";
    }

    std::ostringstream stream;
    for (size_t index = 0; index < notes.size(); ++index) {
        if (index != 0) {
            stream << " ";
        }
        stream << notes[index];
    }
    return stream.str();
}

bool WriteAll(int fd, const uint8_t *bytes, size_t length, SendWriteMetrics *metrics, std::string *error)
{
    if (metrics != nullptr) {
        *metrics = {};
    }
    const int64_t write_started_us = core::diag::NowSteadyTimeUs();
    size_t offset = 0;
    while (offset < length) {
        const size_t remaining = length - offset;
        int flags = 0;
#ifdef MSG_NOSIGNAL
        flags |= MSG_NOSIGNAL;
#endif
        const ssize_t written = send(fd, bytes + offset, remaining, flags);
        if (written < 0) {
            if (errno == EINTR) {
                continue;
            }
            if (error != nullptr) {
                *error = "failed to write video packet bytes: " + std::string(std::strerror(errno));
            }
            return false;
        }
        if (written == 0) {
            if (error != nullptr) {
                *error = "failed to write video packet bytes: send returned 0 before packet completion";
            }
            return false;
        }
        if (metrics != nullptr) {
            ++metrics->send_calls;
            if (static_cast<size_t>(written) < remaining) {
                ++metrics->partial_writes;
            }
        }
        offset += static_cast<size_t>(written);
    }
    if (metrics != nullptr) {
        metrics->write_duration_us = core::diag::NowSteadyTimeUs() - write_started_us;
    }
    return true;
}

uint8_t EncodeVideoCodecTag(const std::string &codec, bool *ok)
{
    *ok = true;
    if (codec == core::kVideoCodecH264) {
        return kVideoCodecTagH264;
    }
    if (codec == core::kVideoCodecJpeg) {
        return kVideoCodecTagJpeg;
    }
    if (codec == core::kVideoCodecH265) {
        return kVideoCodecTagH265;
    }
    *ok = false;
    return 0;
}

std::vector<uint8_t> BuildPlaceholderPayloadForJpeg()
{
    return video::jpeg::GetPlaceholderJpegFrameBytes();
}

std::chrono::milliseconds ResolveJpegFrameInterval()
{
    return std::chrono::milliseconds(kJpegFrameIntervalMs);
}

void SetLastRuntimeError(const std::string &error)
{
    core::diag::Warn(
        "transport/video_channel",
        "stream_error",
        "message=%s",
        error.c_str());
    std::lock_guard<std::mutex> lock(g_runtime.mutex);
    g_runtime.last_error = error;
}

bool SerializeVideoPacket(
    const core::VideoPacketEnvelope &packet,
    const std::vector<uint8_t> &payload,
    std::vector<uint8_t> *bytes,
    std::string *error)
{
    if (bytes == nullptr) {
        if (error != nullptr) {
            *error = "video packet serialization requires a non-null output buffer";
        }
        return false;
    }

    bool has_codec_tag = false;
    const uint8_t codec_tag = EncodeVideoCodecTag(packet.codec, &has_codec_tag);
    if (!has_codec_tag) {
        if (error != nullptr) {
            *error = "unsupported runtime video codec `" + packet.codec + "`";
        }
        return false;
    }

    bytes->assign(kVideoPacketHeaderLength + payload.size(), 0);
    (*bytes)[0] = codec_tag;

    const uint64_t pts_us = static_cast<uint64_t>(packet.pts_us);
    const uint32_t payload_length = static_cast<uint32_t>(payload.size());
    for (size_t index = 0; index < sizeof(pts_us); ++index) {
        (*bytes)[1 + index] = static_cast<uint8_t>((pts_us >> ((7 - index) * 8)) & 0xFF);
    }
    (*bytes)[9] = packet.is_keyframe ? 1 : 0;
    for (size_t index = 0; index < sizeof(payload_length); ++index) {
        (*bytes)[10 + index] = static_cast<uint8_t>((payload_length >> ((3 - index) * 8)) & 0xFF);
    }
    std::memcpy(bytes->data() + kVideoPacketHeaderLength, payload.data(), payload.size());
    return true;
}

bool PrepareVideoPacketBytes(
    const core::VideoTransportState &state,
    int64_t pts_us,
    bool is_keyframe,
    const std::vector<uint8_t> &payload,
    std::vector<uint8_t> *bytes,
    std::string *error)
{
    const core::VideoPacketTransportResult result = PrepareVideoPacket(
        state,
        {
            state.session_id,
            state.selected_video_codec,
            pts_us,
            is_keyframe,
            static_cast<int32_t>(payload.size())
        });
    if (!result.accepted) {
        if (error != nullptr) {
            *error = JoinNotes(result.notes);
        }
        return false;
    }

    return SerializeVideoPacket(result.packet, payload, bytes, error);
}

bool SendPreparedVideoPacket(
    int client_fd,
    const core::VideoTransportState &state,
    int64_t pts_us,
    bool is_keyframe,
    const std::vector<uint8_t> &payload,
    SendDiagnostics *diag,
    std::string *error)
{
    std::vector<uint8_t> packet_bytes;
    std::string packet_error;
    if (!PrepareVideoPacketBytes(state, pts_us, is_keyframe, payload, &packet_bytes, &packet_error)) {
        if (error != nullptr) {
            *error = "transport/video_channel.prepare_packet: " + packet_error;
        }
        return false;
    }

    std::string write_error;
    SendWriteMetrics write_metrics = {};
    if (!WriteAll(client_fd, packet_bytes.data(), packet_bytes.size(), &write_metrics, &write_error)) {
        if (error != nullptr) {
            *error = "transport/video_channel.write_packet: " + write_error;
        }
        return false;
    }
    RecordSendDiagnostics(state, pts_us, is_keyframe, payload.size(), packet_bytes.size(), write_metrics, diag);
    return true;
}

void StreamJpegPlaceholderPackets(int client_fd, const core::VideoTransportState &state, SendDiagnostics *diag)
{
    const std::vector<uint8_t> payload = BuildPlaceholderPayloadForJpeg();
    if (payload.empty()) {
        SetLastRuntimeError("transport/video_channel.jpeg_placeholder: payload must not be empty");
        return;
    }

    const std::chrono::milliseconds frame_interval = ResolveJpegFrameInterval();
    const auto started_at = std::chrono::steady_clock::now();
    auto next_deadline = started_at;

    while (IsRuntimeRunning()) {
        const auto now = std::chrono::steady_clock::now();
        if (next_deadline > now) {
            std::this_thread::sleep_until(next_deadline);
        }

        const auto current_time = std::chrono::steady_clock::now();
        const int64_t pts_us = std::chrono::duration_cast<std::chrono::microseconds>(current_time - started_at).count();
        std::string packet_error;
        if (!SendPreparedVideoPacket(client_fd, state, pts_us, true, payload, diag, &packet_error)) {
            SetLastRuntimeError(packet_error);
            break;
        }
        next_deadline = current_time + frame_interval;
    }
}

void StreamH264AccessUnits(int client_fd, const core::VideoTransportState &state, SendDiagnostics *diag)
{
    capture::h264::ScreenCaptureSource source;
    std::string source_error;
    if (!source.Start(state, &source_error)) {
        SetLastRuntimeError("transport/video_channel.h264_start: " + source_error);
        return;
    }

    while (IsRuntimeRunning()) {
        capture::h264::AccessUnit unit = {};
        std::string pull_error;
        const capture::h264::PullResult pull_result =
            source.PullAccessUnit(kH264AccessUnitPollMs, &unit, &pull_error);
        if (pull_result == capture::h264::PullResult::kTimeout) {
            continue;
        }
        if (pull_result == capture::h264::PullResult::kStopped) {
            if (IsRuntimeRunning()) {
                SetLastRuntimeError("transport/video_channel.h264_pull: capture runtime stopped unexpectedly");
            }
            break;
        }
        if (pull_result == capture::h264::PullResult::kError) {
            SetLastRuntimeError("transport/video_channel.h264_pull: " + pull_error);
            break;
        }
        if (unit.bytes.empty()) {
            continue;
        }

        std::string packet_error;
        if (!SendPreparedVideoPacket(client_fd, state, unit.pts_us, unit.is_keyframe, unit.bytes, diag, &packet_error)) {
            SetLastRuntimeError(packet_error);
            break;
        }
    }

    source.Stop();
}

void StreamVideoPackets(int client_fd)
{
    core::VideoTransportState configured_state = {};
    {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        configured_state = g_runtime.transport_state;
    }

    const core::VideoTransportActivationResult activation = ActivateVideoChannel(configured_state);
    if (!activation.activated) {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        g_runtime.last_error = JoinNotes(activation.notes);
        return;
    }

    {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        g_runtime.transport_state = activation.state;
        g_runtime.state = "streaming";
        g_runtime.last_error.clear();
    }

    SendDiagnostics send_diag = {};
    if (activation.state.selected_video_codec == core::kVideoCodecH264) {
        StreamH264AccessUnits(client_fd, activation.state, &send_diag);
    } else if (activation.state.selected_video_codec == core::kVideoCodecJpeg) {
        StreamJpegPlaceholderPackets(client_fd, activation.state, &send_diag);
    } else {
        SetLastRuntimeError(
            "transport/video_channel.stream: unsupported negotiated codec `" + activation.state.selected_video_codec + "`");
    }

    if (send_diag.packets > 0) {
        LogSendSummary(activation.state, send_diag, "stream_end", 0);
    }
}

void ListenerThreadMain(core::VideoTransportState state)
{
    const int listen_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (listen_fd < 0) {
        SetRuntimeError("failed to create video listener socket: " + std::string(std::strerror(errno)));
        return;
    }

    int reuse_addr = 1;
    setsockopt(listen_fd, SOL_SOCKET, SO_REUSEADDR, &reuse_addr, sizeof(reuse_addr));

    sockaddr_in address = {};
    address.sin_family = AF_INET;
    address.sin_port = htons(static_cast<uint16_t>(core::kDefaultVideoPort));
    address.sin_addr.s_addr = htonl(INADDR_ANY);

    if (bind(listen_fd, reinterpret_cast<sockaddr *>(&address), sizeof(address)) != 0) {
        close(listen_fd);
        SetRuntimeError(
            "failed to bind video listener on 0.0.0.0:" + std::to_string(core::kDefaultVideoPort) + ": " +
            std::string(std::strerror(errno)));
        return;
    }
    if (listen(listen_fd, 1) != 0) {
        close(listen_fd);
        SetRuntimeError(
            "failed to listen on video port " + std::to_string(core::kDefaultVideoPort) + ": " +
            std::string(std::strerror(errno)));
        return;
    }

    {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        g_runtime.listen_fd = listen_fd;
        g_runtime.listener_bound = true;
        g_runtime.startup_complete = true;
        g_runtime.state = "listening";
        g_runtime.last_error.clear();
        g_runtime.transport_state = state;
        g_runtime.startup_cv.notify_all();
    }

    while (IsRuntimeRunning()) {
        const int client_fd = accept(listen_fd, nullptr, nullptr);
        if (client_fd < 0) {
            if (!IsRuntimeRunning()) {
                break;
            }
            if (errno == EINTR) {
                continue;
            }
            std::lock_guard<std::mutex> lock(g_runtime.mutex);
            g_runtime.last_error = "video listener accept failed: " + std::string(std::strerror(errno));
            continue;
        }

        {
            std::lock_guard<std::mutex> lock(g_runtime.mutex);
            g_runtime.client_fd = client_fd;
            g_runtime.state = "connected";
            g_runtime.last_error.clear();
        }

        StreamVideoPackets(client_fd);

        {
            std::lock_guard<std::mutex> lock(g_runtime.mutex);
            CloseSocket(&g_runtime.client_fd);
            if (g_runtime.running) {
                g_runtime.transport_state.active = false;
                g_runtime.transport_state.binding.state = core::kChannelStatePendingOpen;
                g_runtime.state = "listening";
            }
        }
    }

    {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        CloseSocket(&g_runtime.client_fd);
        CloseSocket(&g_runtime.listen_fd);
        g_runtime.listener_bound = false;
        g_runtime.transport_state.active = false;
        g_runtime.running = false;
        if (g_runtime.state != "error") {
            g_runtime.state = "stopped";
        }
        g_runtime.startup_complete = true;
        g_runtime.startup_cv.notify_all();
    }
}

core::VideoUnitPreview BuildVideoUnitPreview(const std::string &codec)
{
    if (codec == core::kVideoCodecH264) {
        return video::h264::BuildVideoUnitPreview();
    }
    return video::jpeg::BuildVideoUnitPreview();
}

}  // namespace

core::VideoTransportState BuildVideoTransportState(
    const core::SessionTransportState &session_state,
    const core::SessionReadyPreview &session_ready,
    const core::SessionConfigRequest &request,
    const std::vector<std::string> &pipeline_stages)
{
    return {
        session_state.session_id,
        session_ready.selected_video_codec,
        session_ready.channel_layout.video,
        session_ready.display,
        {
            request.video_max_width,
            request.video_max_height,
            request.video_max_fps,
            request.has_video_bitrate_kbps,
            request.video_bitrate_kbps,
            request.has_video_iframe_interval_ms,
            request.video_iframe_interval_ms
        },
        session_ready.selected_video_codec == core::kVideoCodecH264 ? "video/h264_video_path" : "video/jpeg_video_path",
        pipeline_stages,
        false
    };
}

core::VideoTransportActivationResult ActivateVideoChannel(const core::VideoTransportState &state)
{
    if (state.session_id.empty()) {
        return {
            state,
            false,
            {
                "The video transport cannot activate without a bound session_id."
            }
        };
    }

    core::VideoTransportState active_state = state;
    active_state.binding.state = core::kChannelStateReady;
    active_state.active = true;

    return {
        active_state,
        true,
        {
            "The video channel activates only after session_ready and an explicit host-side open.",
            "Once active, the channel remains binary-only for negotiated video packet delivery."
        }
    };
}

bool StartVideoChannelRuntime(const core::VideoTransportState &state, std::string *error)
{
    if (state.session_id.empty()) {
        if (error != nullptr) {
            *error = "video runtime requires a non-empty session_id";
        }
        return false;
    }

    std::thread thread_to_join;
    {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        if (g_runtime.thread.joinable()) {
            g_runtime.running = false;
            CloseSocket(&g_runtime.client_fd);
            CloseSocket(&g_runtime.listen_fd);
            thread_to_join = TakeRuntimeThreadLocked();
        }
    }

    if (thread_to_join.joinable()) {
        thread_to_join.join();
    }

    {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        g_runtime.running = true;
        g_runtime.listener_bound = false;
        g_runtime.startup_complete = false;
        g_runtime.state = "starting";
        g_runtime.last_error.clear();
        g_runtime.transport_state = state;
        g_runtime.transport_state.active = false;
        g_runtime.thread = std::thread(ListenerThreadMain, g_runtime.transport_state);
    }

    std::unique_lock<std::mutex> lock(g_runtime.mutex);
    g_runtime.startup_cv.wait_for(lock, std::chrono::seconds(kVideoStartupWaitSeconds), []() {
        return g_runtime.startup_complete;
    });

    if (!g_runtime.listener_bound) {
        std::thread failed_thread;
        if (g_runtime.thread.joinable()) {
            failed_thread = TakeRuntimeThreadLocked();
        }
        const std::string failure_message =
            g_runtime.last_error.empty() ? "video listener runtime failed to start" : g_runtime.last_error;
        lock.unlock();
        if (failed_thread.joinable()) {
            failed_thread.join();
        }
        if (error != nullptr) {
            *error = failure_message;
        }
        return false;
    }

    return true;
}

void StopVideoChannelRuntime()
{
    std::thread thread_to_join;
    {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        g_runtime.running = false;
        CloseSocket(&g_runtime.client_fd);
        CloseSocket(&g_runtime.listen_fd);
        g_runtime.listener_bound = false;
        g_runtime.transport_state.active = false;
        if (g_runtime.thread.joinable()) {
            thread_to_join = TakeRuntimeThreadLocked();
        }
        if (g_runtime.state != "error") {
            g_runtime.state = "stopped";
        }
    }

    if (thread_to_join.joinable()) {
        thread_to_join.join();
    }
}

core::VideoPacketTransportResult PrepareVideoPacket(
    const core::VideoTransportState &state, const core::VideoPacketRequest &request)
{
    if (!state.active) {
        return {
            state,
            false,
            {},
            {
                "The video transport must be activated before the device prepares encoded packet delivery."
            }
        };
    }

    if (request.session_id != state.session_id) {
        return {
            state,
            false,
            {},
            {
                "Video packet preparation requires the same session_id as the active video transport."
            }
        };
    }

    if (request.codec != state.selected_video_codec) {
        return {
            state,
            false,
            {},
            {
                "Video packet preparation must use the codec selected during session_ready."
            }
        };
    }

    if (request.payload_length <= 0) {
        return {
            state,
            false,
            {},
            {
                "payload_length must be positive for every video packet."
            }
        };
    }

    const core::VideoUnitPreview unit_preview = BuildVideoUnitPreview(state.selected_video_codec);
    return {
        state,
        true,
        {
            core::kChannelNameVideo,
            core::kPayloadTypeBinary,
            request.codec,
            request.pts_us,
            request.is_keyframe,
            request.payload_length,
            unit_preview.unit_fields,
            unit_preview.delivery,
            unit_preview.payload_semantics
        },
        {
            "The native transport runtime preserves one encoded access unit or JPEG frame per video payload.",
            "Exact binary packing remains provisional, but codec, pts_us, is_keyframe, and payload_length stay explicit."
        }
    };
}

}  // namespace transport
}  // namespace hscrcpy
