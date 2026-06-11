#pragma once

#include <QObject>
#include <QTimer>

/* A practice metronome, independent of any loaded song.
 * The first beat of every measure carries the accent. */
class Metronome : public QObject {
    Q_OBJECT

    // Read write access from the interface
    Q_PROPERTY(int bpm READ bpm WRITE setBpm NOTIFY bpmChanged)
    Q_PROPERTY(int beatsPerMeasure READ beatsPerMeasure WRITE setBeatsPerMeasure NOTIFY beatsPerMeasureChanged)

    // Read only access from the interface
    Q_PROPERTY(bool running READ running NOTIFY runningChanged)
    Q_PROPERTY(int currentBeat READ currentBeat NOTIFY currentBeatChanged)

public:
    // keep the bpm inside a range a human can practice with.
    static constexpr int minBpm = 20;
    static constexpr int maxBpm = 240;

    static constexpr int minBeatsPerMeasure = 1;
    static constexpr int maxBeatsPerMeasure = 12;

    Metronome();

    int bpm() const;
    void setBpm(int beatsPerMinute);
    Q_SIGNAL void bpmChanged();

    int beatsPerMeasure() const;
    void setBeatsPerMeasure(int beats);
    Q_SIGNAL void beatsPerMeasureChanged();

    bool running() const;
    Q_SIGNAL void runningChanged(bool running);

    // the beat we are on inside the measure, -1 when stopped.
    int currentBeat() const;
    Q_SIGNAL void currentBeatChanged();

    // emitted on every beat. accent is true on the first beat of the measure.
    Q_SIGNAL void tick(int beat, bool accent);

    Q_INVOKABLE void start();
    Q_INVOKABLE void stop();
    Q_INVOKABLE void toggle();

#ifdef TEST_BUILD
    static int testMetronome();
#endif

private:
    // milliseconds between two beats.
    int beatInterval() const;

    // triggered by the timer.
    void timerTick();

    int m_bpm = 60;
    int m_beatsPerMeasure = 4;
    int m_currentBeat = -1;
    bool m_running = false;

    QTimer m_timer;
};
