# Harmonicon — Español (es-ES) UI strings.
#
# Mantén las claves sincronizadas con assets/locales/en-US/main/ui.ftl.

app-title = Harmonicon

# Menú principal
menu-play = Jugar
menu-song-editor-2 = Editor de Canciones
menu-options = Opciones
menu-credits = Créditos
menu-tutorial = Tutorial
menu-quit = Salir

# Menú de juego
play-song = Tocar Canción
jam-session = Sesión Jam
jam-generate = Generar Jam
bending-trainer = Entrenador de Bends

# Selección de modo
select-mode = Seleccionar Modo
play-2d = Jugar en 2D
play-3d = Jugar en 3D

# Generar Jam (base sintetizada, sin necesidad de una canción)
jam-generate-title = Generar una Base de Jam
jam-generate-start = Empezar la Jam

# Selección de canción / artista
select-artist = Seleccionar Artista
select-song = Seleccionar Canción
no-songs-found = No se encontraron canciones. Añade carpetas en assets/songs/<artista>/<canción>/

# Opciones
options-title = Opciones
options-language = Idioma

# Compartido
back = ← Volver

# Song Editor 2 — botones de transporte y panel de modificadores
editor-mode-edit = ✎ Editar
editor-mode-perform = 🎵 Interpretar
editor-lock = 🔒 Bloquear
editor-play = ▶ Reproducir
editor-pause = ⏸ Pausar
editor-stop = ■ Detener
editor-practice = 🎤 Practicar
editor-save = 💾 Guardar
editor-load = 📂 Cargar
editor-browse = 📂 Examinar
mod-blow = Soplar
mod-draw = Aspirar
mod-bend = Doblar
mod-overblow = Oversoplo
mod-overdraw = Overaspiración
mod-slide = Slide
mod-wah = Wah
mod-vibrato = Vibrato
mod-delete = Eliminar

# Song Editor 2 — etiquetas de los campos de metadatos
editor-field-tempo = Tempo de la Música
editor-field-key = Tono de la Armónica
editor-field-position = Posición
editor-field-harmonica = Armónica
editor-field-music = Música de Fondo
editor-field-name = Nombre
editor-field-author = Autor
editor-harmonica-diatonic = ‹ Diatónica (10 orificios) ›
editor-harmonica-chromatic = ‹ Cromática (12 orificios) ›

# Song Editor 2 — títulos de diálogos de archivo
dialog-save-chart = Guardar partitura
dialog-load-chart = Cargar partitura
dialog-select-music = Seleccionar música de fondo

# Song Editor 2 — mensajes de validación al arrastrar
drag-denied-bend = Este orificio no admite esta profundidad de doblado
drag-denied-overblow = El oversoplo solo está disponible en los orificios 1–6
drag-denied-overdraw = La overaspiración solo está disponible en los orificios 7–10
drag-denied-overlap = Ya hay otra nota aquí

# Song Editor 2 — mensajes del modo de práctica
practice-no-music = No hay música de fondo configurada — ¡toca junto con la partitura!
practice-prompt = ▶ Toca %note%…
practice-wrong-note = ▶ %got% → se necesita %expected%
practice-hit-perfect = ✓ PERFECTO  %note%  +%pts% pts
practice-hit-good = ✓ BIEN  %note%  +%pts% pts
practice-missed = ✗ Fallaste %note%
practice-done = Hecho — %hits%/%total% notas  ·  %score% pts

# Song Editor 2 — descripciones de los botones
editor-back-tooltip = Salir del editor y volver al menú principal
editor-mode-edit-tooltip = Cambiar al modo Editar — coloca, mueve y edita notas en la cuadrícula
editor-mode-perform-tooltip = Cambiar al modo Interpretar — reproduce o practica la partitura
editor-lock-tooltip = Bloquear la cuadrícula para evitar ediciones accidentales al revisar
editor-save-tooltip = Guardar esta partitura en un archivo .harpchart
editor-load-tooltip = Cargar una partitura desde un archivo .harpchart
editor-play-tooltip = Iniciar o reanudar la reproducción de la partitura
editor-pause-tooltip = Pausar la reproducción en el mismo punto
editor-stop-tooltip = Detener la reproducción y volver el cursor al inicio
editor-practice-tooltip = Modo práctica — toca junto con tu armónica y recibe retroalimentación en vivo
mod-blow-tooltip = Establecer la nota seleccionada como soplo (exhalar)
mod-draw-tooltip = Establecer la nota seleccionada como aspiración (inhalar)
mod-bend-tooltip = Alternar la profundidad de doblado de la nota seleccionada: ninguno → medio tono → tono completo → tono y medio
mod-overblow-tooltip = Establecer la nota seleccionada como oversoplo (técnica avanzada de soplo, solo diatónica)
mod-overdraw-tooltip = Establecer la nota seleccionada como overaspiración (técnica avanzada de aspiración, solo diatónica)
mod-slide-tooltip = Establecer la nota seleccionada para usar el botón slide (solo armónicas cromáticas)
mod-wah-tooltip = Alternar la velocidad de wah-wah de la nota seleccionada
mod-vibrato-tooltip = Alternar la velocidad de vibrato de la nota seleccionada
mod-delete-tooltip = Eliminar la nota seleccionada
editor-harmonica-toggle-tooltip = Haz clic para alternar entre armónica Diatónica y Cromática
editor-field-key-tooltip = Haz clic para recorrer los tonos de la armónica
editor-field-position-tooltip = Haz clic para recorrer las posiciones de interpretación
editor-browse-tooltip = Elegir un archivo de audio de música de fondo para esta partitura

# Lecciones — menú, lector, veredicto en resultados
menu-lessons = Lecciones
no-lessons-found = No se encontraron lecciones. Añade carpetas en assets/lessons/<unidad>/<lección>/
lesson-locked = bloqueada
lesson-passed = Superada
lesson-start = Empezar la Lección
lesson-mark-done = Marcar como Hecha
lesson-goal-accuracy = Objetivo: %pct%% de precisión general
lesson-goal-technique = Objetivo: %pct%% de precisión en las notas de %technique%
lesson-goal-finish = Objetivo: tocarla hasta el final
lesson-goal-scale-adherence = Objetivo: %pct%% de las notas dentro de la escala o mejor
lesson-complete-banner = LECCIÓN SUPERADA
lesson-failed-banner = Objetivo no alcanzado — relee la lección e inténtalo de nuevo

# Lecciones — títulos de unidad (la clave sale del campo "unit" de cada lesson.json)
lesson-unit-blowing = Unidad 1 · Soplar la Armónica
lesson-unit-rhythm = Unidad 2 · Contar el Blues

# Lección: nota única
lesson-single-note-title = Tocar una Sola Nota
lesson-single-note-body =
    El mayor obstáculo del principiante con la armónica: sacar una nota limpia en lugar de un acorde de vecinas.
    Frunce los labios como para silbar, o di la sílaba "tu" — la abertura debe ser apenas más ancha que un agujero.
    Relájate: la armónica entra profunda entre los labios, apoyada en la parte interna húmeda, no sujeta por el borde seco.
    Inclina la parte trasera de la armónica ligeramente hacia arriba y deja caer la mandíbula, para que el aire salga lento y cálido, desde el vientre.
    En este ejercicio, notas largas en los agujeros 4, 5 y 6 se deslizan hacia la línea de acierto. Sopla cada una con suavidad — no importa el volumen, importa la pureza.
    Si oyes dos notas a la vez, no aprietes: estrecha un poco la abertura y frena la respiración.

# Lección: varias notas (acordes)
lesson-multiple-notes-title = Tocar Varias Notas a la Vez
lesson-multiple-notes-body =
    Una sola nota no es el único objetivo — algunos riffs de blues suenan a propósito dos o tres agujeros juntos, como un acorde.
    Ensancha la embocadura para cubrir los agujeros que quieres y ninguno más allá; el mismo control de aire que te dio una nota limpia ahora te da un grupo controlado de ellas.
    Los acordes de soplido van en agujeros vecinos: los agujeros 1-2-3 soplados juntos suenan un brillante acorde de Do mayor.
    Los acordes de aspirado funcionan igual: los agujeros 2-3-4 aspirados juntos suenan un acorde de Sol mayor.
    En este ejercicio, las levadas de acorde se deslizan hacia la línea de acierto — el juego escucha que cada nota del acorde suene en el mismo instante, no una tras otra.
    Si solo registra parte del acorde, seguramente no estás cubriendo todos los agujeros por igual; ensancha la embocadura en vez de soplar más fuerte.

# Lección: bloqueo de lengua (instructiva)
lesson-tongue-blocking-title = Bloqueo de Lengua
lesson-tongue-blocking-body =
    Hasta ahora diste forma a las notas con los labios (frunciendo) — el bloqueo de lengua es la otra embocadura clásica: cubre varios agujeros con la boca, y apoya la lengua plana sobre la armónica para bloquear todos menos uno.
    Levanta la lengua de un agujero y suena solo, exactamente igual que una nota única con los labios fruncidos — el micrófono realmente no puede distinguir las dos técnicas, así que esta lección no puede verificar cuál estás usando.
    Lo que el bloqueo de lengua desbloquea y el fruncido no puede: aparta la lengua de dos agujeros de los extremos a la vez (bloqueando solo los del medio) y consigues una división de octava — dos notas, una octava de distancia, sonando juntas.
    También te deja golpear la lengua contra un agujero rítmicamente para un pulso percusivo tipo "chaca-chaca", y cambiar de esquina de la boca a mitad de frase sin perder el sello de aire.
    Prueba la lección de división de octava a continuación — es la recompensa concreta y medible de esta técnica: el juego puede oír si las dos notas de la división suenan juntas, aunque no pueda oír el bloqueo de lengua en sí.

# Lección: división de octava (bloqueo de lengua)
lesson-octave-split-title = Divisiones de Octava
lesson-octave-split-body =
    El bloqueo de lengua permite tocar dos agujeros a la vez, silenciando los que quedan entre ellos — el clásico es la división de octava.
    Apoya la lengua plana sobre la armónica, cubriendo los dos agujeros del medio, y deja pasar el aire solo por el agujero de cada lado.
    En los agujeros 1 y 4 soplados juntos suenan Do4 y Do5 — la misma nota, una octava más arriba. Los agujeros 2 y 5, y los agujeros 3 y 6, funcionan igual.
    En este ejercicio, los dos agujeros de cada división deben sonar juntos, igual que un acorde — el bloqueo de lengua en sí no se puede verificar por el micrófono, pero la octava que produce sí.
    Si solo oyes una nota, revisa que la lengua cubra del todo los agujeros del medio, en vez de quedar ladeada hacia un lado.

# Lección: deslizamientos
lesson-slides-title = Deslizamientos
lesson-slides-body =
    Dos técnicas distintas comparten el nombre "slide" en la armónica — este ejercicio cubre las dos.
    La primera es un deslizamiento físico: mueve la armónica de lado por tu embocadura, de un agujero al siguiente, manteniendo el sello sin romperlo en vez de parar y reiniciar la respiración en cada nota. En este ejercicio, desliza suavemente por los agujeros 4-5-6 soplados — el juego escucha tres notas normales, pero la técnica está en cómo las conectas, no solo en tocarlas bien.
    La segunda es una liberación de bend: ataca una nota ya doblada hacia abajo, y déjala subir suavemente hasta la nota natural — un lamento clásico del blues. En este ejercicio, dobla el agujero 2 aspirado medio tono hacia abajo y sostenla, luego libérala suavemente hasta la nota natural; el juego valida la nota doblada en el momento en que la tocas.
    Mantén el aire constante en ambas — el deslizamiento debe sonar como una sola respiración continua, no una serie de ataques separados.

# Lección: forma de las manos / wah
lesson-hand-wah-title = La Forma de las Manos y el Wah
lesson-hand-wah-body =
    Tus manos son el control de timbre de la armónica. Ahuécalas detrás del instrumento formando una cámara de aire sellada, y abre y cierra el sello para que "hable": "ua".
    Sujeta la armónica entre el pulgar y el índice de una mano, y sella la otra mano detrás, como una almeja.
    Copa cerrada = sonido oscuro, apagado. Copa abierta = brillante y fuerte. Abrir la copa rítmicamente mientras suena una nota produce el clásico wah-wah.
    En este ejercicio, sostén cada nota con firmeza y abre-y-cierra la copa unas dos veces por segundo — el juego escucha ese pulso en tu sonido.
    Mantén la respiración constante; solo se mueven las manos. Si no registra nada, aprieta el sello de la copa — casi todo el efecto vive en el último centímetro del cierre.

# Lección: llamada y respuesta
lesson-call-response-title = Llamada y Respuesta
lesson-call-response-body =
    Esto es llamada y respuesta: el juego toca una frase corta, y luego es tu turno de tocarla de vuelta.
    Escucha la demo sintetizada — una serie de una, dos, y luego tres notas — y repite exactamente lo que oíste, a tu propio ritmo; el juego se congela y te espera, el tiempo que necesites.
    Aquí no hay prisa ni reloj corriendo en tu contra: solo importa la nota, no el tiempo.
    Si tocas la nota equivocada, no pasa nada — el juego solo sigue esperando hasta que aciertes, así que escúchala de nuevo en tu cabeza e inténtalo otra vez.
    Esta es la misma habilidad de "escuchar y tocar" que usarás improvisando con otros músicos: alguien toca una frase, tú respondes.

# Lección: improvisación
lesson-improvisation-title = Improvisar sobre el Blues
lesson-improvisation-body =
    Ahora toca juntarlo todo: la forma de 12 compases, la escala de blues y tus propias decisiones, tocadas en vivo sobre una jam de verdad.
    Esta lección abre una Jam Session normal — la rejilla de 12 compases y el mapa de agujeros de tu armónica cambian de color en vivo mientras tocas: dorado significa que tocaste una nota del acorde que suena ahora mismo, verde significa que estás dentro de la escala de blues, ámbar significa que saliste de ella.
    Esto es 2ª posición: tu armónica en Do toca en el tono de Sol, el clásico esquema cross-harp del blues — el agujero 2 aspirado es tu nota base.
    No hay una melodía fija que acertar; toca lo que quieras sobre los acordes y deja que tu oído siga el color del mapa de agujeros.
    Cuando sientas que estás listo para parar, abre el menú de pausa y pulsa "Finish Lesson" — el juego cuenta cuántas de tus notas cayeron dentro de la escala o sobre un tono del acorde y juzga el ejercicio con eso.
    Apunta a verde y dorado la mayor parte del tiempo; alguna nota ámbar de vez en cuando es normal, hasta expresiva — solo no te quedes ahí.

# Lección: leer la rejilla de 12 compases
lesson-twelve-bar-title = Leer la Rejilla del Blues de 12 Compases
lesson-twelve-bar-body =
    Casi toda canción de blues sigue el mismo ciclo de 12 compases — aprende a leerlo una vez y podrás seguir cualquier jam de blues del planeta.
    Cada celda de la rejilla es un compás de cuatro tiempos. Los números romanos nombran los acordes: I es el acorde de casa, IV el viaje intermedio, V la tensión del regreso.
    El esquema clásico: cuatro compases de I, dos de IV, dos de I, uno de V, uno de IV y dos compases finales de I (el último suele cambiarse a V para lanzar el siguiente chorus — el "turnaround").
    Cuéntalo en voz alta: "UN dos tres cuatro, DOS dos tres cuatro..." — doce compases, y el ciclo vuelve a empezar.
    Verás esta rejilla en vivo en la Jam Session, donde el compás actual se ilumina mientras suena la base. Después de esta lección, abre una Jam Session y solo mira pasar unos ciclos, contando, antes de tocar una sola nota.

# Lección: usar los pies
lesson-using-your-feet-title = Usar los Pies
lesson-using-your-feet-body =
    El buen sentido del tiempo no viene de mirar la pantalla — viene de tu cuerpo. Marca el pie en cada tiempo, y deja que ese pulso físico guíe tu forma de tocar en vez de perseguir las notas mientras se deslizan.
    Antes de empezar, cuenta "1, 2, 3, 4" en voz alta varias veces al tempo del ejercicio, marcando el pie en cada número, hasta que se sienta automático en vez de contado.
    En este ejercicio, un pulso constante de negras se desliza en el agujero 4 — sigue marcando el pie todo el tiempo, incluso entre notas, y deja que cada soplido/aspirado caiga exactamente en una marca.
    La ventana de tiempo aquí es más estrecha que en otros ejercicios a propósito: esta lección trata enteramente de caer en el tiempo, no de la nota ni de la técnica.
    Si vas siempre adelantado o atrasado, no mires el camino de notas — cierra los ojos y sigue solo tu pie.

# Juego — cuenta atrás, leyenda, pistas del diagrama de armónica
gameplay-get-ready = PREPÁRATE
gameplay-legend-blow = ■ SOPLO
gameplay-legend-draw = ■ ASPIRACIÓN
harmonica-overlay-hint-view = Armónica  ·  se ilumina mientras tocas
harmonica-overlay-hint-select = Armónica  ·  haz clic en una nota para seleccionarla

# Overlay del metrónomo
metronome-click-off = clic: apagado
metronome-click-on = clic: encendido

# Entrenador de Bends
bending-drill-off = Ejercicio: apagado
bending-drill-on = Ejercicio: encendido · racha %streak%
bending-hint = Esc para volver  ·  M silencia el clic  ·  feel alterna recto/shuffle
bending-no-note-for-technique = Este agujero no tiene nota para esa técnica.

# Jam Session
jam-loop-off = Bucle: apagado
jam-loop-on = Bucle: encendido
jam-hole-map-hint = Tu armónica  ·  dorado = tono del acorde ahora mismo  ·  verde = nota de la escala de blues  ·  soplo arriba / aspiración abajo

# Pantalla de resultados
results-song-complete = CANCIÓN COMPLETADA
results-by-technique = Por técnica
results-new-best = ★ ¡NUEVO RÉCORD! ★

# Calibración de latencia
calibration-title = Calibración de Latencia
calibration-mean-offset-placeholder = Desfase medio: —
calibration-mean-offset = Desfase medio: %sign%%ms%ms
calibration-suggested-placeholder = Actual: —   →   Sugerido: —
calibration-suggested = Actual: %current%ms   →   Sugerido: %suggested%ms

# Opciones
options-input-lag = Retardo de entrada

# Recorrido guiado del tutorial (menu::tutorial)
tutorial-step = Paso %n% de %total%
tutorial-skip = Saltar Tutorial
tutorial-title-main = Menú Principal
tutorial-body-main = Tu base — entra a una canción, explora las lecciones o abre Opciones desde aquí.
tutorial-title-play = Jugar
tutorial-body-play = Elige una canción real, empieza una jam libre o practica bends — elige cómo quieres jugar.
tutorial-title-mode-select = Seleccionar Modo
tutorial-body-mode-select = Elige 2D (un camino de notas que se desliza) o 3D (una armónica que tocas junto a ti).
tutorial-title-gameplay = Tocando una Canción
tutorial-body-gameplay = Las notas caen hacia la línea de acierto — toca la nota correcta en tu armónica en el momento justo para anotar.
tutorial-title-jam-session = Jam Session
tutorial-body-jam-session = Juego libre: la rejilla de 12 compases y un mapa de agujeros en vivo guían tu improvisación — nada aquí se puntúa.
tutorial-title-bending-trainer = Entrenador de Bends
tutorial-body-bending-trainer = Practica bends de forma aislada: elige un objetivo en el diagrama, escúchalo y luego intenta igualarlo.
tutorial-title-options = Opciones
tutorial-body-options = El volumen, el estilo de las notas, el modelo de armónica y la calibración del micrófono están aquí.
tutorial-title-theme = Tema
tutorial-body-theme = Elige un tema visual para los menús — cambia los fondos y el estilo de los botones.
tutorial-title-lessons = Lecciones
tutorial-body-lessons = Un plan guiado: notas únicas, acordes, bends e improvisación sobre el blues.
tutorial-title-jam-generate = Generar Jam
tutorial-body-jam-generate = Genera una base instantánea en cualquier tono y tempo — sin necesidad de una canción.
tutorial-title-song-editor = Editor de Canciones
tutorial-body-song-editor = Crea o edita una partitura en esta cuadrícula, luego reprodúcela o practica junto a ella en vivo.
