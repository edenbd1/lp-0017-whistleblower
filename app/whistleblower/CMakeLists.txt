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

    # The framework's logos_module() only pulls Qt6::Core +
    # RemoteObjects. Our plugin embeds QML via QQuickWidget so we
    # need a richer set of Qt modules. Find them up-front and pass
    # via LINK_TARGETS so the framework wires them into the plugin
    # target (including PUBLIC include paths for AUTOMOC).
    find_package(Qt6 REQUIRED COMPONENTS
        Widgets Quick QuickWidgets Qml Concurrent Gui Network OpenGL)
    # The SDK transitively pulls these but doesn't propagate the
    # include directories to consumers — we re-find them so the
    # imported targets are available for LINK_TARGETS below.
    find_package(nlohmann_json REQUIRED)
    find_package(OpenSSL REQUIRED)
    # Boost 1.90+ uses BoostConfig.cmake — Boost::system rather than
    # FindBoost.cmake's COMPONENTS arg. Try the modern form first.
    find_package(Boost CONFIG REQUIRED)

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
        EXTERNAL_LIBS lp0017_ffi
        LINK_TARGETS Qt6::Widgets Qt6::Quick Qt6::QuickWidgets
                     Qt6::Qml Qt6::Concurrent Qt6::Gui Qt6::Network Qt6::OpenGL
                     nlohmann_json::nlohmann_json
                     OpenSSL::SSL Boost::system
    )

    # Compile the QML resource into the plugin target. The framework
    # cmake doesn't have first-class QML handling so we wire it here.
    qt_add_resources(whistleblower_module_plugin "qml"
        PREFIX "/qml"
        BASE "qml"
        FILES qml/Main.qml
    )

else()
    set(CMAKE_AUTOMOC ON)
    set(CMAKE_AUTORCC ON)
    find_package(Qt6 REQUIRED COMPONENTS
        Core Gui Widgets Quick QuickWidgets Qml Concurrent
    )

    # Qt's moc resolves Q_PLUGIN_METADATA(FILE "...") relative to the
    # location of the header that declares the plugin (here src/plugin.h).
    # Stage a copy of metadata.json into src/ so moc finds it without
    # us having to relativise the path in the header.
    configure_file(${CMAKE_CURRENT_SOURCE_DIR}/metadata.json
                   ${CMAKE_CURRENT_SOURCE_DIR}/src/metadata.json COPYONLY)

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
