#include "transport/video_channel.h"

#include <arpa/inet.h>
#include <cerrno>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <cstring>
#include <mutex>
#include <sstream>
#include <string>
#include <sys/socket.h>
#include <thread>
#include <unistd.h>
#include <vector>

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
static constexpr int kH264FrameIntervalMs = 100;

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

bool WriteAll(int fd, const uint8_t *bytes, size_t length, std::string *error)
{
    size_t offset = 0;
    while (offset < length) {
        int flags = 0;
#ifdef MSG_NOSIGNAL
        flags |= MSG_NOSIGNAL;
#endif
        const ssize_t written =
            send(fd, bytes + offset, length - offset, flags);
        if (written < 0) {
            if (errno == EINTR) {
                continue;
            }
            if (error != nullptr) {
                *error = "failed to write video packet bytes: " + std::string(std::strerror(errno));
            }
            return false;
        }
        offset += static_cast<size_t>(written);
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

std::vector<uint8_t> BuildPlaceholderPayload(const std::string &codec)
{
    if (codec == core::kVideoCodecH264) {
        return video::h264::GetPlaceholderH264AccessUnitBytes();
    }
    return video::jpeg::GetPlaceholderJpegFrameBytes();
}

std::chrono::milliseconds ResolveFrameInterval(const std::string &codec)
{
    if (codec == core::kVideoCodecH264) {
        return std::chrono::milliseconds(kH264FrameIntervalMs);
    }
    return std::chrono::milliseconds(kJpegFrameIntervalMs);
}

bool IsKeyframe(const std::string &codec, uint64_t frame_index)
{
    if (codec == core::kVideoCodecJpeg) {
        return true;
    }
    return frame_index == 0 || frame_index % 30 == 0;
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

    const std::vector<uint8_t> payload = BuildPlaceholderPayload(activation.state.selected_video_codec);
    if (payload.empty()) {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        g_runtime.last_error = "placeholder video payload must not be empty";
        return;
    }

    const std::chrono::milliseconds frame_interval = ResolveFrameInterval(activation.state.selected_video_codec);
    const auto started_at = std::chrono::steady_clock::now();
    auto next_deadline = started_at;
    uint64_t frame_index = 0;

    while (IsRuntimeRunning()) {
        const auto now = std::chrono::steady_clock::now();
        if (next_deadline > now) {
            std::this_thread::sleep_until(next_deadline);
        }

        const auto current_time = std::chrono::steady_clock::now();
        const int64_t pts_us = std::chrono::duration_cast<std::chrono::microseconds>(current_time - started_at).count();
        std::vector<uint8_t> packet_bytes;
        std::string packet_error;
        if (!PrepareVideoPacketBytes(
                activation.state,
                pts_us,
                IsKeyframe(activation.state.selected_video_codec, frame_index),
                payload,
                &packet_bytes,
                &packet_error)) {
            std::lock_guard<std::mutex> lock(g_runtime.mutex);
            g_runtime.last_error = packet_error;
            break;
        }

        std::string write_error;
        if (!WriteAll(client_fd, packet_bytes.data(), packet_bytes.size(), &write_error)) {
            std::lock_guard<std::mutex> lock(g_runtime.mutex);
            g_runtime.last_error = write_error;
            break;
        }

        ++frame_index;
        next_deadline = current_time + frame_interval;
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
    const std::vector<std::string> &pipeline_stages)
{
    return {
        session_state.session_id,
        session_ready.selected_video_codec,
        session_ready.channel_layout.video,
        session_ready.display,
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
