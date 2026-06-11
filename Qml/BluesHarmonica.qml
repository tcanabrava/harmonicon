import QtQuick 2.15
import QtQuick.Window 2.15
import QtQuick.Controls 2.15 as QQC2
import org.kde.kirigami 2.18 as Kirigami
import QtQuick.Layouts 1.12
import QtWebView 1.15

import orin.music.harmonica 1.0 as Orin

Kirigami.Page {
    // duration of the beat in milisseconds
    property double beatTime: 60 / harmonicasheet.bpmTotal * 1000;
    property double currentTime: 0
    property double delta_increment: ((noteFlow.spacing + noteFlow.baseHeight) * updateTimer.interval) / beatTime
    property int timeCounter: 0

    title: qsTr("Blues Harmonica")

    PartitureChooserOverlay {
        id: partitureOverlay
        onPartitureChoosed: {
            harmonicasheet.file = folder + "/" + fileName
            title.text = qsTr("Song: ") + fileName
            close();
        }
    }

    Orin.HarmonicaSheet {
        id: harmonicasheet
        onSendBeat: {
            chordProgression.paintHolesSoundData(beat)
            twelveBarProgression.advance()
        }
        onErrorMessage: {
            errorText.text = message
        }
        // the metronome follows the speed of the loaded song.
        onReadyChanged: {
            if (harmonicasheet.ready) {
                metronome.bpm = harmonicasheet.bpmTotal
            }
        }
        onBpmTotalChanged: {
            if (harmonicasheet.ready) {
                metronome.bpm = harmonicasheet.bpmTotal
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        Text {
            id: title
            Layout.alignment: Qt.AlignHCenter
        }
        Text {
            id: errorText
        }
        QQC2.SplitView {
            orientation: Qt.Horizontal
            Layout.fillWidth: true
            Layout.fillHeight: true
            ColumnLayout {
                id: mainLayout
                QQC2.SplitView.minimumWidth: 400

                BluesHarmonicaChordProgression {
                    id: chordProgression
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Item {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 264
                        y: 29
                        height: parent.height - 10
                        clip: true
                        Timer {
                            id: updateTimer
                            repeat: true
                            interval: 20 // 60hz, maybe add support for 120hz later?
                            onRunningChanged: {
                                noteFlow.y = 0
                            }
                            onTriggered: {
                                // delta_increment. the 0.01 is the error correction.
                                // because the delta_increment is a real, it will always
                                // add an error that we need to remove from time to time.
                                // this calculation here is incorrect, and we need to
                                // come with a better algorithm to fix the error correction
                                // values based on the bpm of the song.
                                // this value was tested for bpms of 60, 120 and 180.
                                // it's clear that this will break after a few loops.
                                let error_correction = (timeCounter % 10 === 0) ? 0.1 : 0.0

                                noteFlow.y = noteFlow.y - delta_increment + error_correction
                                timeCounter += 1
                            }
                        }
                        HarmonicaNoteFlow {
                            id: noteFlow
                            x: 0
                            y: 0
                            sheet: harmonicasheet
                            anchors.horizontalCenter: parent.horizontalCenter
                        }
                    }
                }

                TwelveBarProgression {
                    id: twelveBarProgression
                    Layout.preferredHeight: mainLayout.height * 0.33
                    Layout.preferredWidth: mainLayout.width
                    enabled: harmonicasheet.ready
                }
            }
            SheetInformation {
                sheet: harmonicasheet
            }
        }
        Metronome {
            id: metronome
            Layout.fillWidth: true
        }
        ControlBar {
            id: controlBar
            ready: harmonicasheet.ready
            Layout.fillWidth: true
            onRequestPartiture: partitureOverlay.open()
            onRequestPlay: {
                updateTimer.running = true
                harmonicasheet.start()
            }
            onRequestPause: {
                updateTimer.running = false
                harmonicasheet.pause()
            }
            onRequestStop: {
                updateTimer.running = false
                harmonicasheet.stop()
                chordProgression.clear()
                twelveBarProgression.clear()
            }
        }
    }
}

