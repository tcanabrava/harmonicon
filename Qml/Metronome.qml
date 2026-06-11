import QtQuick 2.15
import QtQuick.Window 2.15
import QtQuick.Controls 2.15 as QQC2
import org.kde.kirigami 2.18 as Kirigami
import QtQuick.Layouts 1.12
import QtMultimedia 5.15

import orin.music.metronome 1.0 as Orin

RowLayout {
    id: root

    property alias bpm: engine.bpm
    readonly property bool running: engine.running

    Orin.Metronome {
        id: engine

        // remember the bpm between sessions.
        Component.onCompleted: bpm = preferences.general.beatsPerMinute
        onBpmChanged: preferences.general.beatsPerMinute = bpm

        onTick: {
            if (muteButton.checked) {
                return
            }
            if (accent) {
                tickHigh.play()
            } else {
                tickLow.play()
            }
        }
    }

    SoundEffect {
        id: tickHigh
        source: "qrc:/Sounds/tick_high.wav"
    }

    SoundEffect {
        id: tickLow
        source: "qrc:/Sounds/tick_low.wav"
    }

    Connections {
        target: engine
        function onBpmChanged() {
            // user interaction breaks the SpinBox binding, restore it
            // when the bpm changes from outside (song load, for instance).
            bpmSpinBox.value = engine.bpm
        }
    }

    Item {
        Layout.fillWidth: true
    }

    QQC2.Label {
        text: qsTr("Metronome")
    }

    Row {
        spacing: 6
        Layout.alignment: Qt.AlignVCenter
        Repeater {
            model: engine.beatsPerMeasure
            Rectangle {
                width: 14
                height: 14
                radius: width / 2
                border.width: 1
                border.color: "gray"
                // the accent beat lights up in a different color.
                color: engine.currentBeat === index
                       ? (index === 0 ? "#FF8800" : "#0088FF")
                       : "transparent"
            }
        }
    }

    QQC2.SpinBox {
        id: bpmSpinBox
        // keep the range in sync with the limits in Metronome.h
        from: 20
        to: 240
        editable: true
        value: engine.bpm
        onValueModified: engine.bpm = value
    }

    QQC2.Label {
        text: qsTr("bpm")
    }

    QQC2.SpinBox {
        id: beatsSpinBox
        from: 1
        to: 12
        value: engine.beatsPerMeasure
        onValueModified: engine.beatsPerMeasure = value
    }

    QQC2.Label {
        text: qsTr("beats")
    }

    QQC2.Button {
        text: engine.running ? qsTr("Stop") : qsTr("Start")
        onClicked: engine.toggle()
    }

    QQC2.Button {
        id: muteButton
        checkable: true
        text: qsTr("Mute")
    }

    Item {
        Layout.fillWidth: true
    }
}
