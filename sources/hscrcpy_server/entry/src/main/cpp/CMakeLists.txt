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
target_link_libraries(entry PUBLIC libace_napi.z.so Threads::Threads)
