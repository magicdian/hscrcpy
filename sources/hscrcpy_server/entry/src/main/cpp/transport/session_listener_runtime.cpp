#include "transport/session_listener_runtime.h"

#include <arpa/inet.h>
#include <cerrno>
#include <chrono>
#include <condition_variable>
#include <cstring>
#include <mutex>
#include <string>
#include <sys/socket.h>
#include <thread>
#include <unistd.h>

#include "core/companion_core.h"
#include "transport/session_channel.h"
#include "transport/video_channel.h"

namespace hscrcpy {
namespace transport {

namespace {

struct ListenerRuntimeState {
    std::mutex mutex;
    std::condition_variable startup_cv;
    std::thread thread;
    bool running = false;
    bool listener_bound = false;
    bool startup_complete = false;
    int32_t session_port = core::kDefaultSessionPort;
    int listen_fd = -1;
    int client_fd = -1;
    std::string state = "stopped";
    std::string last_error;
    std::string active_session_id;
};

ListenerRuntimeState g_runtime;

core::SessionListenerRuntimeStatus SnapshotLocked()
{
    return {
        g_runtime.running,
        g_runtime.listener_bound,
        g_runtime.session_port,
        g_runtime.state,
        g_runtime.last_error,
        g_runtime.active_session_id
    };
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
    std::lock_guard<std::mutex> lock(g_runtime.mutex);
    g_runtime.running = false;
    g_runtime.listener_bound = false;
    g_runtime.state = "error";
    g_runtime.last_error = message;
    g_runtime.active_session_id.clear();
    CloseSocket(&g_runtime.listen_fd);
    CloseSocket(&g_runtime.client_fd);
    g_runtime.startup_complete = true;
    g_runtime.startup_cv.notify_all();
}

void SetActiveSessionId(const std::string &session_id)
{
    std::lock_guard<std::mutex> lock(g_runtime.mutex);
    g_runtime.active_session_id = session_id;
}

void ClearActiveSessionId()
{
    std::lock_guard<std::mutex> lock(g_runtime.mutex);
    g_runtime.active_session_id.clear();
}

core::SessionErrorMessage BuildRuntimeSessionError(
    const std::string &session_id, const std::string &message, bool retryable)
{
    return {
        core::kSessionMessageTypeSessionError,
        session_id,
        core::kSessionErrorInvalidMessage,
        message,
        retryable
    };
}

bool WriteFrame(int fd, const std::string &payload, std::string *error)
{
    size_t offset = 0;
    const std::string framed_payload = payload + "\n";
    while (offset < framed_payload.size()) {
        int flags = 0;
#ifdef MSG_NOSIGNAL
        flags |= MSG_NOSIGNAL;
#endif
        const ssize_t written = send(
            fd,
            framed_payload.data() + offset,
            framed_payload.size() - offset,
            flags);
        if (written < 0) {
            if (errno == EINTR) {
                continue;
            }
            if (error != nullptr) {
                *error = "failed to write session transport frame: " + std::string(std::strerror(errno));
            }
            return false;
        }
        offset += static_cast<size_t>(written);
    }
    return true;
}

bool ReadFrame(int fd, std::string *payload, std::string *error)
{
    payload->clear();
    char buffer[1024];
    while (IsRuntimeRunning()) {
        const ssize_t received = recv(fd, buffer, sizeof(buffer), 0);
        if (received == 0) {
            return false;
        }
        if (received < 0) {
            if (errno == EINTR) {
                continue;
            }
            if (error != nullptr) {
                *error = "failed to read session transport frame: " + std::string(std::strerror(errno));
            }
            return false;
        }

        for (ssize_t index = 0; index < received; ++index) {
            const char current = buffer[index];
            if (current == '\n') {
                return true;
            }
            if (current != '\r') {
                payload->push_back(current);
            }
        }
    }
    return false;
}

void SendRuntimeErrorFrame(int fd, const std::string &session_id, const std::string &message, bool retryable)
{
    std::string ignored_error;
    WriteFrame(fd, SerializeSessionErrorMessage(BuildRuntimeSessionError(session_id, message, retryable)), &ignored_error);
}

void HandleClientConnection(int client_fd)
{
    core::SessionTransportState session_state = {};
    bool has_session_state = false;
    StopVideoChannelRuntime();

    while (IsRuntimeRunning()) {
        std::string inbound_payload;
        std::string io_error;
        if (!ReadFrame(client_fd, &inbound_payload, &io_error)) {
            if (!io_error.empty()) {
                std::lock_guard<std::mutex> lock(g_runtime.mutex);
                g_runtime.last_error = io_error;
            }
            break;
        }
        if (inbound_payload.empty()) {
            continue;
        }

        std::string message_type;
        std::string parse_error;
        if (!TryParseSessionMessageType(inbound_payload, &message_type, &parse_error)) {
            SendRuntimeErrorFrame(client_fd, has_session_state ? session_state.session_id : "", parse_error, false);
            break;
        }

        if (message_type == core::kSessionMessageTypeHostHello) {
            core::HostHelloRequest request = {};
            if (!TryParseHostHelloMessage(inbound_payload, &request, &parse_error)) {
                SendRuntimeErrorFrame(client_fd, "", parse_error, false);
                break;
            }

            const core::SessionTransportOpenResult open_result = core::OpenSessionTransport(request);
            session_state = open_result.state;
            has_session_state = true;
            SetActiveSessionId(session_state.session_id);

            std::string write_error;
            if (!WriteFrame(client_fd, open_result.outbound_message.payload_json, &write_error)) {
                std::lock_guard<std::mutex> lock(g_runtime.mutex);
                g_runtime.last_error = write_error;
                break;
            }

            if (!open_result.accepted) {
                break;
            }
            continue;
        }

        if (message_type == core::kSessionMessageTypeSessionConfig) {
            core::SessionConfigRequest request = {};
            if (!TryParseSessionConfigMessage(inbound_payload, &request, &parse_error)) {
                SendRuntimeErrorFrame(client_fd, has_session_state ? session_state.session_id : "", parse_error, false);
                break;
            }

            if (!has_session_state) {
                SendRuntimeErrorFrame(
                    client_fd,
                    request.session_id,
                    "session_config is invalid before a successful host_hello/device_hello exchange",
                    false);
                break;
            }

            const core::SessionTransportConfigureResult configure_result =
                core::ConfigureSessionTransport(session_state, request);
            if (!configure_result.accepted) {
                session_state = configure_result.state;
                SetActiveSessionId(session_state.session_id);
            } else {
                std::string video_runtime_error;
                if (configure_result.has_video_transport &&
                    !StartVideoChannelRuntime(configure_result.video_transport, &video_runtime_error)) {
                    SendRuntimeErrorFrame(
                        client_fd,
                        request.session_id,
                        "failed to start the negotiated video runtime: " + video_runtime_error,
                        false);
                    std::lock_guard<std::mutex> lock(g_runtime.mutex);
                    g_runtime.last_error = video_runtime_error;
                    break;
                }

                session_state = configure_result.state;
                SetActiveSessionId(session_state.session_id);
            }

            std::string write_error;
            if (!WriteFrame(client_fd, configure_result.outbound_message.payload_json, &write_error)) {
                std::lock_guard<std::mutex> lock(g_runtime.mutex);
                g_runtime.last_error = write_error;
                break;
            }

            if (!configure_result.accepted) {
                break;
            }
            continue;
        }

        if (message_type == core::kSessionMessageTypeStopSession) {
            core::StopSessionRequest request = {};
            if (!TryParseStopSessionMessage(inbound_payload, &request, &parse_error)) {
                SendRuntimeErrorFrame(client_fd, has_session_state ? session_state.session_id : "", parse_error, false);
                break;
            }

            if (has_session_state) {
                session_state = core::StopSessionTransport(session_state, request).state;
            }
            StopVideoChannelRuntime();
            break;
        }

        if (message_type == core::kControlEventType) {
            core::ControlEventRequest request = {};
            if (!TryParseControlEventMessage(inbound_payload, &request, &parse_error)) {
                SendRuntimeErrorFrame(client_fd, has_session_state ? session_state.session_id : "", parse_error, false);
                break;
            }

            if (!has_session_state) {
                SendRuntimeErrorFrame(
                    client_fd,
                    request.session_id,
                    "control_event is invalid before the session reaches the ready state",
                    false);
                break;
            }

            (void)core::DispatchControlEvent(session_state, request);
            continue;
        }

        SendRuntimeErrorFrame(
            client_fd,
            has_session_state ? session_state.session_id : "",
            "unsupported inbound message type `" + message_type + "` on the session channel",
            false);
        break;
    }

    StopVideoChannelRuntime();
}

void ListenerThreadMain(int32_t session_port)
{
    const int listen_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (listen_fd < 0) {
        SetRuntimeError("failed to create session listener socket: " + std::string(std::strerror(errno)));
        return;
    }

    int reuse_addr = 1;
    setsockopt(listen_fd, SOL_SOCKET, SO_REUSEADDR, &reuse_addr, sizeof(reuse_addr));

    sockaddr_in address = {};
    address.sin_family = AF_INET;
    address.sin_port = htons(static_cast<uint16_t>(session_port));
    address.sin_addr.s_addr = htonl(INADDR_ANY);

    if (bind(listen_fd, reinterpret_cast<sockaddr *>(&address), sizeof(address)) != 0) {
        close(listen_fd);
        SetRuntimeError(
            "failed to bind session listener on 0.0.0.0:" + std::to_string(session_port) + ": " +
            std::string(std::strerror(errno)));
        return;
    }
    if (listen(listen_fd, 1) != 0) {
        close(listen_fd);
        SetRuntimeError(
            "failed to listen on session port " + std::to_string(session_port) + ": " +
            std::string(std::strerror(errno)));
        return;
    }

    {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        g_runtime.listen_fd = listen_fd;
        g_runtime.listener_bound = true;
        g_runtime.state = "listening";
        g_runtime.last_error.clear();
        g_runtime.startup_complete = true;
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
            g_runtime.last_error = "session listener accept failed: " + std::string(std::strerror(errno));
            continue;
        }

        {
            std::lock_guard<std::mutex> lock(g_runtime.mutex);
            g_runtime.client_fd = client_fd;
            g_runtime.state = "connected";
            g_runtime.last_error.clear();
        }

        HandleClientConnection(client_fd);

        {
            std::lock_guard<std::mutex> lock(g_runtime.mutex);
            CloseSocket(&g_runtime.client_fd);
            if (g_runtime.running) {
                g_runtime.state = "listening";
            }
        }
        ClearActiveSessionId();
    }

    {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        CloseSocket(&g_runtime.client_fd);
        CloseSocket(&g_runtime.listen_fd);
        g_runtime.listener_bound = false;
        g_runtime.active_session_id.clear();
        if (g_runtime.state != "error") {
            g_runtime.state = "stopped";
        }
        g_runtime.running = false;
        g_runtime.startup_complete = true;
        g_runtime.startup_cv.notify_all();
    }
}

}  // namespace

core::SessionListenerRuntimeStatus StartSessionListenerRuntime(int32_t session_port)
{
    if (session_port <= 0) {
        session_port = core::kDefaultSessionPort;
    }

    std::thread thread_to_join;
    {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        if (g_runtime.running) {
            return SnapshotLocked();
        }
        if (g_runtime.thread.joinable()) {
            thread_to_join = TakeRuntimeThreadLocked();
        }

        g_runtime.running = true;
        g_runtime.listener_bound = false;
        g_runtime.startup_complete = false;
        g_runtime.session_port = session_port;
        g_runtime.state = "starting";
        g_runtime.last_error.clear();
        g_runtime.active_session_id.clear();
        g_runtime.thread = std::thread(ListenerThreadMain, session_port);
    }

    if (thread_to_join.joinable()) {
        thread_to_join.join();
    }

    std::unique_lock<std::mutex> lock(g_runtime.mutex);
    g_runtime.startup_cv.wait_for(lock, std::chrono::seconds(2), []() {
        return g_runtime.startup_complete;
    });
    return SnapshotLocked();
}

core::SessionListenerRuntimeStatus StopSessionListenerRuntime()
{
    std::thread thread_to_join;
    {
        std::lock_guard<std::mutex> lock(g_runtime.mutex);
        g_runtime.running = false;
        CloseSocket(&g_runtime.client_fd);
        CloseSocket(&g_runtime.listen_fd);
        g_runtime.listener_bound = false;
        g_runtime.active_session_id.clear();
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

    std::lock_guard<std::mutex> lock(g_runtime.mutex);
    return SnapshotLocked();
}

core::SessionListenerRuntimeStatus GetSessionListenerRuntimeStatus()
{
    std::lock_guard<std::mutex> lock(g_runtime.mutex);
    return SnapshotLocked();
}

}  // namespace transport
}  // namespace hscrcpy
