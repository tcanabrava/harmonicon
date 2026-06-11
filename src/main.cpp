#include <QApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QtWebEngine>
#include <QSurfaceFormat>

#include "Harmonica/HarmonicaSheet.h"
#include "Metronome/Metronome.h"

#include "preferences.h"

int main(int argc, char *argv[])
{
#if QT_VERSION < QT_VERSION_CHECK(6, 0, 0)
    QCoreApplication::setAttribute(Qt::AA_EnableHighDpiScaling);
#endif
    QtWebEngine::initialize();


    QApplication app(argc, argv);
    app.setOrganizationName("Tomaz");
    app.setOrganizationDomain("Canabrava");

    qmlRegisterType<HarmonicaSheet>("orin.music.harmonica", 1,0, "HarmonicaSheet");
    qmlRegisterType<Metronome>("orin.music.metronome", 1,0, "Metronome");
    qmlRegisterUncreatableType<HarmonicaSoundData>("orin.music.harmonica", 1,0, "HarmonicaSoundData", "Can use, not create");

    qRegisterMetaType<HarmonicaSoundData>("HarmonicaSoundData");

    QQmlApplicationEngine engine;

    engine.rootContext()->setContextProperty("preferences", Preferences::self());

    const QUrl url(QStringLiteral("qrc:/Qml/main.qml"));
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreated,
                     &app, [url](QObject *obj, const QUrl &objUrl) {
        if (!obj && url == objUrl)
            QCoreApplication::exit(-1);
    }, Qt::QueuedConnection);
    engine.load(url);
    int retValue = app.exec();

    Preferences::self()->sync();

    return retValue;
}
