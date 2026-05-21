cmake_minimum_required(VERSION 3.20)
project(whistleblower LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Two build paths:
#
# 1. Framework build — when $LOGOS_MODULE_BUILDER_ROOT is set (i.e.
#    inside the logos-module-builder Nix dev shell), include
#    LogosModule.cmake and let it wire up Qt + the SDK + LGX packaging.
#    This is the production path that produces an installable .lgx.
#
# 2. Manual build — fallback for IDE workflows and the standalone
#    preview app. Plain Qt6 CMake. Useful for QML iteration without
#    spinning up the full Logos stack.

if(DEFINED ENV{LOGOS_MODULE_BUILDER_ROOT})
    include($ENV{LOGOS_MODULE_BUILDER_ROOT}/cmake/LogosModule.cmake)

    # Keep module.json in sync with metadata.json on every configure —
    # the lambda-prize validation bot looks for the former, but the
    # Basecamp package_manager reads the latter.
    configure_file(${CMAKE_CURRENT_SOURCE_DIR}/metadata.json
                   ${CMAKE_CURRENT_BINARY_DIR}/module.json COPYONLY)

    logos_module(
        NAME    whistleblower
        SOURCES src/plugin.h
                src/plugin.cpp
                src/backend.h
                src/backend.cpp
        QML     qml/Main.qml
        EXTERNAL_LIBS lp0017_ffi
    )

else()
    set(CMAKE_AUTOMOC ON)
    set(CMAKE_AUTORCC ON)
    find_package(Qt6 REQUIRED COMPONENTS
        Core Gui Widgets Quick QuickWidgets Qml Concurrent
    )

    qt_add_plugin(whistleblower_plugin CLASS_NAME WhistleblowerPlugin)

    target_sources(whistleblower_plugin PRIVATE
        src/plugin.h
        src/plugin.cpp
        src/backend.h
        src/backend.cpp
    )

    qt_add_resources(whistleblower_plugin "qml"
        PREFIX "/qml"
        BASE "qml"
        FILES qml/Main.qml
    )

    target_link_libraries(whistleblower_plugin PRIVATE
        Qt6::Core Qt6::Gui Qt6::Widgets Qt6::Quick
        Qt6::QuickWidgets Qt6::Qml Qt6::Concurrent
    )

    # Basecamp's plugin discovery expects <plugin>/<plugin>.<ext>,
    # not the default lib-prefixed name.
    set_target_properties(whistleblower_plugin PROPERTIES
        OUTPUT_NAME "whistleblower"
        PREFIX ""
    )

    # Copy the manifest next to the binary so the user can drop the
    # whole directory into Basecamp's user-plugins dir.
    add_custom_command(TARGET whistleblower_plugin POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy
            ${CMAKE_CURRENT_SOURCE_DIR}/metadata.json
            $<TARGET_FILE_DIR:whistleblower_plugin>/metadata.json)
    add_custom_command(TARGET whistleblower_plugin POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy
            ${CMAKE_CURRENT_SOURCE_DIR}/metadata.json
            $<TARGET_FILE_DIR:whistleblower_plugin>/module.json)
endif()
