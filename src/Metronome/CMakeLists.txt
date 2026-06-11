add_library(OrinMetronome STATIC
    Metronome.cpp
)

target_link_libraries(
    OrinMetronome
    PUBLIC Qt${QT_VERSION_MAJOR}::Core
)

if (BUILD_TESTS)
    target_compile_definitions(OrinMetronome PUBLIC TEST_BUILD)
    add_subdirectory(Unittest)
endif()
