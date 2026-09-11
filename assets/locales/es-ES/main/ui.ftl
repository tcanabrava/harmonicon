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
jam-generate-key = Tono
jam-generate-tempo = Tempo
jam-generate-progression = Progresión
jam-generate-position = Posición
jam-generate-scale = Escala
jam-generate-genre = Género

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
options-adaptive-difficulty-tooltip = Ajusta automáticamente cuántas notas de la canción se muestran a la vez, según tu desempeño.
options-fullscreen = Pantalla completa
options-fullscreen-tooltip = Juega en pantalla completa en vez de en una ventana.
options-colorblind-palette = Paleta para daltonismo
options-colorblind-palette-tooltip = Usa un par fijo de colores soplar/aspirar seguro para daltonismo, en vez de los colores de nota del tema actual.
options-zoom = Zoom
options-zoom-tooltip = Ajusta el tamaño de toda la interfaz.
options-zoom-label = Zoom: {$percent}%
options-pitch-detect = Detección de tono
options-microphone = Micrófono
options-microphone-tooltip = De qué dispositivo de entrada capturar tu armónica.
options-mic-retry-tooltip = Intenta reconectar con el micrófono.
options-note-labels = Etiquetas de notas
options-note-labels-tooltip = Muestra las notas que caen como números de agujero, en vez de flechas de soplar/aspirar.
options-harmonica-tooltip = Qué modelo de armónica aparece en el juego en 3D.
options-music-volume-tooltip = Volumen de la pista de acompañamiento.
options-metronome-volume-tooltip = Volumen del clic del metrónomo.
options-theme-tooltip = Cambia el tema visual de los menús.
options-calibrate-input-lag = Calibrar la latencia de entrada
options-calibrate-input-lag-tooltip = Mide la latencia de audio de tu equipo y la aplica automáticamente.
options-back-tooltip = Vuelve al menú principal.
options-button-style = Botones de acción
options-button-style-tooltip = Cómo muestran icono y texto los botones de acción del Editor de Canciones.
options-button-style-icon-only = Solo icono
options-button-style-text-beside-icon = Texto junto al icono
options-button-style-text-only = Solo texto
theme-back-to-options = ← Volver a Opciones

# Compartido
back = ← Volver

# Song Editor 2 — botones de transporte y panel de modificadores
editor-back-label = Volver
editor-mode-edit = Editar
editor-mode-record = Grabar
editor-mode-play = Reproducir
editor-mode-expected = Marcar notas correctas
editor-lock = Bloquear
editor-undo = Deshacer
editor-redo = Rehacer
editor-delete = Eliminar
editor-copy = Copiar
editor-paste = Pegar
editor-metronome = Metrónomo
editor-play = Reproducir
editor-pause = Pausar
editor-stop = Detener
editor-practice = Practicar
editor-finish = Finalizar
editor-save = Guardar
editor-load = Cargar
editor-browse = 📂 Examinar
editor-import-midi = ♬ Importar MIDI
mod-blow = Soplar
mod-draw = Aspirar
mod-bend = Doblar
mod-overblow = Oversoplo
mod-overdraw = Overaspiración
mod-slide = Slide
mod-wah = Wah
mod-vibrato = Vibrato
mod-delete = Eliminar
editor-tool-select = Seleccionar
editor-tool-erase = Borrar Tramo
editor-tool-remove = Quitar Tramo
editor-tool-tempo = Tempo

# Song Editor 2 — etiquetas de los campos de metadatos
editor-field-tempo = Tempo de la Música
editor-field-time-signature = Compás
editor-field-key = Tono de la Armónica
editor-field-position = Posición
editor-field-harmonica = Armónica
editor-field-music = Música de Fondo
editor-field-name = Nombre
editor-field-author = Autor
editor-field-midi-track = Pista MIDI
editor-field-midi-track-tooltip = Qué pista del archivo MIDI importado colocar en la cuadrícula.
editor-field-scale = Escala
editor-field-scale-tooltip = Contra qué escala se mide el tinte rojo de "fuera de la escala" en la cuadrícula.
editor-field-text-tooltip = Haz clic para editar; escribe un valor y luego haz clic fuera o pulsa Intro para confirmar.
editor-harmonica-diatonic = ‹ Diatónica (10 orificios) ›
editor-harmonica-chromatic = ‹ Cromática (12 orificios) ›
editor-field-content-kind = Grabación
editor-content-kind-song = ‹ Grabar Canción ›
editor-content-kind-lesson = ‹ Grabar Lección ›
editor-field-snap-mode = Ajuste de Cuadrícula
editor-snap-mode-sixteenth = ‹ Semicorcheas rectas ›
editor-snap-mode-shuffle = ‹ Shuffle (corcheas con swing) ›
editor-snap-mode-triplet = ‹ Tresillos de corchea ›

# Song Editor 2 — leyenda de colores (tercera columna del formulario)
editor-legend-toggle = Leyenda
editor-legend-toggle-tooltip = Muestra u oculta la columna de leyenda de colores.
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
editor-legend-triplet-line = Línea de subdivisión de tresillo (pulsos 4/8 del tiempo)
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
editor-field-lesson-scale = Escala de la lección
editor-field-lesson-path = Ruta del curso

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
editor-confirm-erase = ¿Borrar del compás {$from} al {$to}? Se eliminará cada nota de ese tramo — el resto de la canción se queda exactamente donde está.
editor-confirm-remove = ¿Quitar del compás {$from} al {$to}? Se eliminará cada nota de ese tramo, y todo lo siguiente se desplazará hacia atrás para cerrar el hueco.

# Song Editor 2 — mensajes del modo de práctica
practice-no-music = No hay música de fondo configurada — ¡toca junto con la partitura!
practice-prompt = ▶ Toca {$note}…
practice-wrong-note = ▶ {$got} → se necesita {$expected}
practice-hit-perfect = ✓ PERFECTO  {$note}  +{$pts} pts
practice-hit-good = ✓ BIEN  {$note}  +{$pts} pts
practice-missed = ✗ Fallaste {$note}
practice-done = Hecho — {$hits}/{$total} notas  ·  {$score} pts
editor-record-status = ⏺ Grabando — {$count} notas capturadas
editor-count-in-status = ◔ Prepárate — grabación en {$seconds}s
editor-metronome-tooltip = Alterna el clic del metrónomo durante Grabar/Reproducir/Practicar
editor-save-success = ✓ Guardado: {$path}
editor-save-warning = ‼ Guardado con avisos: {$detail}
editor-save-failed = ✗ Error al guardar: {$detail}
editor-load-success = ✓ Cargado: {$path}
editor-load-failed = ✗ Error al cargar: {$detail}

# Song Editor 2 — descripciones de los botones
editor-back-tooltip = Salir del editor y volver al menú principal
editor-mode-edit-tooltip = Cambiar al modo Editar — coloca, mueve y edita notas en la cuadrícula
editor-mode-record-tooltip = Cambiar al modo Grabar — graba notas de tu armónica directo en la cuadrícula
editor-mode-play-tooltip = Cambiar al modo Reproducir — reproduce o practica la partitura
editor-mode-expected-tooltip = Solo en builds de desarrollo: marca las notas correctas encima de una grabación, para el benchmark de detección de notas (note_bench)
editor-lock-tooltip = Bloquear la cuadrícula para evitar ediciones accidentales al revisar
editor-undo-tooltip = Deshacer la última edición (colocar/mover/eliminar nota, pegar, Borrar/Quitar, una toma de grabación entera, ...)
editor-redo-tooltip = Rehacer la última edición deshecha
editor-delete-tooltip = Eliminar la(s) nota(s) seleccionada(s)
editor-copy-tooltip = Copiar la(s) nota(s) seleccionada(s)
editor-paste-tooltip = Pegar las últimas notas copiadas al inicio de la vista actual
editor-save-tooltip = Guardar esta partitura en un archivo .harpchart
editor-load-tooltip = Cargar una partitura desde un archivo .harpchart
editor-play-tooltip = Iniciar o reanudar la reproducción de la partitura
editor-pause-tooltip = Pausar la reproducción en el mismo punto
editor-stop-tooltip = Detener la reproducción y volver el cursor al inicio
editor-practice-tooltip = Modo práctica — toca junto con tu armónica y recibe retroalimentación en vivo
editor-record-play-tooltip = Empieza a grabar desde la posición actual — o reanuda una grabación en pausa
editor-record-stop-tooltip = Termina la grabación — el cursor se queda donde paró
editor-finish-tooltip = Finaliza la grabación y vuelve al inicio — grabar de nuevo reemplaza las notas sobre las que toques
editor-record-detect-label = Detectar
editor-debug-recording-button = Grabación de Depuración
editor-debug-recording-tooltip = Solo en builds de desarrollo: también graba el audio bruto del micrófono en assets/debug_songs/<song>/ al guardar, para diagnosticar problemas de detección de tono más tarde
editor-debug-recording-erase = Borrar Grabación
editor-debug-recording-erase-tooltip = Descarta el audio bruto capturado para que la próxima grabación empiece de cero
editor-debug-recording-off = Apagado
editor-debug-recording-armed = Listo — pulsa Play para grabar
editor-debug-recording-status = Grabando — {$secs}s capturados
mod-blow-tooltip = Establecer la nota seleccionada como soplo (exhalar)
mod-draw-tooltip = Establecer la nota seleccionada como aspiración (inhalar)
mod-bend-tooltip = Alternar la profundidad de doblado de la nota seleccionada: ninguno → medio tono → tono completo → tono y medio
mod-overblow-tooltip = Establecer la nota seleccionada como oversoplo (técnica avanzada de soplo, solo diatónica)
mod-overdraw-tooltip = Establecer la nota seleccionada como overaspiración (técnica avanzada de aspiración, solo diatónica)
mod-slide-tooltip = Establecer la nota seleccionada para usar el botón slide (solo armónicas cromáticas)
mod-wah-tooltip = Alternar la velocidad de wah-wah de la nota seleccionada
mod-vibrato-tooltip = Alternar la velocidad de vibrato de la nota seleccionada
mod-delete-tooltip = Eliminar la nota seleccionada
editor-tool-select-tooltip = Haz clic en un punto de la línea de tiempo y luego en un lado (o haz clic y arrastra para seleccionar un rango)
editor-tool-erase-tooltip = Haz clic en un punto de la línea de tiempo y luego en un lado (o haz clic y arrastra un tramo) para borrar sus notas, dejando un hueco
editor-tool-remove-tooltip = Haz clic en un punto de la línea de tiempo y luego en un lado (o haz clic y arrastra un tramo) para borrar sus notas y desplazar todo lo siguiente hacia atrás, cerrando el hueco
editor-tool-tempo-tooltip = Haz clic en la regla para añadir un cambio de tempo ahí, o haz clic en uno existente para quitarlo
editor-harmonica-toggle-tooltip = Haz clic para alternar entre armónica Diatónica y Cromática
editor-content-kind-toggle-tooltip = Haz clic para alternar entre grabar una canción normal y una lección del currículo
editor-snap-mode-toggle-tooltip = Haz clic para alternar la subdivisión del pulso a la que se ajusta un clic en la cuadrícula — semicorcheas rectas, corcheas shuffle (con swing) o tresillos de corchea rectos
editor-lesson-form-tooltip = Campos del currículo para lesson.json — solo se usan mientras "Grabar Lección" está activo
editor-lesson-details-toggle-tooltip = Haz clic para mostrar u ocultar los campos del currículo de la lección
editor-field-lesson-pass-criteria-tooltip = Haz clic para alternar cómo se evalúa esta lección — Ninguno, Precisión, Técnica, Adherencia a la Escala, Adherencia a Notas del Acorde, Disciplina de Frase
editor-field-lesson-technique-tooltip = Haz clic para alternar qué técnica se evalúa — solo se usa cuando el Criterio de Aprobación es Técnica
editor-field-lesson-progression-tooltip = Haz clic para alternar la progresión de acompañamiento de una lección basada en jam — Ninguna, Estándar, Quick-Change, Menor
editor-field-lesson-scale-tooltip = Haz clic para alternar la escala usada para evaluar una lección de jam
editor-field-lesson-path-tooltip = Haz clic para elegir si esta lección es obligatoria o una rama optativa
editor-field-key-tooltip = Haz clic para recorrer los tonos de la armónica
editor-field-position-tooltip = Haz clic para recorrer las posiciones de interpretación
editor-browse-tooltip = Elegir un archivo de audio de música de fondo para esta partitura
editor-import-midi-tooltip = Cargar un archivo MIDI y elegir una pista para colocarla en la cuadrícula de notas — Guardar escribe entonces una pista de acompañamiento a partir de sus otras pistas
editor-silence-track-label = Silencio
editor-silence-track-tooltip = El intervalo, en segundos, entre cada par de notas consecutivas

# Lecciones — menú, lector, veredicto en resultados
menu-lessons = Lecciones
no-lessons-found = No se encontraron lecciones. Añade carpetas en assets/lessons/<unidad>/<lección>/
lesson-passed = Superada
lesson-start = Empezar la Lección
lesson-widget-metronome-toggle = Iniciar / Detener
lesson-widget-tempo-decrease = − 5 BPM
lesson-widget-tempo-increase = + 5 BPM
lesson-widget-feel-toggle = Recto / Shuffle
lesson-widget-sound-toggle = Sonido activado / desactivado
lesson-widget-key-previous = Tono anterior
lesson-widget-key-next = Tono siguiente
lesson-widget-bar-previous = Compás anterior
lesson-widget-bar-next = Compás siguiente
lesson-widget-bar-reset = Volver al compás 1
lesson-mark-done = Marcar como Hecha
lesson-goal-accuracy = Objetivo: {$pct}% de precisión general
lesson-goal-technique = Objetivo: {$pct}% de precisión en las notas de {$technique}
lesson-goal-finish = Objetivo: tocarla hasta el final
lesson-goal-scale-adherence = Objetivo: {$pct}% de las notas dentro de la escala o mejor
lesson-goal-chord-tone-adherence = Objetivo: {$pct}% de las notas como notas del acorde
lesson-goal-phrase-discipline = Objetivo: {$pct}% de las notas tocadas fuera de una pausa — deja espacio
lesson-complete-banner = LECCIÓN SUPERADA
lesson-failed-banner = Objetivo no alcanzado — relee la lección e inténtalo de nuevo

# Lecciones — títulos de unidad (la clave sale del campo "unit" de cada lesson.json)
lesson-unit-blowing = Unidad 1 · Soplar la Armónica
lesson-unit-rhythm = Unidad 2 · Contar el Blues
lesson-unit-blues = Unidad 3 · Vocabulario del Blues
lesson-unit-scales = Unidad 4 · Escalas e Improvisación
lesson-unit-jazz = Unidad 5 · Jazz
lesson-unit-navigation = Unidad 6 · Encontrar el Camino

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

# Lección: improvisación con cambio rápido
lesson-quick-change-improv-title = Improvisación con Cambio Rápido
lesson-quick-change-improv-body =
    Misma jam abierta y la misma escala de blues que la lección de improvisación original, pero la progresión base cambia al "quick change" — el compás 2 pasa al acorde IV en vez de quedarse en el I.
    Esto significa que el terreno armónico cambia bajo tus pies dos compases antes que en la forma estándar de 12 compases que ya conoces.
    Nada cambia sobre qué notas "están en la escala" — esta lección se juzga igual que la lección de improvisación original — pero prestar atención a ese cambio de acorde más temprano afinará tu sentido de dónde estás en la forma.
    Si te pierdes, la cuadrícula de 12 compases y el mapa de agujeros siguen en vivo y reflejan la forma quick change en tiempo real.
    Cuando sientas que estás listo para parar, abre el menú de pausa y pulsa "Finalizar Lección" — se juzga igual que el ejercicio de improvisación original, adherencia a la escala contra la escala de blues.

# Lección: pregunta y respuesta
lesson-question-answer-title = Pregunta y Respuesta
lesson-question-answer-body =
    Esta lección no trata de lo que tocas — trata de lo que no tocas. Toca durante dos compases, luego para de verdad durante dos compases, alternando por toda la forma.
    Dejar un silencio de verdad es la clave: una frase que recibe respuesta necesita espacio para la respuesta, y ese espacio solo existe si dejas de preguntar.
    Es tentador seguir toqueteando durante el descanso — resístete. El mapa de agujeros y tus propios oídos saben la diferencia entre un descanso y una nota sostenida.
    Esta es la misma Jam Session abierta de las demás lecciones de improvisación; toca las frases o escalas que te parezcan bien en tus dos compases, luego suelta la armónica de verdad.
    Cuando sientas que estás listo para parar, abre el menú de pausa y pulsa "Finalizar Lección" — el juego juzga cuánto de lo que tocaste cayó fuera de esas ventanas de descanso.

# Lección: escala mayor
lesson-major-scale-title = La Escala Mayor
lesson-major-scale-body =
    Ocho notas, subiendo y bajando: 1 soplado, 1 aspirado, 2 soplado, 2 aspirado doblado un tono entero, 3 soplado, 3 aspirado doblado un tono entero, 3 aspirado, 4 soplado.
    La armónica de blues vive en la escala de blues, pero la escala mayor común es la base de la que hasta ese sonido de blues se aleja al doblar las notas.
    Dos de estas notas necesitan los doblados de un tono entero de la lección de doblados profundos, cayendo en Fa y La en vez de las notas naturales que esos agujeros dan sin doblar nada.
    Tócala despacio, prestando atención a dónde exactamente debe caer la nota — un doblado poco profundo se queda corto y todavía suena como la nota natural debajo de él.
    Cuando puedas oír la diferencia entre una nota natural y su vecina doblada, estás listo para usar esta escala en melodías de verdad, no solo en este ejercicio.

# Lección: escala pentatónica menor
lesson-minor-pentatonic-scale-title = Escala Pentatónica Menor
lesson-minor-pentatonic-scale-body =
    Seis notas, subiendo y bajando: 2 aspirado, 3 aspirado doblado, 4 soplado, 4 aspirado, 5 aspirado, 6 soplado — la escala de blues que ya conoces, sin la nota azul (la 5ª bemol).
    Esta es la escala pentatónica menor, una de las escalas más usadas en el blues, el rock y la música folk — y en esta armónica, es solo la escala de blues sin una nota.
    Como deja fuera la nota que exige el oído más fino (la 5ª bemol queda en una posición incómoda entre las otras dos), suele ser más fácil tocarla limpia en una frase rápida que la escala de blues completa.
    Fíjate en cómo suena completa y con sabor a blues por sí sola — la 5ª bemol es un condimento, no una obligación.
    Cuando esta forma te resulte tan automática como la escala de blues completa, prueba a alternar entre las dos a mitad de frase — es exactamente la elección que hacen los improvisadores de verdad sobre la marcha.

# Lección: escala country
lesson-country-scale-title = La Escala Country
lesson-country-scale-body =
    Nueve notas, subiendo y bajando, usando solo notas naturales de soplado y aspirado — sin doblar nada: 1 soplado, 1 aspirado, 2 soplado, 2 aspirado, 4 soplado, 4 aspirado, 5 soplado, 6 soplado, 6 aspirado.
    Esta es la escala pentatónica mayor, apodada "escala Country" en la pedagogía de la armónica porque es el sonido del country, el folk y la música old-time — brillante y abierta, a diferencia del blues.
    Fíjate en el salto entre 2 aspirado y 4 soplado: esta armónica simplemente no tiene una nota natural en medio en esa octava, y por eso la escala salta ahí.
    Como nada aquí necesita doblado, es una excelente escala para usar pronto en tu aprendizaje, o siempre que quieras un sonido alegre y nada bluesy en vez del color más oscuro de la escala de blues.
    Compara lo distinto que suena esto de las frases de escala de blues del resto de esta unidad — mismo instrumento, misma tonalidad, un ambiente completamente diferente.

# Lección: improvisación en la escala mayor
lesson-major-scale-improv-title = Improvisación en la Escala Mayor
lesson-major-scale-improv-body =
    Otra Jam Session abierta, pero esta vez el mapa de agujeros y el juicio de "Finalizar Lección" están calibrados con la escala mayor común que acabas de aprender, no con la escala de blues.
    Todo lo que toques se mide contra Do mayor (las notas de la lección de la escala mayor) en vez de la hexatónica de blues que usan las demás lecciones de jam de este juego.
    Se sentirá distinto bajo tus dedos: sin notas azules en las que apoyarte, sin 3ª ni 5ª dobladas por efecto — solo las siete notas rectas de la escala mayor, dentro o fuera.
    Usa los dos doblados de tono entero de la lección de la escala mayor con libertad; siguen siendo notas válidas, solo que caen en notas de la escala mayor (Fa y La) en vez de notas de blues.
    Cuando sientas que estás listo para parar, abre el menú de pausa y pulsa "Finalizar Lección" — se juzga por cuánto de lo que tocaste se mantuvo dentro de la escala mayor.

# Lección: improvisación pentatónica menor
lesson-minor-pentatonic-improv-title = Improvisación Pentatónica Menor
lesson-minor-pentatonic-improv-body =
    Otra Jam Session abierta, esta vez calibrada con la escala pentatónica menor de la lección que acabas de terminar — la escala de blues sin la 5ª bemol.
    Todo lo que toques se mide contra esa forma de seis-notas-menos-una, en vez de la hexatónica de blues completa que usan las demás lecciones de jam.
    Notarás que perdona menos — no hay nota azul que te salve si una frase se desvía, solo las cinco notas pentatónicas limpias, dentro o fuera.
    Esta es exactamente la escala a la que recurren los músicos de verdad cuando quieren algo que siempre suene "seguro" sobre casi cualquier acorde — esa fiabilidad es el motivo de practicarla por separado.
    Cuando sientas que estás listo para parar, abre el menú de pausa y pulsa "Finalizar Lección" — se juzga por cuánto de lo que tocaste se mantuvo dentro de la escala pentatónica menor.

# Lección: círculo de quintas
lesson-circle-of-fifths-title = El Círculo de Quintas
lesson-circle-of-fifths-body =
    El diagrama de arriba organiza las 12 tonalidades de forma que cada vecina queda a una quinta justa de distancia — el clásico "círculo de quintas." La tonalidad de tu armónica de referencia está arriba, marcada "harp."
    Cada posición que has estado tocando es solo un número de pasos en sentido horario alrededor de este círculo desde la tonalidad de tu armónica. La 2ª posición (cross harp — el sonido de la mayoría de la armónica de blues) es exactamente un paso en sentido horario. La 3ª posición son dos pasos.
    Por eso la 2ª posición funciona como funciona: la tonalidad de la jam está una quinta justa por encima de la tonalidad de la armónica, y una quinta es el intervalo más consonante después de la octava — lo que también explica por qué la armónica de blues se apoya tanto en las notas aspiradas en esa posición.
    También existen posiciones más allá de la 3ª — 4ª, 5ª, e incluso 12ª (un paso en el sentido *contrario* alrededor del círculo) — cada una con su propio carácter, de más brillante a más oscura y exótica cuanto más te alejas de casa.
    No necesitas memorizar este diagrama — solo recuerda la idea: elige una armónica, y cada posición en la que puede tocar ya está trazada para ti, un paso a la vez, alrededor de este círculo.

# Lección: jam del círculo de quintas
lesson-circle-of-fifths-jam-title = Jam del Círculo de Quintas
lesson-circle-of-fifths-jam-body =
    Hora de tocar el círculo de verdad, no solo leer sobre él. Esta es una Jam Session abierta con una sola armónica de C fija — sin cambiar de armónica, nunca.
    Cada pocos compases, el juego llama a una nueva posición: 1ª, luego 2ª, luego 3ª, repitiendo en ciclo. Observa la brújula junto al diagrama de tu armónica — la tonalidad resaltada cambia para mostrar cuál está activa ahora mismo.
    Cambiar de posición en una sola armónica significa cambiar dónde resuelven tus frases, no qué agujeros existen. La 1ª posición descansa en el agujero 4 (soplo); la 2ª se apoya en las notas aspiradas y descansa ahí; la 3ª cambia de nuevo. Los agujeros son los mismos — lo que cuenta como "casa" es lo que se mueve.
    No te preocupes si estás a mitad de una frase cuando cambia la llamada — termina tu idea y luego dirígete hacia las notas de descanso de la nueva posición.
    Cuando sientas que estás listo para parar, abre el menú de pausa y pulsa "Finalizar Lección" — se juzga por cuánto de lo que tocaste se mantuvo dentro de la posición que estaba llamada en ese momento.

# Lección: corcheas con swing
lesson-swing-eighths-title = Corcheas con Swing
lesson-swing-eighths-body =
    Bienvenido a la unidad de jazz. Mismo balanceo de corcheas larga-corta que shuffle-feel, pero la ventana de tiempo es más ajustada — el swing de jazz premia la precisión, no solo las notas correctas.
    Dos pares de notas esta vez en vez de uno: 4 soplado/4 aspirado, luego 5 soplado/5 aspirado, alternando. Mantén el mismo balanceo larga-corta en cada par al cambiar entre ellos.
    Si shuffle-feel te resultó cómodo, esto debería sentirse familiar pero exigir más — escucha con atención si estás cayendo justo en el tiempo con swing o solo cerca de él.
    Este sentido del tiempo más ajustado atraviesa el resto de esta unidad; las lecciones de ii-V-I y blues de jazz que siguen asumen que ya puedes swingar corcheas con limpieza.
    Ve despacio antes de acelerar — un swing limpio a un tempo moderado vale más que uno descuidado a un tempo rápido.

# Lección: notas del acorde ii-V-I
lesson-ii-v-i-chord-tones-title = Notas del Acorde ii-V-I
lesson-ii-v-i-chord-tones-body =
    La cadencia central del vocabulario del jazz: un acorde ii menor con séptima, un acorde V dominante con séptima, y un acorde I mayor con séptima, cada uno arpegiado por turno — Dm7, luego G7, luego Cmaj7.
    Dos de estas notas necesitan los doblados de un tono entero de la lección de doblados profundos (2 aspirado doblado para Fa, 3 aspirado doblado para La) — los mismos doblados que ya usaste en la escala mayor, ahora aplicados para dibujar un acorde en vez de una escala.
    Fíjate en cómo cada arpegio tiene su propio carácter: Dm7 suena en suspenso, G7 tira hacia la resolución, y Cmaj7 finalmente llega a casa — ese tirón es el punto central de esta cadencia.
    Esta es la columna vertebral armónica bajo una enorme cantidad del repertorio de jazz; en cuanto tengas estas tres formas bajo tus dedos, empezarás a escuchar este mismo movimiento ii-V-I en todas partes.
    Tócalo despacio al principio, enfocándote en caer limpio en cada nota del acorde en vez de apresurarte por los cambios.

# Lección: la forma del blues de jazz
lesson-jazz-blues-form-title = La Forma del Blues de Jazz
lesson-jazz-blues-form-body =
    Misma Jam Session abierta que las demás lecciones de improvisación, pero la progresión base ahora es el blues de jazz completo — la forma estándar de 12 compases con una vuelta ii-V-I de verdad construida en los últimos compases.
    El compás 8 sustituye por una dominante secundaria, luego los compases 9 y 10 recorren el propio ii-V que acabas de aprender en la lección anterior, resolviendo al I antes de la vuelta final en V.
    Esto se juzga igual que la improvisación sobre notas del acorde: cae específicamente en una nota del acorde a medida que cambian los acordes, no solo en cualquier lugar de la escala — pero ahora los cambios ocurren más rápido y de forma menos predecible en esa vuelta.
    El mapa de agujeros y la cuadrícula de 12 compases siguen en vivo y reflejan la forma del blues de jazz en tiempo real, así que apóyate en ellos si pierdes la cuenta de dónde empieza la vuelta.
    Cuando sientas que estás listo para parar, abre el menú de pausa y pulsa "Finalizar Lección" — el juego suma cuánto de lo que tocaste cayó en una nota del acorde.

# Lección: fundamentos del slide cromático
lesson-chromatic-slide-basics-title = Fundamentos del Slide Cromático
lesson-chromatic-slide-basics-body =
    Tu primera lección de armónica cromática: una escala cromática completa, un semitono a la vez, subiendo y bajando — usando el botón de slide para completar cada nota que el patrón de soplado/aspirado por sí solo se salta.
    A diferencia de un doblado diatónico, el slide es mecánico, no de embocadura: pulsa el botón para una nota deslizada, suéltalo para una natural, y la altura cambia instantánea y precisamente.
    Cerca de la mitad de este recorrido necesita el slide pulsado — fíjate en cómo siempre se usa para alcanzar la nota específica un semitono por encima de lo que ese agujero ya da sin pulsar nada.
    Esta escala es a propósito solo técnica, no melodía — en cuanto pulsar el slide justo en la nota correcta te resulte automático, estarás listo para usarlo musicalmente, no solo mecánicamente.
    Tómate tu tiempo para encajar el momento del slide con el inicio de la nota — un slide pulsado un poco tarde todavía se registra como la nota equivocada, sin el slide.
# Unit 6 — hole navigation and register changes

lesson-landmarks-title = Puntos de Referencia: Agujeros 1, 4, 7 y 10
lesson-landmarks-body =
    La armónica no tiene trastes y no puedes verla mientras tocas, así que te orientas por el tacto y por el oído.
    En una armónica en do, soplar en los agujeros 1, 4, 7 y 10 da siempre un do — la misma nota, una octava más arriba cada vez. Domina esos cuatro y siempre sabrás dónde estás.
    Desliza a lo largo de la armónica en lugar de separarla: las comisuras de los labios siguen en contacto y así cuentas los agujeros al tacto.
    Este ejercicio sube por los cuatro puntos de referencia y vuelve. Sin prisa — compara cada nota con la anterior y escucha la octava.

lesson-middle-register-title = El Registro Medio: Agujeros 4 a 6
lesson-middle-register-body =
    Los agujeros 4 a 6 son donde vive la mayoría de las melodías, y donde la armónica se comporta de forma más sencilla: soplar y aspirar se alternan formando do re mi fa sol la.
    Es el registro que conviene automatizar primero — todo lo demás se mide a partir de él.
    Mantén el aire suave y constante. Una nota del registro medio necesita mucho menos aire del que suele darle un principiante.
    Tócalo subiendo y bajando. El giro de arriba es donde se pierde el hilo, así que ve despacio.

lesson-hole-jumps-title = Saltar Entre Agujeros
lesson-hole-jumps-body =
    Moverse dos o tres agujeros no es lo mismo que ir al de al lado: hay que cortar el aire, moverse y volver a empezar.
    Deslizarse hace sonar todos los agujeros intermedios, y un salto limpio se convierte en un borrón. Corta el aire un instante mientras te mueves — un hueco mínimo se nota mucho menos que un borrón.
    Este ejercicio salta entre los agujeros de referencia, todos do, así que un aterrizaje erróneo se oye al momento.
    Si caes en el agujero contiguo, no lo corrijas deslizando. Para y coloca la nota otra vez.

lesson-crossing-three-four-title = Cruzar los Agujeros 3 y 4
lesson-crossing-three-four-body =
    Del agujero 3 al 4 está la costura entre el registro grave y el medio, y es el primer punto en el que la armónica deja de parecer regular.
    Subiendo tocas sol, si, do, re — pero el salto de sol a si es mayor que los pasos de alrededor, lo que hace que el movimiento parezca más largo de lo que es.
    Mantén la posición de la boca idéntica y deja que se mueva la armónica. La mayoría se pasa aquí y aterriza en el agujero 5.
    Tócalo subiendo y bajando hasta que el tamaño del movimiento deje de sorprenderte.

lesson-crossing-six-seven-title = Cruzar los Agujeros 6 y 7
lesson-crossing-six-seven-body =
    Este es el que pilla a todo el mundo. Hasta el agujero 6, aspirar es más agudo que soplar. Del 7 hacia arriba se invierte: soplar es más agudo que aspirar.
    Cruza la costura y tus instintos quedan de pronto al revés.
    Subiendo por sol, la, si, do tocas soplar 6, aspirar 6, luego aspirar 7 y soplar 7 — el aspirar y el soplar intercambian el orden justo en el cruce.
    No hay más truco que saber que llega y practicarlo hasta que la inversión sea lo que tus manos esperan. Ve despacio y dite los números de los agujeros.

lesson-low-register-title = El Registro Grave: Agujeros 1 a 3
lesson-low-register-body =
    El grave es más oscuro y lleno, y menos indulgente: las lengüetas son más largas, así que piden aire más lento y cálido y tardan un instante más en sonar.
    Apurarlas da una nota ahogada, o nada. Baja la mandíbula y piensa "o" en lugar de "i".
    Este ejercicio sube do re mi sol si do. El sol sale de aspirar 2 y no de soplar 3 — ambos dan la misma altura en una armónica en do, y elegir uno mantiene distinta cada nota del ejercicio.
    Mantén el aire lento. Si una nota se atasca, afloja en vez de apretar.

lesson-high-register-title = El Registro Agudo: Agujeros 7 a 10
lesson-high-register-body =
    Los cuatro agujeros de arriba quieren lo contrario que los cuatro de abajo: las lengüetas son cortas, así que necesitan menos aire y una boca algo más cerrada.
    Soplar fuerte aquí arriba suena estridente y puede atascar una lengüeta.
    Recuerda la inversión — del agujero 7 hacia arriba, aspirar queda por debajo de soplar, así que esta serie alterna aspirar-soplar todo el rato, y no soplar-aspirar.
    Afloja según subes. Si una nota aguda chilla, es exceso de aire, no falta.

lesson-register-leaps-title = Saltar Entre los Registros
lesson-register-leaps-body =
    El cierre de la unidad: los tres registros en un solo ejercicio, saltando entre ellos como hace un solo de verdad.
    Cada registro quiere un aire distinto — lento y cálido abajo, suave arriba — así que un salto es también un cambio de respiración, no solo de posición.
    Los do de referencia son tu red de seguridad. Si pierdes el sitio, busca el más cercano y vuelve a empezar desde ahí.
    Haz la primera pasada lo bastante despacio como para colocar cada nota con intención. La velocidad viene después de la precisión, nunca antes.


# Juego — cuenta atrás, leyenda, pistas del diagrama de armónica
gameplay-get-ready = PREPÁRATE
gameplay-legend-blow = ■ SOPLO
gameplay-legend-draw = ■ ASPIRACIÓN
harmonica-overlay-hint-view = Armónica  ·  se ilumina mientras tocas
harmonica-overlay-hint-select = Armónica  ·  haz clic en una nota para seleccionarla
gameplay-chart-info = Tono: {$key}  ♩ = {$bpm}  {$time_sig}
gameplay-chart-author = Canción: {$author}
gameplay-techniques-toggle = {$arrow} TÉCNICAS

# Menú de pausa
pause-quit-song = Salir de la canción
pause-finish-lesson = Terminar lección
pause-wait-for-note-button = ⏸ Esperar nota
pause-wait-for-note-on = Esperar nota: activado
pause-wait-for-note-off = Esperar nota: desactivado
pause-speed = Velocidad: {$pct}%
pause-adaptive-difficulty-button = Dificultad adaptativa
pause-adaptive-difficulty-on = Dificultad adaptativa: activada
pause-adaptive-difficulty-off = Dificultad adaptativa: desactivada
pause-phrase-section = Sección: {$name} — Aprendido: {$pct}%
pause-phrase-no-sections = No hay frases en esta canción
pause-drag-section-hint = Haz clic en una sección de la barra de progreso de arriba para seleccionarla
pause-notes-update-hint = Las notas se actualizan en vivo — reanuda para verlas
pause-clear-loop = Borrar bucle
pause-loop-off = Bucle: desactivado
pause-loop-range = Bucle: {$start}s–{$end}s
pause-drag-loop-hint = Arrastra en la barra de progreso de arriba para definir un rango de bucle

# Overlay del metrónomo
metronome-click-off = clic: apagado
metronome-click-on = clic: encendido
metronome-feel-straight = ritmo: recto
metronome-feel-shuffle = ritmo: shuffle

# Entrenador de Bends
bending-drill-off = Ejercicio: apagado
bending-drill-on = Ejercicio: encendido · racha {$streak}
bending-hint = Esc para volver  ·  M silencia el clic  ·  feel alterna recto/shuffle
bending-no-note-for-technique = Este agujero no tiene nota para esa técnica.
bending-key-label = Tono
bending-listen-button = 🔊 Escuchar
bending-drill-button = 🎲 Ejercicio
bending-play-it-target = Tócala — objetivo {$note}
bending-in-tune = ✓ Afinado  ({$note})
bending-cents-sharp = ↑ {$cents} cents agudo  (objetivo {$note})
bending-cents-flat = ↓ {$cents} cents grave  (objetivo {$note})
bending-detect-label = Detectar
bending-section-setup = Configuración
bending-section-target = Objetivo de Práctica
bending-section-drill = Ejercicio
bending-section-tempo = Tempo
bending-tempo-decrease = Reducir tempo
bending-tempo-increase = Aumentar tempo

# Jam Session
jam-loop-button = ↻ Bucle
jam-loop-off = Bucle: apagado
jam-loop-on = Bucle: encendido
jam-hole-map-hint = Tu armónica  ·  dorado = tono del acorde ahora mismo  ·  verde = nota de la escala de blues  ·  soplo arriba / aspiración abajo
jam-call-response-button = ⇄ Pregunta y Respuesta
jam-call-response-off = Pregunta y Respuesta: apagado
jam-call-response-on = Pregunta y Respuesta: encendido
jam-call-response-listen = Escucha…
jam-call-response-your-turn = Tu turno
jam-midi-track-mute-tooltip = Haz clic para silenciar/activar esta pista
jam-rhythm-guide = Guía de Ritmo
jam-position-label = Posición: {$position}
jam-spectrogram-style-button = ↻ Vista
jam-spectrogram-style-bars = Barras
jam-spectrogram-style-oscilloscope = Osciloscopio

# Pantalla de resultados
results-song-complete = CANCIÓN COMPLETADA
results-by-technique = Por técnica
results-new-best = ◆ ¡NUEVO RÉCORD! ◆
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
results-increase-latency = Aumentar el retraso de entrada a {$ms}ms
results-decrease-latency = Reducir el retraso de entrada a {$ms}ms
results-score = Puntuación: {$points}
results-best-score = Mejor puntuación

# Calibración de latencia
calibration-title = Calibración de Latencia
calibration-instructions = Toca cualquier nota en cada pulso — el juego mide cuánto tarda el micrófono en detectar el sonido.
calibration-mean-offset-placeholder = Desfase medio: —
calibration-mean-offset = Desfase medio: {$sign}{$ms}ms
calibration-suggested-placeholder = Actual: —   →   Sugerido: —
calibration-suggested = Actual: {$current}ms   →   Sugerido: {$suggested}ms

# Opciones
options-input-lag = Retardo de entrada
options-input-lag-tooltip = Adelanta/retrasa las notas detectadas para ajustarse al retardo de audio de tu equipo.

# Recorrido guiado del tutorial (menu::tutorial)
tutorial-step = Paso {$n} de {$total}
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

editor-tab-chart = Partitura
editor-tab-details = Detalles
# Saludo de primer arranque (menu::pages::welcome) — se muestra una sola
# vez, cuando aún no existe profile.json.
welcome-title = Bienvenido a Harmonicon
welcome-body = Tocas una armónica real en tu micrófono, y Harmonicon escucha y te puntúa mientras las notas avanzan. Nada funciona hasta que pueda oírte, así que empieza por ahí. Una armónica diatónica en Do sirve para casi todo lo de aquí.
welcome-setup-mic = Configurar el micrófono
welcome-tour = Hacer el tour guiado
welcome-lessons = Empezar con una lección
welcome-skip = Omitir por ahora
# Problemas de micrófono. `mic-warning-*` es el aviso durante el juego
# (gameplay::mic_warning_overlay), breve y sin el error bruto del
# dispositivo; `options-mic-*` es el aviso de Opciones, donde los detalles
# son justo lo que se busca.
mic-warning-failed = Sin micrófono — nada de lo que toques se puntuará. Revisa las Opciones.
mic-warning-permission = Esperando permiso del micrófono.
options-mic-failed = Sin micrófono: {$reason}
options-mic-awaiting-permission = Esperando permiso del micrófono — concédelo y reinténtalo
# Se muestra en el selector junto a un detector que solo resuelve una nota a
# la vez, porque elegirlo hace imposible acertar cualquier acorde.
algo-single-notes-only = solo notas sueltas
# Aviso durante el juego para esa combinación (gameplay::warning_banner).
chord-warning-monophonic = Esta canción tiene acordes, y el detector de tono elegido oye una nota a la vez. Elige FFT o NMF en Opciones para puntuarlos.
# "¿Qué armónica tienes?" (menu::pages::harp_check) — entre elegir la
# canción y cargarla.
harp-check-title = Tu armónica
harp-check-intro = Si la tuya es otra, dilo aquí y elige qué debe conservar el cambio.
harp-check-key = Tono
harp-check-type = Tipo
harp-check-mapping = Cuando la armónica sea distinta
harp-check-same-holes = Mismos agujeros (la melodía cambia de tono)
harp-check-transpose = Misma melodía (cambian los agujeros)
harp-check-play = Tocar
harp-check-cost-clean = Se toca tal como está escrito en esta armónica.
harp-check-cost-bends = {$count} nota(s) necesitan un bend
harp-check-cost-overblows = {$count} nota(s) necesitan un overblow
harp-check-cost-unreachable = {$count} nota(s) no se pueden tocar en esta armónica
# Nombra la armónica para la que se escribió la canción, en la página de
# elección.
harp-check-chart-harp = Escrita para una armónica {$kind} en {$key}.
harp-kind-diatonic = diatónica
harp-kind-chromatic = cromática
# El selector de pista en la página de comprobación de la armónica, visible
# solo para un archivo importado con más de una parte tocable.
harp-check-track = Parte a tocar
harp-check-track-option = {$name} — {$notes} notas, {$percent}% tocable
harp-check-track-unnamed = Pista {$index}
# Encabezado sobre los cinco niveles de entrenamiento de una lección, con
# cuánto de la escalera se ha completado (el medidor de dominio).
lesson-training-heading = Entrenamiento — {$percent}% dominado

# The skill-tree view of the curriculum: one row per track.
lesson-tree-title = Árbol de Habilidades
lesson-tree-find-next = Buscar la siguiente lección
lesson-tree-broken = Este plan de estudios no se puede dibujar: {$error}
lesson-tree-unit-progress = {$done}/{$needed}
lesson-tree-needs = Necesita: {$lessons}
lesson-tree-optional = Optativa
lesson-track-tone = Tono
lesson-track-hand = Wah de Mano
lesson-track-tongue = Bloqueo de Lengua
lesson-track-bend = Bends
lesson-track-vibrato = Vibrato
lesson-track-slide = Corredera
lesson-track-time = Tiempo
lesson-track-form = Forma
lesson-track-train = Ritmo de Tren
lesson-track-scales = Escalas
lesson-track-theory = Teoría
lesson-track-harmony = Armonía
lesson-track-vocabulary = Vocabulario
lesson-track-improv = Improvisación
lesson-track-navigation = Navegación
lesson-track-positions = Posiciones
lesson-track-ear = Entrenamiento Auditivo
lesson-unit-advanced = Optativas avanzadas
lesson-unit-rhythm-lab = Laboratorio de ritmo
lesson-unit-positions = Posiciones aplicadas
lesson-unit-ear-training = Entrenamiento auditivo
lesson-match-heard-note-title = Iguala una nota escuchada
lesson-match-heard-note-body = Escucha la llamada y después encuentra y repite cada altura. La pista de notas está oculta: usa el oído, prueba un agujero cercano si hace falta y recuerda el sonido al encontrarlo.
lesson-melodic-motion-title = Sube, baja o se repite
lesson-melodic-motion-body = Antes de tocar, nombra el movimiento que oíste: más alto, más bajo o repetido. Después imítalo. Reconocer la dirección reduce la búsqueda antes de identificar las notas exactas.
lesson-hidden-short-phrase-title = Repite una frase oculta
lesson-hidden-short-phrase-body = Guarda la frase completa en la memoria, incluida su dirección y ritmo, y tócala sin indicaciones visuales. Escucha el contorno antes de perseguir agujeros individuales.
lesson-find-tonic-title = Encuentra la tónica
lesson-find-tonic-body = Escucha qué nota hace que cada frase parezca completa y resuelve en ella. En este ejercicio con armónica en Do, el centro es un Do soplado, pero encuéntralo por el sonido antes de usar el número del agujero.
lesson-first-position-landings-title = Llegadas en primera posición
lesson-first-position-landings-body = En primera posición, Do es el centro en una armónica en Do. Toca las notas del acorde de Do mayor y termina cada frase en un Do soplado.
lesson-second-position-landings-title = Llegadas en segunda posición
lesson-second-position-landings-body = En segunda posición, Sol se convierte en el centro. Apóyate en el 2 aspirado y en las notas del acorde de Sol; escucha cómo la misma armónica resuelve hacia otro centro.
lesson-third-position-landings-title = Llegadas en tercera posición
lesson-third-position-landings-body = La tercera posición centra la música en Re y da un color menor a las notas aspiradas naturales. Usa el 4 y el 8 aspirados como referencias de tónica y termina en Re.
lesson-eighth-note-triplets-title = Tresillos de corchea
lesson-eighth-note-triplets-body = Divide cada pulso en tres partes iguales. Cuenta «un-tre-si» y mantén cada ataque a la misma distancia antes de acelerar.
lesson-straight-vs-shuffle-title = Recto frente a shuffle
lesson-straight-vs-shuffle-body = Las corcheas rectas dividen el pulso por igual. El shuffle usa la primera y la tercera parte de un tresillo. Escucha el cambio y coloca la nota corta lo bastante tarde para crear el swing.
lesson-syncopation-rests-title = Síncopas y silencios
lesson-syncopation-rests-body = Empieza en el «y» entre los pulsos numerados. Sigue contando durante cada pausa: un silencio tiene una duración exacta y da forma al siguiente contratiempo.
lesson-sixteenth-articulation-title = Articulación de semicorcheas
lesson-sixteenth-articulation-body = Coloca cuatro ataques limpios dentro de un pulso. Usa una lengua ligera y aire constante para iniciar cada nota con claridad sin tensar la mandíbula.
lesson-tempo-stability-title = Estabilidad del tempo
lesson-tempo-stability-body = Sigue el metrónomo cuando acelera y desacelera. Escucha el siguiente pulso antes de tocar y ajústate de inmediato en vez de conservar el tempo anterior.
lesson-high-blow-bends-title = Bends soplados agudos: agujeros 8–10
lesson-high-blow-bends-body =
    El bend cambia de dirección en el registro agudo: los agujeros 8, 9 y 10 se doblan al soplar. Empieza con el 8. Mantén un 8 soplado estable, reduce el espacio dentro de la boca y eleva el centro de la lengua hasta bajar medio tono. Mantén el aire uniforme; soplar con más fuerza dificulta el control.

    Repite el mismo movimiento de medio tono en 9 y 10. El agujero 10 también puede bajar un tono entero, por eso el ejercicio deja más tiempo para ese objetivo. Escucha primero la nota natural, muévete con intención a la nota doblada y vuelve sin un nuevo ataque. La frase final conecta los tres agujeros musicalmente.
lesson-overblows-title = Oversoplos
lesson-overblows-body = Los oversoplos cierran la lengüeta normal de soplado para que la lengüeta aspirada suene por encima de su altura habitual. Empieza suavemente en los agujeros 4, 5 y 6 y vuelve a la lengüeta natural sin forzar el aire.
lesson-overdraws-title = Overaspiraciones
lesson-overdraws-body = Las overaspiraciones aplican el mismo control de lengüeta a una aspiración en el registro agudo. Trabaja suavemente en los agujeros 7–10 y compara cada nota con la natural vecina antes de enlazarlas.
lesson-unit-tongue-block = Optativas de bloqueo de lengua
lesson-tongue-slap-title = Golpe de lengua
lesson-tongue-slap-body = Empieza haciendo sonar varios agujeros y coloca la lengua sobre el peine hasta dejar solo la nota objetivo. El acorde breve antes de la nota limpia crea el golpe. El micrófono no puede verificar la embocadura, así que marca la instrucción tras practicar despacio.
lesson-tongue-pull-title = Levantamiento de lengua
lesson-tongue-pull-body = Empieza con la nota aislada por el bloqueo, levanta brevemente la lengua para exponer el acorde y vuelve. Mantén el aire continuo y el movimiento pequeño.
lesson-side-pull-title = Levantamiento lateral
lesson-side-pull-body = Mantén la lengua anclada mientras levantas brevemente un lado para añadir un acorde estrecho sin perder la nota principal. Practica el movimiento sin aire antes de tocar.
lesson-tongue-rake-title = Barrido de lengua
lesson-tongue-rake-body = Sopla o aspira un acorde mientras barres los agujeros con la lengua y termina en una nota limpia. Mantén la mandíbula quieta para que la lengua produzca el movimiento.
lesson-tongue-flutter-title = Aleteo de lengua
lesson-tongue-flutter-body = Alterna rápidamente la lengua sobre los agujeros bloqueados con aire constante. Empieza con pulsos lentos y regulares; acelera solo si la nota principal y el acorde siguen claros.
lesson-moving-octaves-title = Octavas móviles
lesson-moving-octaves-body = Mantén un bloqueo de tres agujeros que deje sonar las dos notas exteriores y desliza la forma completa. El ejercicio puntúa que ambas alturas de la octava sigan presentes en cada llegada.
lesson-unit-ornaments = Adornos
lesson-track-ornaments = Adornos
lesson-slow-warble-title = Warble lento
lesson-slow-warble-body = Alterna dos agujeros vecinos sin interrumpir el aire. Empieza despacio para que ambas alturas sean claras; la partitura evalúa el patrón audible sin importar si mueves la cabeza, las manos o la armónica.
lesson-fast-shake-title = Shake rápido
lesson-fast-shake-body = Comprime la alternancia entre agujeros vecinos en semicorcheas regulares. Mantén pequeño el movimiento y conserva dos alturas distintas al aumentar la velocidad.
lesson-trill-cells-title = Células de trino
lesson-trill-cells-body = Añade una alternancia breve con la nota vecina superior y vuelve limpiamente a la principal. Trata el adorno como parte medida de la frase.
lesson-glissandi-title = Glissandi
lesson-glissandi-body = Recorre agujeros consecutivos a velocidad uniforme y llega al destino escrito. El micrófono puntúa el recorrido y la llegada; mantén constante la presión del aire.
lesson-unit-expression = Expresión
lesson-hand-vibrato-title = Vibrato de mano
lesson-hand-vibrato-body = Abre y cierra la copa de las manos alrededor de la armónica para pulsar el timbre y el volumen, manteniendo estable la nota. El método de producción es instructivo.
lesson-throat-vibrato-title = Vibrato de garganta
lesson-throat-vibrato-body = Mantén una nota relajada e introduce pulsos suaves y regulares en la garganta sin cortar el aire. Empieza despacio y detente si tensas el cuello.
lesson-diaphragm-vibrato-title = Vibrato de diafragma
lesson-diaphragm-vibrato-body = Usa pulsos pequeños y regulares de presión del aire con la boca y la garganta relajadas. Busca una onda controlada sobre una nota continua.
lesson-vibrato-control-title = Control de velocidad del vibrato
lesson-vibrato-control-body = Iguala objetivos lentos de tres pulsos y rápidos de cinco pulsos. El juego mide la frecuencia de oscilación de la altura, no el método físico ni la profundidad exacta.
lesson-dynamic-shaping-title = Crescendo y diminuendo
lesson-dynamic-shaping-body = Mantén una altura mientras creces gradualmente de suave a fuerte y después invierte la forma. La puntuación actual no puede evaluar honestamente la envolvente de volumen; escúchate y grábate.
lesson-long-tone-control-title = Control de notas largas
lesson-long-tone-control-body = Mantén cada nota durante toda su duración con un centro estable. Empieza cómodamente, relaja los hombros y reserva aire para terminar sin apretar.
lesson-unit-accompaniment = Acompañamiento
lesson-track-accompaniment = Acompañamiento
lesson-chord-rhythm-title = Ritmo de acordes
lesson-chord-rhythm-body = Coloca acordes breves y equilibrados en el pulso y suéltalos limpiamente para apoyar la música sin llenarla demasiado.
lesson-behind-singer-title = Tocar detrás de una voz
lesson-behind-singer-body = Trata la voz como primer plano. Toca suavemente solo cuando haya espacio, evita su registro y deja libres las palabras importantes. Esta decisión auditiva es instructiva.
lesson-vocal-fills-title = Rellenos vocales
lesson-vocal-fills-body = Espera durante la frase vocal imaginaria y responde al hueco con un lick breve. Termina antes de que empiece la siguiente frase.
lesson-trading-fours-title = Intercambio de cuatro compases
lesson-trading-fours-body = Alterna cuatro compases tocando con cuatro escuchando. Usa el turno silencioso para oír a una pareja imaginaria y responder después.
lesson-unit-chromatic = Armónica cromática
lesson-chromatic-button-coordination-title = Coordinación del botón
lesson-chromatic-button-coordination-body = Pulsa y suelta la corredera sin detener el aire ni mover la boca. Alterna notas naturales y elevadas hasta que el botón coincida exactamente con el cambio de altura.
lesson-enharmonic-slide-choices-title = Opciones enarmónicas
lesson-enharmonic-slide-choices-body = Una misma altura puede tener más de una escritura o ruta. Elige la que conecte la frase y prepare la nota siguiente. La puntuación no distingue alturas equivalentes, así que esta lección es instructiva.
lesson-chromatic-fragments-title = Fragmentos cromáticos
lesson-chromatic-fragments-body = Practica células cortas de semitonos en vez de recorrer toda la escala. Escucha cada forma como un gesto y sincroniza la corredera con el aire.
lesson-legato-slide-phrasing-title = Fraseo legato con corredera
lesson-legato-slide-phrasing-body = Conecta notas naturales y elevadas sin hueco. Mantén constante el aire mientras el botón cambia la lengüeta sonora.
lesson-chromatic-jazz-study-title = Luces cruzadas
lesson-chromatic-jazz-study-body = Aplica swing, fragmentos cromáticos y coordinación limpia de la corredera en este estudio original escrito para Harmonicon.
lesson-unit-musicianship = Musicalidad
lesson-intervals-title = Intervalos
lesson-intervals-body = Escucha y toca la distancia desde Do hasta una segunda, tercera, cuarta, quinta y octava. Nombra la distancia antes de la segunda nota.
lesson-chord-construction-title = Construcción de acordes
lesson-chord-construction-body = Construye una tríada apilando grados alternos: fundamental, tercera y quinta. Añade el siguiente grado para formar un acorde de séptima.
lesson-guide-tones-title = Notas guía
lesson-guide-tones-body = Las terceras y séptimas identifican el acorde y conducen suavemente al siguiente. Síguelas en un esquema ii–V–I.
lesson-arpeggio-scale-title = Del arpegio a la escala
lesson-arpeggio-scale-body = Presenta el acorde con fundamental, tercera y quinta y conecta esos puntos con notas vecinas de la escala.
lesson-transposition-title = Transposición
lesson-transposition-body = La transposición mueve cada altura por el mismo intervalo. Cambiar de tonalidad de armónica suele conservar la digitación; cambiar de posición conserva la armónica y cambia el centro tonal.
lesson-song-form-listening-title = Escuchar la forma
lesson-song-form-listening-body = Sigue grabaciones sin tocar y marca secciones repetidas, contrastes, retornos y turnarounds. Verifica tu análisis con la grabación y marca la lección.
lesson-unit-practice = Habilidades de práctica
lesson-track-practice = Práctica
lesson-slow-practice-title = Práctica lenta
lesson-slow-practice-body = Reduce la velocidad hasta controlar cada repetición y después súbela un paso pequeño. La velocidad resulta de repeticiones precisas.
lesson-loop-isolation-title = Aislar un bucle
lesson-loop-isolation-body = Selecciona la frase más corta que contenga el problema, añade un poco de contexto y repítela hasta lograr varias ejecuciones limpias antes de ampliar el bucle.
lesson-spaced-review-title = Repaso espaciado
lesson-spaced-review-body = Vuelve al material difícil después de una pausa. Mezcla una habilidad reciente con otra antigua y anota qué necesita el próximo repaso.
lesson-self-recording-title = Auto-grabación
lesson-self-recording-body = Graba una toma completa, escúchala sin el instrumento y apunta un problema de tiempo y otro de tono. Repite con un solo cambio concreto.
lesson-mixed-review-title = Repaso mixto
lesson-mixed-review-body = Alterna llegadas de posición, bends, vibrato, cambios de tempo y movimiento de registro. Los cambios ponen a prueba la memoria.
