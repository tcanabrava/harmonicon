= English =

== What it is ==

Orin is a software for learning Blues Harmonica, 10 hole harmonica.
It will probably be improved for other instruments in the future.

== Compilation ==

You need:
    - A C++ Compiller (clang or gcc)
    - cmake
    - Qt
    - KDE Kirigami
    - confgen

with all of those software installed, you can configure the project as usual using cmake:

```
$ git clone orin
$ mkdir build
$ cd build
$ cmake ../../orin -DCMAKE_INSTALL_PREFIX=/home/user/orin
$ make
$ make install
```

== Usage ==

Then open the application, it shall show you a selector for flute and harmonica, only harmonica is implemented
currently.
click on load a song, and start the music.
Follow the notes with the harmonica.

A metronome is available at the bottom of the harmonica page. It clicks with an accent
on the first beat of the measure, the bpm and the beats per measure are adjustable, and
the bpm follows the loaded song automatically.

= Português =

== O que é ==

Orin é um software para ensino de instrumentos musicais baseado em acompanhar notas indo de encontro aos instrumentos.

== Compilação ==

Voce precisa:
    - Um compilador de C++ (clang ou gcc)
    - cmake
    - Qt
    - KDE Kirigami
    - confgen

Com isso instalado, compile o software como de costume.

```
$ git clone orin
$ mkdir build
$ cd build
$ cmake ../../orin -DCMAKE_INSTALL_PREFIX=/home/user/orin
$ make
$ make install
```

== Uso das coisas ==

Abra a aplicação, isso irá te mostrar um seletor de flauta e gaita, mas no momento apenas gaita esta implementado.
Escolha gaita, e selecione uma das musicas exemplo, na pasta Orin/Examples. Carregue a musica, e siga as notas no instrumento.

Um metrônomo está disponível na parte inferior da página da gaita. Ele toca um clique com
acento no primeiro tempo do compasso, o bpm e os tempos por compasso são ajustáveis, e o
bpm acompanha automaticamente a música carregada.
