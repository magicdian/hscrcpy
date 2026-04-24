# the minimum version of CMake.
cmake_minimum_required(VERSION 3.5.0)
project(hscrcpy_server)

set(NATIVERENDER_ROOT_PATH ${CMAKE_CURRENT_SOURCE_DIR})

if(DEFINED PACKAGE_FIND_FILE)
    include(${PACKAGE_FIND_FILE})
endif()

include_directories(${NATIVERENDER_ROOT_PATH}
                    ${NATIVERENDER_ROOT_PATH}/include)
find_package(Threads REQUIRED)

set(ENTRY_NATIVE_SOURCES
    bridge/napi_init.cpp
    bridge/companion_napi.cpp
    bridge/napi_helpers.cpp
    capture/screen_capture_authorization_probe.cpp
    capture/h264_screen_capture_source.cpp
    codec/h264_encoder_capability.cpp
    control/control_message_handler.cpp
    control/input_injector.cpp
    core/companion_core.cpp
    transport/session_channel.cpp
    transport/session_listener_runtime.cpp
    transport/video_channel.cpp
    video/h264_video_path.cpp
    video/jpeg_video_path.cpp
    video/video_path_utils.cpp
)

add_library(entry SHARED ${ENTRY_NATIVE_SOURCES})
target_link_libraries(
    entry
    PUBLIC
    libace_napi.z.so
    libhilog_ndk.z.so
    libnative_avscreen_capture.so
    libnative_media_codecbase.so
    libnative_media_core.so
    libnative_media_venc.so
    libnative_window.so
    Threads::Threads
)

set(UITEST_EXTENSION_POC_SOURCES
    capture/screen_capture_authorization_probe.cpp
    codec/h264_encoder_capability.cpp
    poc/uitest_extension_poc.cpp
)

add_library(hscrcpy_uitest_poc SHARED ${UITEST_EXTENSION_POC_SOURCES})
set_target_properties(
    hscrcpy_uitest_poc
    PROPERTIES
    PREFIX ""
    OUTPUT_NAME "hscrcpy_uitest_poc"
)
target_link_libraries(
    hscrcpy_uitest_poc
    PUBLIC
    libhilog_ndk.z.so
    libnative_avscreen_capture.so
    libnative_media_codecbase.so
    libnative_media_core.so
    Threads::Threads
)
