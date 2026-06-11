add_subdirectory(Harmonica)
add_subdirectory(Metronome)

set(PROJECT_SOURCES
        main.cpp
        ../qml.qrc
)


compile_configuration(PROJECT_SOURCES ../preferences.conf)


if(${QT_VERSION_MAJOR} GREATER_EQUAL 6)
    qt_add_executable(Orin
        MANUAL_FINALIZATION
        ${PROJECT_SOURCES}
    )
else()
    if(ANDROID)
        add_library(Orin SHARED
            ${PROJECT_SOURCES}
        )
    else()
        add_executable(Orin
          ${PROJECT_SOURCES}
        )
    endif()
endif()

target_compile_definitions(Orin
  PRIVATE $<$<OR:$<CONFIG:Debug>,$<CONFIG:RelWithDebInfo>>:QT_QML_DEBUG>)

target_link_libraries(Orin
  PRIVATE OrinHarmonica
  PRIVATE OrinMetronome
  PRIVATE Qt${QT_VERSION_MAJOR}::Core
  PRIVATE Qt${QT_VERSION_MAJOR}::Widgets
  PRIVATE Qt${QT_VERSION_MAJOR}::WebEngine

  Qt${QT_VERSION_MAJOR}::Quick
  KF5::Kirigami2
 )

set_target_properties(Orin PROPERTIES
    MACOSX_BUNDLE_GUI_IDENTIFIER orin.tcanabrava.org
    MACOSX_BUNDLE_BUNDLE_VERSION ${PROJECT_VERSION}
    MACOSX_BUNDLE_SHORT_VERSION_STRING ${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR}
)

if(QT_VERSION_MAJOR EQUAL 6)
    qt_import_qml_plugins(Orin)
    qt_finalize_executable(Orin)
endif()
