#include "../Metronome.h"

#include <QCoreApplication>

int main(int argc, char *argv[]) {
    // QTimer needs a Qt application context, even if no event loop runs.
    QCoreApplication app(argc, argv);

    int ret = Metronome::testMetronome();
    if (ret != 0) {
        return ret;
    }
}
