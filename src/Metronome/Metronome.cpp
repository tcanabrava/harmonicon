#include "Metronome.h"

#include <algorithm>

#ifdef TEST_BUILD
#include <iostream>
#endif

Metronome::Metronome()
{
    // the default coarse timer is allowed to drift 5% to save battery,
    // which is audible on a metronome. ask for millisecond precision.
    m_timer.setTimerType(Qt::PreciseTimer);
    connect(&m_timer, &QTimer::timeout, this, &Metronome::timerTick);
}

int Metronome::bpm() const
{
    return m_bpm;
}

void Metronome::setBpm(int beatsPerMinute)
{
    const int clamped = std::clamp(beatsPerMinute, minBpm, maxBpm);
    if (clamped == m_bpm) {
        return;
    }

    m_bpm = clamped;
    if (m_running) {
        // setInterval on an active timer restarts it with the new interval.
        m_timer.setInterval(beatInterval());
    }

    Q_EMIT bpmChanged();
}

int Metronome::beatsPerMeasure() const
{
    return m_beatsPerMeasure;
}

void Metronome::setBeatsPerMeasure(int beats)
{
    const int clamped = std::clamp(beats, minBeatsPerMeasure, maxBeatsPerMeasure);
    if (clamped == m_beatsPerMeasure) {
        return;
    }

    m_beatsPerMeasure = clamped;

    // restart the measure so the accent lands on the next beat.
    if (m_running) {
        m_currentBeat = -1;
    }

    Q_EMIT beatsPerMeasureChanged();
}

bool Metronome::running() const
{
    return m_running;
}

int Metronome::currentBeat() const
{
    return m_currentBeat;
}

int Metronome::beatInterval() const
{
    return qRound(60.0 / m_bpm * 1000);
}

void Metronome::start()
{
    if (m_running) {
        return;
    }

    m_running = true;
    m_currentBeat = -1;
    m_timer.start(beatInterval());
    Q_EMIT runningChanged(m_running);

    // beat one fires immediately, the timer handles the following ones.
    timerTick();
}

void Metronome::stop()
{
    if (!m_running) {
        return;
    }

    m_timer.stop();
    m_running = false;
    m_currentBeat = -1;
    Q_EMIT runningChanged(m_running);
    Q_EMIT currentBeatChanged();
}

void Metronome::toggle()
{
    if (m_running) {
        stop();
    } else {
        start();
    }
}

void Metronome::timerTick()
{
    m_currentBeat = (m_currentBeat + 1) % m_beatsPerMeasure;
    Q_EMIT currentBeatChanged();
    Q_EMIT tick(m_currentBeat, m_currentBeat == 0);
}

#ifdef TEST_BUILD
int Metronome::testMetronome()
{
    Metronome metronome;

    // bpm is clamped to the playable range.
    metronome.setBpm(10000);
    if (metronome.bpm() != Metronome::maxBpm) {
        std::cerr << "Bpm not clamped to maximum";
        return 1;
    }
    metronome.setBpm(-5);
    if (metronome.bpm() != Metronome::minBpm) {
        std::cerr << "Bpm not clamped to minimum";
        return 1;
    }

    // the beat interval follows the bpm: 120 bpm = 500ms.
    metronome.setBpm(120);
    if (metronome.beatInterval() != 500) {
        std::cerr << "Wrong interval for 120 bpm: " << metronome.beatInterval();
        return 1;
    }
    metronome.setBpm(60);
    if (metronome.beatInterval() != 1000) {
        std::cerr << "Wrong interval for 60 bpm: " << metronome.beatInterval();
        return 1;
    }

    // beatsPerMeasure is clamped too.
    metronome.setBeatsPerMeasure(0);
    if (metronome.beatsPerMeasure() != Metronome::minBeatsPerMeasure) {
        std::cerr << "BeatsPerMeasure not clamped to minimum";
        return 1;
    }
    metronome.setBeatsPerMeasure(100);
    if (metronome.beatsPerMeasure() != Metronome::maxBeatsPerMeasure) {
        std::cerr << "BeatsPerMeasure not clamped to maximum";
        return 1;
    }

    // beats cycle inside the measure and the accent lands on beat zero.
    metronome.setBeatsPerMeasure(4);
    int ticksSeen = 0;
    int accentsSeen = 0;
    bool beatsInOrder = true;
    bool accentsCorrect = true;
    QObject::connect(&metronome, &Metronome::tick, [&](int beat, bool accent) {
        beatsInOrder = beatsInOrder && (beat == ticksSeen % 4);
        accentsCorrect = accentsCorrect && (accent == (beat == 0));
        ticksSeen += 1;
        accentsSeen += accent ? 1 : 0;
    });

    // drive two full measures by hand, no event loop needed.
    metronome.m_running = true;
    metronome.m_currentBeat = -1;
    for (int i = 0; i < 8; i++) {
        metronome.timerTick();
    }

    if (!beatsInOrder) {
        std::cerr << "Beats emitted out of order";
        return 1;
    }
    if (!accentsCorrect) {
        std::cerr << "Accent on the wrong beat";
        return 1;
    }
    if (ticksSeen != 8) {
        std::cerr << "Expected 8 ticks, got " << ticksSeen;
        return 1;
    }
    if (accentsSeen != 2) {
        std::cerr << "Expected 2 accents, got " << accentsSeen;
        return 1;
    }
    if (metronome.currentBeat() != 3) {
        std::cerr << "Expected to end on beat 3, got " << metronome.currentBeat();
        return 1;
    }

    // stop resets the position.
    metronome.stop();
    if (metronome.running() || metronome.currentBeat() != -1) {
        std::cerr << "Stop did not reset the metronome";
        return 1;
    }

    return 0;
}
#endif
