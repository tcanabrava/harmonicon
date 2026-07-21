# Harmonicon — Español (es-ES) UI strings.
#
# Mantén las claves sincronizadas con assets/locales/en-US/main/ui.ftl.

app-title = Harmonicon

# Menú principal
menu-play = Jugar
menu-options = Opciones
menu-help = Ayuda / Acerca de
menu-credits = Créditos
menu-tutorial = Tutorial
menu-quit = Salir

# Menú de juego
play-song = Tocar Canción
menu-create-song = Crear Canción
jam-session = Sesión Jam
bending-trainer = Entrenador de Bends

# Submenú de Sesión Jam
jam-session-pick-song = Elegir una Canción
jam-generate = Generar Jam

# Menú de Ayuda / Acerca de
help-about-title = Ayuda / Acerca de
help-documentation = Documentación
help-docs-not-found = La documentación aún no se ha generado localmente — ejecuta `mdbook build` en docs/book/.
menu-about = Acerca de
about-title = Acerca de Harmonicon
about-body = Harmonicon es un juego de ritmo para armónica diatónica y cromática: toca una armónica real en el micrófono y se puntúa en tiempo real contra una partitura, creado para enseñar armónica de blues y jazz jugando.
about-version = Versión { $version }

# Selección de modo
select-mode = Seleccionar Modo
play-2d = Jugar en 2D
play-3d = Jugar en 3D

# Generar Jam (base sintetizada, sin necesidad de una canción)
jam-generate-title = Generar una Base de Jam
jam-generate-start = Empezar la Jam
jam-generate-key = Tono: %key%
jam-generate-tempo = Tempo: %bpm%
jam-generate-progression = Progresión: %progression%
jam-generate-position = Posición: %position%

# Créditos
credits-back-to-menu = Volver al Menú

# Selección de canción / artista
select-artist = Seleccionar Artista
select-song = Seleccionar Canción
no-songs-found = No se encontraron canciones. Añade carpetas en assets/songs/<artista>/<canción>/

# Opciones
options-title = Opciones
options-language = Idioma
options-adaptive-difficulty = Dificultad Adaptativa
options-adaptive-difficulty-on = Dificultad Adaptativa: activada
options-adaptive-difficulty-off = Dificultad Adaptativa: desactivada
options-fullscreen = Pantalla completa
options-fullscreen-on = Pantalla completa: activada
options-fullscreen-off = Pantalla completa: desactivada
options-pitch-detect = Detección de tono
options-microphone = Micrófono
options-note-labels-button = Etiquetas de notas
options-note-labels-arrows = Etiquetas de notas: flechas
options-note-labels-numbers = Etiquetas de notas: números
options-calibrate-input-lag = Calibrar la latencia de entrada
theme-back-to-options = ← Volver a Opciones

# Compartido
back = ← Volver

# Song Editor 2 — botones de transporte y panel de modificadores
editor-mode-edit = ✎ Editar
editor-mode-record = ⏺ Grabar
editor-mode-play = 🎵 Reproducir
editor-lock = 🔒 Bloquear
editor-play = ▶ Reproducir
editor-pause = ⏸ Pausar
editor-stop = ■ Detener
editor-practice = 🎤 Practicar
editor-finish = ⏹ Finalizar
editor-save = 💾 Guardar
editor-load = 📂 Cargar
editor-browse = 📂 Examinar
editor-import-midi = 🎹 Importar MIDI
mod-blow = Soplar
mod-draw = Aspirar
mod-bend = Doblar
mod-overblow = Oversoplo
mod-overdraw = Overaspiración
mod-slide = Slide
mod-wah = Wah
mod-vibrato = Vibrato
mod-delete = Eliminar
editor-tool-select = ✂ ⟕ Selecionar
editor-tool-erase = ✂ Borrar Tramo
editor-tool-remove = ✂ Quitar Tramo
editor-tool-tempo = ♩ Tempo

# Song Editor 2 — etiquetas de los campos de metadatos
editor-field-tempo = Tempo de la Música
editor-field-key = Tono de la Armónica
editor-field-position = Posición
editor-field-harmonica = Armónica
editor-field-music = Música de Fondo
editor-field-name = Nombre
editor-field-author = Autor
editor-field-midi-track = Pista MIDI
editor-harmonica-diatonic = ‹ Diatónica (10 orificios) ›
editor-harmonica-chromatic = ‹ Cromática (12 orificios) ›
editor-field-content-kind = Grabación
editor-content-kind-song = ‹ Grabar Canción ›
editor-content-kind-lesson = ‹ Grabar Lección ›

# Song Editor 2 — leyenda de colores (tercera columna del formulario)
editor-legend-notes = Colores de las notas (cuadrícula)
editor-legend-normal = Nota normal de soplo/aspiración
editor-legend-bend = Bend (cuanto más profundo, más rojo)
editor-legend-overblow = Overblow
editor-legend-overdraw = Overdraw
editor-legend-slide = Slide (solo cromática)
editor-legend-out-of-scale = Tinte rojo = fuera de la escala de la canción
editor-legend-selected = Borde dorado = nota seleccionada
editor-legend-blow = Soplo
editor-legend-draw = Aspiración
editor-legend-dragging = Al arrastrar una nota
editor-legend-drag-ok = Posición de destino válida
editor-legend-drag-bad = Inválida (superposición o técnica incompatible)
editor-legend-elsewhere = En otras partes de la pantalla
editor-legend-tempo-marker = Marcador de cambio de tempo (encabezado de la cuadrícula)
editor-legend-split-point = Herramienta Seleccionar: punto de división
editor-legend-range-preview = Herramienta Seleccionar: vista previa del rango
editor-legend-active-button = Botón de modo/herramienta actualmente activo
editor-legend-scrollbar-blow = Minimapa de la barra de desplazamiento: nota de soplo
editor-legend-scrollbar-draw = Minimapa de la barra de desplazamiento: nota de aspiración
editor-legend-scrollbar-note = Nota: aquí ese azul/naranja significa soplo/aspiración — un significado distinto al de los colores de las notas anteriores, que representan la técnica.

# Song Editor 2 — campos exclusivos de lección (mostrados mientras
# "Grabar Lección" está activo)
editor-lesson-details-header = Detalles de la Lección
editor-field-lesson-id = ID de Lección
editor-field-lesson-unit = Unidad
editor-field-lesson-explanation = Explicación
editor-field-lesson-prerequisites = Requisitos Previos
editor-field-lesson-pass-criteria = Criterio de Aprobación
editor-field-lesson-threshold = Umbral
editor-field-lesson-technique = Técnica
editor-field-lesson-progression = Progresión

# Song Editor 2 — títulos de diálogos de archivo
dialog-save-chart = Guardar partitura
dialog-load-chart = Cargar partitura
dialog-save-lesson = Guardar lección
dialog-load-lesson = Cargar lección
dialog-select-music = Seleccionar música de fondo
dialog-select-midi = Seleccionar archivo MIDI
dialog-file-name = Nombre de archivo:
dialog-cancel-esc = Cancelar  (Esc)

# Song Editor 2 — mensajes de validación al arrastrar
drag-denied-bend = Este orificio no admite esta profundidad de doblado
drag-denied-overblow = El oversoplo solo está disponible en los orificios 1–6
drag-denied-overdraw = La overaspiración solo está disponible en los orificios 7–10
drag-denied-overlap = Ya hay otra nota aquí

# Song Editor 2 — confirmación de la herramienta Borrar/Quitar de la línea de tiempo
editor-confirm-erase = ¿Borrar del compás %from% al %to%? Se eliminará cada nota de ese tramo — el resto de la canción se queda exactamente donde está.
editor-confirm-remove = ¿Quitar del compás %from% al %to%? Se eliminará cada nota de ese tramo, y todo lo siguiente se desplazará hacia atrás para cerrar el hueco.

# Song Editor 2 — mensajes del modo de práctica
practice-no-music = No hay música de fondo configurada — ¡toca junto con la partitura!
practice-prompt = ▶ Toca %note%…
practice-wrong-note = ▶ %got% → se necesita %expected%
practice-hit-perfect = ✓ PERFECTO  %note%  +%pts% pts
practice-hit-good = ✓ BIEN  %note%  +%pts% pts
practice-missed = ✗ Fallaste %note%
practice-done = Hecho — %hits%/%total% notas  ·  %score% pts
editor-record-status = ⏺ Grabando — %count% notas capturadas

# Song Editor 2 — descripciones de los botones
editor-back-tooltip = Salir del editor y volver al menú principal
editor-mode-edit-tooltip = Cambiar al modo Editar — coloca, mueve y edita notas en la cuadrícula
editor-mode-record-tooltip = Cambiar al modo Grabar — graba notas de tu armónica directo en la cuadrícula
editor-mode-play-tooltip = Cambiar al modo Reproducir — reproduce o practica la partitura
editor-lock-tooltip = Bloquear la cuadrícula para evitar ediciones accidentales al revisar
editor-save-tooltip = Guardar esta partitura en un archivo .harpchart
editor-load-tooltip = Cargar una partitura desde un archivo .harpchart
editor-play-tooltip = Iniciar o reanudar la reproducción de la partitura
editor-pause-tooltip = Pausar la reproducción en el mismo punto
editor-stop-tooltip = Detener la reproducción y volver el cursor al inicio
editor-practice-tooltip = Modo práctica — toca junto con tu armónica y recibe retroalimentación en vivo
editor-record-play-tooltip = Empieza a grabar desde la posición actual — o reanuda una grabación en pausa
editor-record-stop-tooltip = Termina la grabación — el cursor se queda donde paró
editor-finish-tooltip = Finaliza la grabación y vuelve al inicio — grabar de nuevo reemplaza las notas sobre las que toques
mod-blow-tooltip = Establecer la nota seleccionada como soplo (exhalar)
mod-draw-tooltip = Establecer la nota seleccionada como aspiración (inhalar)
mod-bend-tooltip = Alternar la profundidad de doblado de la nota seleccionada: ninguno → medio tono → tono completo → tono y medio
mod-overblow-tooltip = Establecer la nota seleccionada como oversoplo (técnica avanzada de soplo, solo diatónica)
mod-overdraw-tooltip = Establecer la nota seleccionada como overaspiración (técnica avanzada de aspiración, solo diatónica)
mod-slide-tooltip = Establecer la nota seleccionada para usar el botón slide (solo armónicas cromáticas)
mod-wah-tooltip = Alternar la velocidad de wah-wah de la nota seleccionada
mod-vibrato-tooltip = Alternar la velocidad de vibrato de la nota seleccionada
mod-delete-tooltip = Eliminar la nota seleccionada
editor-tool-select-tooltip = Click a point on the timeline then click a side (or click-drag a range)
editor-tool-erase-tooltip = Haz clic en un punto de la línea de tiempo y luego en un lado (o haz clic y arrastra un tramo) para borrar sus notas, dejando un hueco
editor-tool-remove-tooltip = Haz clic en un punto de la línea de tiempo y luego en un lado (o haz clic y arrastra un tramo) para borrar sus notas y desplazar todo lo siguiente hacia atrás, cerrando el hueco
editor-tool-tempo-tooltip = Haz clic en la regla para añadir un cambio de tempo ahí, o haz clic en uno existente para quitarlo
editor-harmonica-toggle-tooltip = Haz clic para alternar entre armónica Diatónica y Cromática
editor-content-kind-toggle-tooltip = Haz clic para alternar entre grabar una canción normal y una lección del currículo
editor-lesson-form-tooltip = Campos del currículo para lesson.json — solo se usan mientras "Grabar Lección" está activo
editor-lesson-details-toggle-tooltip = Haz clic para mostrar u ocultar los campos del currículo de la lección
editor-field-lesson-pass-criteria-tooltip = Haz clic para alternar cómo se evalúa esta lección — Ninguno, Precisión, Técnica, Adherencia a la Escala, Adherencia a Notas del Acorde, Disciplina de Frase
editor-field-lesson-technique-tooltip = Haz clic para alternar qué técnica se evalúa — solo se usa cuando el Criterio de Aprobación es Técnica
editor-field-lesson-progression-tooltip = Haz clic para alternar la progresión de acompañamiento de una lección basada en jam — Ninguna, Estándar, Quick-Change, Menor
editor-field-key-tooltip = Haz clic para recorrer los tonos de la armónica
editor-field-position-tooltip = Haz clic para recorrer las posiciones de interpretación
editor-browse-tooltip = Elegir un archivo de audio de música de fondo para esta partitura
editor-import-midi-tooltip = Cargar un archivo MIDI y elegir una pista para colocarla en la cuadrícula de notas — Guardar escribe entonces una pista de acompañamiento a partir de sus otras pistas
editor-silence-track-label = Silencio
editor-silence-track-tooltip = El intervalo, en segundos, entre cada par de notas consecutivas

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
lesson-goal-chord-tone-adherence = Objetivo: %pct%% de las notas como notas del acorde
lesson-goal-phrase-discipline = Objetivo: %pct%% de las notas tocadas fuera de una pausa — deja espacio
lesson-complete-banner = LECCIÓN SUPERADA
lesson-failed-banner = Objetivo no alcanzado — relee la lección e inténtalo de nuevo

# Lecciones — títulos de unidad (la clave sale del campo "unit" de cada lesson.json)
lesson-unit-blowing = Unidad 1 · Soplar la Armónica
lesson-unit-rhythm = Unidad 2 · Contar el Blues
lesson-unit-blues = Unidad 3 · Vocabulario del Blues

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

# Lección: respiración
lesson-breathing-title = Respiración y Notas Largas
lesson-breathing-body =
    Las notas largas y estables son la base sobre la que se construye todo lo demás — antes de los bends, el vibrato o los licks rápidos, tu aire necesita estar calmado y controlado.
    Respira desde el diafragma, no desde el pecho: deja que el vientre se expanda al inhalar, y mantén los hombros relajados y quietos.
    En este ejercicio, los agujeros 1 al 4 (soplado y aspirado) se sostienen de tres a cuatro tiempos cada uno — respira a través de la armónica, no empujes el aire.
    Una nota temblorosa o que se escapa no puntúa como limpia; una nota firme y estable sí, incluso a bajo volumen.
    Si te quedas sin aire a mitad de la nota, estás usando más del que necesitas — relájate y deja que la armónica haga menos esfuerzo por más sonido.

# Lección: primer bend
lesson-first-bend-title = Tu Primer Bend: 4 Aspirado
lesson-first-bend-body =
    El bend de medio tono en el 4 aspirado es el clásico primer bend que aprende todo armonicista — baja levemente la lengua y la mandíbula mientras aspiras, como si dijeras "iii" deslizando hacia "ooo".
    No tenses la garganta; el bend viene de la forma dentro de la boca, no de apretar más fuerte.
    En este ejercicio, el 4 aspirado normal alterna con el 4 aspirado con bend — escucha cómo la altura baja medio tono cada vez que haces el bend.
    Practica en el Entrenador de Bends (en el menú Jugar) si quieres oír la nota objetivo y comprobar tu afinación antes de volver aquí.
    Un bend impreciso todavía cuenta si se acerca lo suficiente — la precisión llega con la práctica, así que no busques la perfección el primer día.

# Lección: bends profundos
lesson-deep-bends-title = Bends Profundos: 2 y 3 Aspirado
lesson-deep-bends-body =
    Los agujeros 2 y 3 aspirados son donde realmente vive el blues de 2ª posición — ambos pueden bajar más que el agujero 4, medio tono y un tono entero.
    Cuanto más profundo el bend, más atrás necesitan ir la lengua y la mandíbula — piensa en la vocal deslizando de "i" a "o" a "u".
    En este ejercicio, el 2 aspirado hace medio tono y luego un tono entero, y el 3 aspirado hace lo mismo — escucha dos alturas distintas por debajo de la nota natural en cada agujero.
    Estas son las dos notas más expresivas de toda la armónica — las notas "azules" que le dan su voz al instrumento.
    Si el bend de tono entero no baja lo suficiente, no fuerces con presión — relaja aún más la garganta; la tensión ahoga el bend en vez de profundizarlo.

# Lección: vibrato
lesson-vibrato-title = Vibrato
lesson-vibrato-body =
    El vibrato añade una leve oscilación a una nota sostenida — un pequeño movimiento de altura o volumen que hace que una nota larga suene viva en vez de estática.
    La fuente clásica es el diafragma: un pulso suave "ja-ja-ja" en la respiración, el mismo músculo del ejercicio de respiración.
    En este ejercicio, sostén cada nota con firmeza y deja que un pulso lento (unas cuatro o cinco veces por segundo) la recorra — el juego escucha esa oscilación.
    Demasiado rápido suena como un temblor; demasiado lento suena como notas separadas. Busca una onda suave y uniforme.
    Si no registra nada, exagera el pulso más de lo que parece natural al principio — siempre puedes suavizarlo después de que el micrófono confirme que está ahí.

# Lección: articulación
lesson-articulation-title = Articulación: Tonguing Ta-Ka
lesson-articulation-body =
    El tonguing es cómo separas notas con claridad sin mover la respiración ni la embocadura — di "ta" o "ka" con la lengua en cada nota nueva, como pulsar un interruptor.
    "Ta-ka" alterna el frente y el fondo de la lengua, permitiéndote articular notas repetidas rápidas sin cansar el aire.
    En este ejercicio, el mismo agujero se repite en corcheas constantes — el juego no puede oír tu lengua directamente, pero una serie de notas ligadas, sin tonguing, solo puntúa el primer ataque. Rearticular cada una es lo que hace que el resto cuente.
    Empieza despacio y exagerado; la velocidad llega después, la claridad primero.
    Si tus notas se difuminan en un solo tono largo en la pista, no estás deteniendo el aire por completo entre ellas — una lengua más firme lo arregla.

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

# Lección: contando de cuatro
lesson-counting-four-title = Contando de Cuatro
lesson-counting-four-body =
    Toda habilidad rítmica de aquí en adelante se apoya en un solo hábito: contar el tiempo en voz alta, o al menos mentalmente, mientras tocas.
    Cuenta "1, 2, 3, 4" con firmeza junto al metrónomo antes de empezar, y sigue contando una vez que empiecen las notas — no dejes de contar solo porque estás tocando.
    En este ejercicio, una nota cae en cada tiempo, luego solo en los tiempos 1 y 3, luego solo en el tiempo 1 — los huecos se agrandan, pero tu cuenta interna nunca debe saltarse nada.
    Si pierdes el tiempo, no adivines — para, reinicia la cuenta desde 1, y vuelve a entrar en el siguiente primer tiempo.
    Este es el hábito más útil de todo este currículo: todo, desde la forma de 12 compases hasta el turnaround, depende de saber siempre exactamente dónde está el tiempo 1.

# Lección: contando los compases
lesson-bar-counting-title = Contando los Compases
lesson-bar-counting-body =
    Ahora cuenta compases en vez de tiempos: este ejercicio recorre la forma completa de 12 compases, una nota raíz en el tiempo 1 de cada compás, para que sientas llegar los cambios de acorde sin necesidad de mirar.
    Esto es 2ª posición: tu armónica en Do toca en la tonalidad de Sol, así que el 2 aspirado es la raíz del acorde I, el 4 soplado es la raíz del acorde IV, y el 4 aspirado es la raíz del acorde V.
    Observa la rejilla de 12 compases iluminarse conforme suena cada compás — combina lo que oyes y tocas con lo que ves, luego intenta contar con los ojos cerrados.
    El patrón es cuatro compases de I, dos de IV, dos de I, uno de V, uno de IV, uno de I y uno de V — la misma forma que leíste en "Leer la Rejilla del Blues de 12 Compases".
    Si caes en la raíz equivocada, probablemente perdiste la cuenta en algún punto intermedio — la solución es siempre la misma: para, recuenta desde el compás 1 en el siguiente chorus.

# Lección: el turnaround
lesson-turnaround-title = El Turnaround
lesson-turnaround-body =
    El turnaround son los últimos dos compases de la forma de 12 compases — el momento en que la música se inclina de vuelta hacia el inicio del siguiente chorus, y la parte que todo armonicista de blues necesita sentir llegar.
    Este ejercicio descansa casi toda la forma a propósito: no hay nada que tocar hasta el compás 12, así que la única forma de acertarlo es seguir contando en silencio todo el camino.
    Cuando llegue el compás 12, toca la raíz del acorde V; luego, justo al inicio del siguiente chorus, toca la raíz del acorde I — eso es el turnaround resolviendo a casa.
    Si tocas en el silencio antes del compás 12, perdiste la cuenta en algún punto anterior — ahí no hay nota que perseguir, solo el tiempo que mantener.
    Esta es la misma llegada que necesitarás oír en jams de verdad: el turnaround suele ser el único momento en que toda una banda se realinea junta.

# Lección: sensación de shuffle
lesson-shuffle-feel-title = Sensación de Shuffle
lesson-shuffle-feel-body =
    La mayor parte del blues no se apoya en corcheas rectas y uniformes — se balancea, con un rebote "shuffle" largo-corto en su lugar.
    Di "tam-TAM, tam-TAM" para sentir la proporción: la primera mitad de cada par dura casi el doble que la segunda.
    Esta partitura declara sensación de shuffle, así que el clic del metrónomo se balancea junto con las notas — escucha el clic, no solo las notas, para engancharte al balanceo.
    En este ejercicio, pares largo-corto alternan soplado y aspirado en el agujero 4 — encaja la nota larga justo en el tiempo y deja que la nota corta rebote desde ahí.
    Si tus pares salen uniformes en vez de balanceados, probablemente sigues contando corcheas rectas en tu cabeza — intenta contar el shuffle como un tresillo, sosteniendo los dos primeros tiempos juntos.

# Lección: chug del tren
lesson-train-chug-title = Tren: El Chug
lesson-train-chug-body =
    El chug es el clásico sonido de tren de la armónica — y en secreto un ejercicio de ritmo y control de la respiración disfrazado.
    Alterna un acorde soplado y un acorde aspirado en los agujeros 1-2-3, firme y uniforme, como una locomotora lenta ganando vapor.
    Respira el ritmo en vez de usar la lengua: deja que la propia respiración haga "juf... paf... juf... paf", no una lengua golpeando para encender y apagar.
    En este ejercicio, el acorde alterna en corcheas constantes a un tempo lento y paciente — todas las notas de cada acorde necesitan sonar juntas para que cuente.
    Si solo registra parte del acorde, abre la embocadura de forma uniforme en los tres agujeros en vez de apretar más de un lado.

# Lección: tren rodando
lesson-train-rolling-title = Tren: Rodando
lesson-train-rolling-body =
    Ahora el tren sale de la estación: el mismo chug que acabas de aprender, pero acelerando gradualmente mientras coge velocidad.
    No persigas la velocidad — deja que crezca naturalmente, igual que un tren de verdad no salta directamente a máxima velocidad.
    Esta partitura es la primera del currículo construida sobre un mapa de tempo en vez de un tempo fijo — las notas están posicionadas por tick, y la base realmente acelera bajo tus pies.
    Sigue respirando el patrón juf-paf de la lección anterior; solo cambia el tempo, no la forma de tu respiración.
    Si te quedas atrás mientras acelera, es normal en tus primeros intentos — el objetivo es mantenerte suelto, no rígido, mientras el tempo cambia.

# Lección: silbato del tren
lesson-train-whistle-title = Tren: El Silbato
lesson-train-whistle-body =
    Todo chug de tren necesita un silbato — un acorde largo y lastimero de dos notas que corta a través del ritmo del chug.
    El silbato va en los agujeros 4 y 5 aspirados juntos, sostenido largamente, con un wah trabajado en él — la misma técnica de manos en copa de la lección del wah.
    En este ejercicio, los coros de chug alternan con un acorde de silbato sostenido — mantén el chug firme, luego abre hacia el silbato y deja que tu mano haga el "wah" mientras sostienes la nota.
    El silbato necesita tanto el acorde (dos notas sonando juntas) como el pulso de wah a la vez — si uno falla, comprueba que estás sosteniendo ambos agujeros por igual mientras tu mano sigue moviéndose.
    Esto combina todo lo de los ejercicios de chug con la técnica de hand-wah — una buena señal de que estás listo para llevar ambos a una jam de verdad.

# Lección: escala de blues
lesson-blues-scale-title = La Escala de Blues
lesson-blues-scale-body =
    Siete notas, subiendo y bajando: 2 aspirado, 3 aspirado con bend, 4 soplado, 4 aspirado con bend, 4 aspirado, 5 aspirado, 6 soplado.
    Esta es la misma escala de blues en 2ª posición de la que sale cada frase de esta unidad — y la mayor parte de la armónica blues.
    Ya tienes ambos bends del ejercicio de bends profundos; esta lección trata de encadenarlos en una sola forma que toques sin pensar.
    Tócala despacio al principio, escuchando cómo las notas con bend encajan entre las naturales, en lugar de reemplazarlas.
    En cuanto esta escala te resulte familiar bajo los dedos, todo lo demás en esta unidad es solo este mismo puñado de notas reordenado.

# Lección: primeras frases
lesson-first-licks-title = Primeras Frases
lesson-first-licks-body =
    Tres frases cortas, tres notas cada una, todas sacadas de la escala de blues que acabas de aprender — sin bends todavía.
    Cada una suena como demostración, luego espera a que la repitas — exactamente igual que el ejercicio de pregunta y respuesta.
    No son solo ejercicios — son frases reales de blues, del tipo que buscarás por instinto en cuanto las tengas bajo los dedos.
    Tómate el tiempo que necesites en cada eco; el juego te espera, así que no hay prisa por llegar.
    En cuanto puedas tocar las tres de memoria, prueba a mezclarlas en una jam session y ve cómo se sienten sobre los cambios.

# Lección: frases con bend
lesson-bent-licks-title = Frases con Bend
lesson-bent-licks-body =
    Ahora las frases encuentran su voz: tres frases construidas alrededor de los bends de 3 y 4 aspirado, las notas "lastimeras" de la escala de blues.
    Cada una suena como demostración, luego espera a que la repitas — el mismo patrón de pregunta y respuesta de la lección anterior, pero cada frase se apoya en un bend.
    Escucha la diferencia entre un bend limpio y uno tembloroso; un bend firme y sostenido es lo que da carácter a estas frases.
    Si una frase se te resiste, vuelve al ejercicio de bends profundos unos minutos y regresa — normalmente el bend en sí, no la frase, es el punto difícil.
    Estas son las mismas notas lastimeras que oirás en casi todo solo de armónica blues — acostúmbrate a ellas aquí y aparecerán en todas partes.

# Lección: frases sobre los cambios
lesson-licks-over-changes-title = Frases Sobre los Cambios
lesson-licks-over-changes-body =
    Un coro completo de 12 compases, pero en lugar de solo raíces o una escala corrida, cada acorde recibe su propia frase corta: una forma sobre el acorde I, otra sobre el IV, otra sobre el V, y el giro final para cerrar.
    Esto combina el ejercicio de contar compases con tus nuevas frases — necesitas saber dónde estás en la forma y tener la frase correcta lista para ella.
    La superposición de frases marca cada línea de 4 compases para que veas la forma de la estructura mientras tocas.
    Si pierdes el hilo, vuelve a la escala de blues que ya conoces en lugar de bloquearte — acertar algo sobre el acorde correcto es mejor que no tocar nada.
    Toca esto varias veces hasta que las frases empiecen a sentirse parte de los acordes sobre los que suenan, no solo notas que recitas en orden.

# Lección: improvisación sobre notas del acorde
lesson-chord-tone-improv-title = Improvisación Sobre Notas del Acorde
lesson-chord-tone-improv-body =
    El ejercicio de improvisación te juzgaba por quedarte en la escala de blues. Este sube el listón: cae específicamente en una nota del acorde a medida que cada acorde cambia, no solo en cualquier lugar seguro de la escala.
    Abre el mismo tipo de Jam Session abierta — el mapa de agujeros recolorea en dorado para una nota del acorde, verde para dentro de la escala, ámbar para fuera de ella — pero esta vez el dorado es el objetivo, no solo una sorpresa agradable.
    Intenta anticipar el cambio un tiempo antes: sabe que el acorde IV está por llegar y ten tu nota objetivo lista antes de que llegue, en lugar de reaccionar después.
    Sigue sin haber melodía fija — toca lo que quieras, solo haz que más notas caigan en dorado que en el ejercicio anterior.
    Cuando sientas que estás listo para parar, abre el menú de pausa y pulsa "Finalizar Lección" para que el juego sume tu fracción de notas del acorde.

# Lección: blues menor
lesson-minor-blues-improv-title = Blues Menor
lesson-minor-blues-improv-body =
    Misma jam abierta, misma armónica en Do, pero la progresión base cambia a un blues menor — la 3ª bemol es ahora la nota de reposo, no solo un color de paso.
    Esto cambia lo que significan "estar en la escala" y "caer en una nota del acorde" bajo tus dedos, aunque no hayas cambiado de armónica ni de posición.
    Déjate llevar por el sonido más oscuro y melancólico que trae la progresión menor — es un ambiente distinto del blues mayor que has estado tocando, no un error que corregir.
    El mapa de agujeros sigue recoloreando en vivo exactamente igual que en las demás lecciones de jam; confía en el color, no en lo que esperarías de un blues mayor.
    Cuando sientas que estás listo para parar, abre el menú de pausa y pulsa "Finalizar Lección" — se juzga igual que el ejercicio de improvisación original, adherencia a la escala contra la escala de blues menor.

# Lección: pregunta y respuesta
lesson-question-answer-title = Pregunta y Respuesta
lesson-question-answer-body =
    Esta lección no trata de lo que tocas — trata de lo que no tocas. Toca durante dos compases, luego para de verdad durante dos compases, alternando por toda la forma.
    Dejar un silencio de verdad es la clave: una frase que recibe respuesta necesita espacio para la respuesta, y ese espacio solo existe si dejas de preguntar.
    Es tentador seguir toqueteando durante el descanso — resístete. El mapa de agujeros y tus propios oídos saben la diferencia entre un descanso y una nota sostenida.
    Esta es la misma Jam Session abierta de las demás lecciones de improvisación; toca las frases o escalas que te parezcan bien en tus dos compases, luego suelta la armónica de verdad.
    Cuando sientas que estás listo para parar, abre el menú de pausa y pulsa "Finalizar Lección" — el juego juzga cuánto de lo que tocaste cayó fuera de esas ventanas de descanso.

# Juego — cuenta atrás, leyenda, pistas del diagrama de armónica
gameplay-get-ready = PREPÁRATE
gameplay-legend-blow = ■ SOPLO
gameplay-legend-draw = ■ ASPIRACIÓN
harmonica-overlay-hint-view = Armónica  ·  se ilumina mientras tocas
harmonica-overlay-hint-select = Armónica  ·  haz clic en una nota para seleccionarla
gameplay-chart-info = Tono: %key%  ♩ = %bpm%  %time_sig%
gameplay-chart-author = Canción: %author%
gameplay-techniques-toggle = %arrow% TÉCNICAS

# Menú de pausa
pause-quit-song = Salir de la canción
pause-finish-lesson = Terminar lección
pause-wait-for-note-button = ⏸ Esperar nota
pause-wait-for-note-on = Esperar nota: activado
pause-wait-for-note-off = Esperar nota: desactivado
pause-speed = Velocidad: %pct%%
pause-adaptive-difficulty-button = Dificultad adaptativa
pause-adaptive-difficulty-on = Dificultad adaptativa: activada
pause-adaptive-difficulty-off = Dificultad adaptativa: desactivada
pause-phrase-section = Sección: %name% — Aprendido: %pct%%
pause-phrase-no-sections = No hay frases en esta canción
pause-drag-section-hint = Haz clic en una sección de la barra de progreso de arriba para seleccionarla
pause-notes-update-hint = Las notas se actualizan en vivo — reanuda para verlas
pause-clear-loop = Borrar bucle
pause-loop-off = Bucle: desactivado
pause-loop-range = Bucle: %start%s–%end%s
pause-drag-loop-hint = Arrastra en la barra de progreso de arriba para definir un rango de bucle

# Overlay del metrónomo
metronome-click-off = clic: apagado
metronome-click-on = clic: encendido
metronome-feel-straight = ritmo: recto
metronome-feel-shuffle = ritmo: shuffle

# Entrenador de Bends
bending-drill-off = Ejercicio: apagado
bending-drill-on = Ejercicio: encendido · racha %streak%
bending-hint = Esc para volver  ·  M silencia el clic  ·  feel alterna recto/shuffle
bending-no-note-for-technique = Este agujero no tiene nota para esa técnica.
bending-key-label = Tono: %key%
bending-listen-button = 🔊 Escuchar
bending-drill-button = 🎲 Ejercicio
bending-play-it-target = Tócala — objetivo %note%
bending-in-tune = ✓ Afinado  (%note%)
bending-cents-sharp = ↑ %cents% cents agudo  (objetivo %note%)
bending-cents-flat = ↓ %cents% cents grave  (objetivo %note%)
bending-detect-label = Detectar

# Jam Session
jam-loop-button = ↻ Bucle
jam-loop-off = Bucle: apagado
jam-loop-on = Bucle: encendido
jam-hole-map-hint = Tu armónica  ·  dorado = tono del acorde ahora mismo  ·  verde = nota de la escala de blues  ·  soplo arriba / aspiración abajo
jam-call-response-button = 🗣 Pregunta y Respuesta
jam-call-response-off = Pregunta y Respuesta: apagado
jam-call-response-on = Pregunta y Respuesta: encendido
jam-call-response-listen = Escucha…
jam-call-response-your-turn = Tu turno

# Pantalla de resultados
results-song-complete = CANCIÓN COMPLETADA
results-by-technique = Por técnica
results-new-best = ★ ¡NUEVO RÉCORD! ★
results-biggest-combo = Combo más alto
results-perfect-hits = Aciertos perfectos
results-good-hits = Buenos aciertos
results-hits = Aciertos
results-delayed-hits = Aciertos tardíos
results-misses = Fallos
results-technique-normal = Notas normales
results-technique-bend = Bends
results-technique-vibrato = Vibrato
results-technique-wah = Wah
results-technique-overblow = Overblow
results-technique-overdraw = Overdraw
results-technique-slide = Slide
results-technique-clean-attack = Ataque limpio
results-avg-timing-offset = Desfase medio de tiempo
results-increase-latency = Aumentar el retraso de entrada a %ms%ms
results-decrease-latency = Reducir el retraso de entrada a %ms%ms
results-score = Puntuación: %points%
results-best-score = Mejor puntuación

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
tutorial-body-main = Tu base — ve a Jugar, abre Opciones o encuentra Ayuda / Acerca de desde aquí.
tutorial-title-play = Jugar
tutorial-body-play = Elige una canción real, crea una, empieza una jam, practica bends o sigue las lecciones — elige cómo quieres jugar.
tutorial-title-mode-select = Seleccionar Modo
tutorial-body-mode-select = Elige 2D (un camino de notas que se desliza) o 3D (una armónica que tocas junto a ti).
tutorial-title-gameplay = Tocando una Canción
tutorial-body-gameplay = Las notas caen hacia la línea de acierto — toca la nota correcta en tu armónica en el momento justo para anotar.
tutorial-title-jam-session-menu = Jam Session
tutorial-body-jam-session-menu = Elige una canción real para improvisar, o genera una base instantánea.
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
tutorial-title-help-about = Ayuda / Acerca de
tutorial-body-help-about = Abre la documentación, lee sobre Harmonicon, repite este recorrido o consulta los créditos.
