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
select-mode = Vista de juego
play-2d = Jugar en 2D
play-3d = Jugar en 3D

# Generar Jam (base sintetizada, sin necesidad de una canción)
jam-generate-title = Generar una Base de Jam
jam-generate-start = Empezar la Jam
jam-generate-preparing = Preparando la banda…
progression-standard = Estándar
progression-quick-change = Quick Change
progression-minor-blues = Blues menor
progression-jazz-blues = Jazz Blues
position-1st = 1.ª
position-2nd = 2.ª
position-3rd = 3.ª
position-4th = 4.ª
position-5th = 5.ª
position-12th = 12.ª
scale-1st-position = 1.ª posición
scale-2nd-position = 2.ª posición
scale-3rd-position = 3.ª posición
scale-major-scale = Escala mayor
scale-minor-pentatonic = Pentatónica menor
scale-country-scale = Escala country
genre-blues = Blues
genre-jazz = Jazz
genre-rock = Rock
genre-reggae = Reggae
genre-country = Country
jam-generate-key = Tono
jam-generate-tempo = Tempo
jam-generate-progression = Progresión
jam-generate-position = Posición
jam-generate-scale = Escala
jam-generate-genre = Género
jam-generate-energy = Energía de la banda
band-energy-low = Baja
band-energy-medium = Media
band-energy-high = Alta

# Descarga inicial de los paquetes de lecciones y canciones
sync-title = Obteniendo lecciones y canciones
sync-in-progress = Harmonicon está descargando sus lecciones y canciones. El juego empezará en cuanto estén listas.
sync-repo-downloading = Descargando {$name}
sync-repo-failed = No se pudo descargar {$name}: {$error}
sync-retry = Reintentar
sync-quit = Salir

# Opciones → Lecciones y canciones (repositorios de contenido)
content-title = Lecciones y canciones
content-subtitle = Las lecciones y canciones vienen de repositorios git, que Harmonicon actualiza cuando se lo pides.
content-back-tooltip = Volver a Opciones
content-check-updates = Buscar actualizaciones
content-songs = Canciones
content-lessons = Lecciones
content-empty = Ningún repositorio.
content-add = Añadir
content-add-hint = Dirección del repositorio o carpeta:
content-add-invalid = "{$input}" no es una dirección de repositorio git (https o ssh) ni una carpeta de este ordenador.
content-add-duplicate = Ese repositorio ya está en la lista.
content-trust-note = Las lecciones y canciones son solo datos, nunca programas, pero añade solo repositorios de confianza.
content-update = Actualizar
content-download = Descargar
content-remove = Quitar
content-confirm-update = ¿Actualizar {$name} a su última versión?
content-confirm-remove = ¿Quitar {$name}? Se borran sus archivos descargados; tu progreso se conserva.
content-status-downloading = Descargando
content-status-download-failed = No se pudo descargar: {$error}
content-status-not-installed = No descargado
content-status-unusable = No se puede usar: {$reason}
content-status-local = Versión {$version}, una carpeta de este ordenador
content-status-checking = Versión {$version}, buscando actualizaciones
content-status-up-to-date = Versión {$version}, al día
content-status-update-available = Versión {$version}, hay una actualización disponible
content-status-check-failed = Versión {$version}, no se pudieron buscar actualizaciones: {$error}
content-status-installed = Versión {$version}
sync-failed = Harmonicon no pudo descargar sus lecciones y canciones.

# Créditos
credits-back-to-menu = Volver al Menú

# Selección de canción / artista
select-artist = Seleccionar Canción
circle-of-fifths-harp-label = armónica
artist-song-count-one = {$n} canción
artist-song-count-many = {$n} canciones
select-song = Seleccionar Canción
song-search = Buscar
song-sort-band = Banda
song-sort-difficulty = Dificultad
song-sort-name = Nombre de canción
song-sort-genre = Género
editor-field-genre = Género
editor-field-genre-tooltip = El género musical de la canción.
no-songs-found = No se encontraron canciones. Añade carpetas en assets/songs/<artista>/<canción>/

# Opciones
options-title = Opciones
options-subtitle-audio = Audio
options-theme = Tema
options-harmonica = Armónica
options-language = Idioma
options-adaptive-difficulty = Dificultad Adaptativa
options-adaptive-difficulty-tooltip = Ajusta automáticamente cuántas notas de la canción se muestran a la vez, según tu desempeño.
options-fullscreen = Pantalla completa
options-fullscreen-tooltip = Juega en pantalla completa en vez de en una ventana.
options-colorblind-palette = Paleta para daltonismo
options-colorblind-palette-tooltip = Usa un par fijo de colores soplar/aspirar seguro para daltonismo, en vez de los colores de nota del tema actual.
options-reduced-motion = Menos movimiento
options-reduced-motion-tooltip = Detiene el movimiento decorativo de la pista — el salto al acertar, las colas animadas de las notas — mientras las notas siguen desplazándose con normalidad.
options-zoom = Zoom
options-music = Música
options-metronome = Metrónomo
options-zoom-tooltip = Ajusta el tamaño de toda la interfaz.
options-zoom-label = {$percent}%
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
options-content = Lecciones y canciones
options-content-tooltip = De dónde vienen las lecciones y canciones, y sus actualizaciones.
options-calibrate-input-lag = Calibrar la latencia de entrada
options-calibrate-input-lag-tooltip = Mide la latencia de audio de tu equipo y la aplica automáticamente.
options-back-tooltip = Vuelve al menú principal.
options-button-style = Botones de acción
options-button-style-tooltip = Cómo muestran icono y texto los botones de acción del Editor de Canciones.
options-button-style-icon-only = Solo icono
options-button-style-text-beside-icon = Texto junto al icono
options-button-style-text-only = Solo texto
theme-back-to-options = ← Volver a Opciones
theme-title = Tema

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
mod-transpose-up = Transponer arriba
mod-transpose-down = Transponer abajo
mod-delete = Eliminar
editor-tool-select = Seleccionar
editor-tool-erase = Borrar Tramo
editor-tool-remove = Quitar Tramo
editor-tool-tempo = Tempo

# Song Editor 2 — etiquetas de los campos de metadatos
editor-field-tempo = Tempo de la Música
editor-field-pickup = Anacrusa (tiempos)
editor-field-time-signature = Compás
editor-field-time-signature-tooltip = Cuántos tiempos hay en un compás. El número inferior indica una duración de nota, por eso siempre es 1, 2, 4, 8 o 16.
editor-field-key = Tono de la Armónica
editor-field-position = Posición
editor-field-harmonica = Armónica
editor-field-music = Música de Fondo
editor-field-name = Nombre
editor-field-author = Autor
editor-field-difficulty = Dificultad
editor-field-difficulty-tooltip = Haz clic para elegir fácil, intermedio, avanzado o experto.
editor-field-feel = Sensación de la canción
editor-field-feel-tooltip = Elige subdivisión recta o shuffle para el metrónomo; predeterminado conserva la elección del jugador.
editor-field-source = Fuente
editor-field-license = Licencia
editor-field-description = Descripción
editor-field-perfect-window = Ventana perfecta (ms)
editor-field-good-window = Ventana buena (ms)
editor-field-miss-window = Ventana de fallo (ms)
editor-field-combo-enabled = Combo
editor-field-combo-base = Base del combo
editor-field-combo-step = Incremento del combo
editor-field-combo-max = Máximo del combo
editor-field-combo-decay = Decaimiento del combo (ms)
editor-field-loop-type = Tipo de bucle
editor-field-loop-repeat = Repetir bucle
editor-field-loop-start = Frase inicial del bucle
editor-field-loop-end = Frase final del bucle
editor-field-midi-track = Pista MIDI
editor-field-midi-track-tooltip = Qué pista del archivo MIDI importado colocar en la cuadrícula.
editor-field-scale = Escala
editor-field-scale-tooltip = Contra qué escala se mide el tinte rojo de "fuera de la escala" en la cuadrícula.
editor-field-text-tooltip = Haz clic para editar; escribe un valor y luego haz clic fuera o pulsa Intro para confirmar.
editor-harmonica-diatonic = ‹ Diatónica (10 orificios) ›
editor-harmonica-paddy-richter = ‹ Paddy Richter (10 orificios) ›
editor-harmonica-country-tuned = ‹ Afinación country (10 orificios) ›
editor-harmonica-natural-minor = ‹ Menor natural (10 orificios) ›
editor-harmonica-chromatic = ‹ Cromática (12 orificios) ›
editor-harmonica-chromatic-16 = ‹ Cromática (16 orificios) ›
editor-field-content-kind = Grabación
editor-content-kind-song = ‹ Grabar Canción ›
editor-content-kind-lesson = ‹ Grabar Lección ›
editor-field-snap-mode = Ajuste de Cuadrícula
editor-snap-mode-sixteenth = ‹ Semicorcheas rectas ›
editor-snap-mode-shuffle = ‹ Shuffle (corcheas con swing) ›
editor-snap-mode-triplet = ‹ Tresillos de corchea ›

# Song Editor 2 — sílabas de conteo de la regla de tiempos. Se imprimen entre
# los números de tiempo, en los ticks donde el ajuste activo puede colocar una
# nota: "y" en la mitad del tiempo (semicorcheas rectas) o en la segunda parte
# del tresillo, "a" en la tercera. El shuffle usa "a", no "y" — es la tercera
# parte de un tresillo sin la segunda.
editor-beat-count-and = y
editor-beat-count-a = a
editor-phrase-marker-tooltip = Frase en el pulso {$tick}: {$details}
editor-phrase-editor-title = Frase en {$position}
editor-phrase-editor-close = Cerrar el editor de frase
editor-phrase-editor-section = Sección
editor-phrase-editor-chord = Acorde
editor-phrase-editor-groove = Groove
editor-phrase-editor-lyric = Letra
editor-field-twelve-bar-tint = Fondo de Blues de 12 Compases

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
editor-legend-repeat-marker = Signo de repetición o casilla
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
editor-midi-import-success = ✓ Se importaron {$count} notas MIDI para armónica en {$key}
editor-midi-import-warning = ‼ Se importaron {$count} notas MIDI para armónica en {$key} — {$approximated} aproximadas, {$mixed} acordes con respiración mixta, {$duplicate} acordes con agujero duplicado
editor-midi-import-failed = ✗ Error al importar MIDI: {$detail}

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
mod-depth = Profund.
mod-depth-tooltip = Profundidad del vibrato/wah de la nota seleccionada, o de la próxima nota que coloques. Clic para avanzar ¼ → ½ → ¾ → 1.
mod-call = Llamada
mod-call-tooltip = Marca la frase de la nota seleccionada como llamada: el juego la toca primero y espera tu respuesta.
mod-split = Split
mod-split-tooltip = Marca la frase de la nota seleccionada como split con bloqueo de lengua.
mod-phrase = Frase
mod-phrase-tooltip = Abre las etiquetas de sección / acorde / groove de la frase de la nota seleccionada.
mod-transpose-up-tooltip = Transpone la selección (o todo) un semitono arriba — Ctrl+↑, Ctrl+Shift+↑ para una octava
mod-transpose-down-tooltip = Transpone la selección (o todo) un semitono abajo — Ctrl+↓, Ctrl+Shift+↓ para una octava
editor-transposed = ✓ {$count} notas transpuestas {$semitones} semitonos
editor-transposed-warning = ‼ {$count} notas transpuestas {$semitones} semitonos — {$kept} sin cambios (imposibles o sitio ocupado), {$mixed} acordes con respiración mixta, {$duplicate} acordes con agujero duplicado
editor-technique-skipped = ‼ {$count} de las notas seleccionadas no admiten esa técnica y se quedaron como estaban
mod-delete-tooltip = Eliminar la nota seleccionada
editor-tool-select-tooltip = Haz clic en un punto de la línea de tiempo y luego en un lado (o haz clic y arrastra para seleccionar un rango)
editor-tool-erase-tooltip = Haz clic en un punto de la línea de tiempo y luego en un lado (o haz clic y arrastra un tramo) para borrar sus notas, dejando un hueco
editor-tool-remove-tooltip = Haz clic en un punto de la línea de tiempo y luego en un lado (o haz clic y arrastra un tramo) para borrar sus notas y desplazar todo lo siguiente hacia atrás, cerrando el hueco
editor-tool-tempo-tooltip = Haz clic en la regla para añadir un cambio de tempo ahí, o haz clic en uno existente para quitarlo
editor-tool-meter = Compás
editor-tool-meter-tooltip = Haz clic en la regla para añadir un cambio de compás en ese tiempo; haz clic en un cambio para pasar al siguiente compás, dando la vuelta completa para quitarlo.
editor-tool-repeat = Repetir
editor-tool-repeat-tooltip = Repite los compases seleccionados en la regla. Pulsa de nuevo sobre los mismos compases para tocarlos una vez más, hasta cuatro veces, y otra más para quitar la repetición.
editor-tool-ending = Casilla
editor-tool-ending-tooltip = Convierte los compases seleccionados en una casilla: dentro de un pasaje repetido se tocan todas las veces menos la última; empezando justo después, solo la última. Pulsa de nuevo para quitarla.
editor-repeat-needs-selection = Primero selecciona compases en la regla (herramienta Seleccionar).
editor-ending-needs-repeat = Una casilla va dentro de un pasaje repetido, o empieza justo donde este termina.
editor-harmonica-toggle-tooltip = Haz clic para alternar entre afinaciones diatónicas y diseños cromáticos de 12 o 16 orificios
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
lesson-widget-feel-toggle = Recto / Shuffle / Tresillos
lesson-widget-sound-toggle = Sonido activado / desactivado
lesson-widget-key-previous = Tono anterior
lesson-widget-key-next = Tono siguiente
lesson-widget-bar-previous = Compás anterior
lesson-widget-bar-next = Compás siguiente
lesson-widget-bar-reset = Volver al compás 1
lesson-widget-section-previous = Sección anterior
lesson-widget-section-next = Sección siguiente
lesson-widget-section-reset = Volver a la sección 1
lesson-widget-step-previous = Paso anterior
lesson-widget-step-next = Paso siguiente
lesson-widget-step-reset = Volver al paso 1
lesson-mark-done = Marcar como Hecha
lesson-goal-accuracy = Objetivo: {$pct}% de precisión general
lesson-goal-technique = Objetivo: {$pct}% de precisión en las notas de {$technique}
lesson-goal-finish = Objetivo: tocarla hasta el final
lesson-goal-scale-adherence = Objetivo: {$pct}% de las notas dentro de la escala o mejor
lesson-goal-chord-tone-adherence = Objetivo: {$pct}% de las notas como notas del acorde
lesson-goal-phrase-discipline = Objetivo: {$pct}% de las notas tocadas fuera de una pausa — deja espacio
lesson-complete-banner = LECCIÓN SUPERADA
lesson-failed-banner = Objetivo no alcanzado — relee la lección e inténtalo de nuevo

# Juego — cuenta atrás, leyenda, pistas del diagrama de armónica
gameplay-get-ready = PREPÁRATE
gameplay-legend-blow = ■ SOPLO
gameplay-legend-draw = ■ ASPIRACIÓN
harmonica-overlay-hint-view = Armónica  ·  se ilumina mientras tocas
harmonica-overlay-hint-select = Armónica  ·  haz clic en una nota, o enfoca el diagrama y usa las flechas
gameplay-chart-info = Tono: {$key}  ♩ = {$bpm}  {$time_sig}
gameplay-chart-author = Canción: {$author}
gameplay-techniques-toggle = {$arrow} TÉCNICAS

# Gameplay — the judgment label at the hit line, one per scoring outcome
gameplay-judgment-perfect = ¡PERFECTO!
gameplay-judgment-good = BIEN
gameplay-judgment-early = ADELANTADO
gameplay-judgment-late = RETRASADO
gameplay-judgment-no-attack = FALLO
gameplay-judgment-wrong-pitch = NOTA INCORRECTA
gameplay-judgment-wrong-pitch-detail = esperado {$expected}  ·  oído {$heard}
gameplay-judgment-wrong-pitch-detail-unplaceable = esperado {$expected}
gameplay-judgment-incomplete-chord = ACORDE INCOMPLETO
gameplay-judgment-technique = TÉCNICA

# Menú de pausa
# Gameplay — live badges for whichever practice aids are on
gameplay-badge-speed = {$pct}% de velocidad · sin música
gameplay-badge-wait = esperando cada nota
gameplay-badge-loop = bucle {$start}s–{$end}s

# Gameplay — the wait-for-note coaching card at the hit line
gameplay-wait-play = Toca {$tab}
gameplay-wait-hearing = oyendo {$tab}
gameplay-wait-listening = escuchando…

pause-group-playback-aids = AYUDAS DE REPRODUCCIÓN
pause-group-phrase-practice = PRÁCTICA DE FRASE
pause-quit-song = Salir de la canción
pause-paused = EN PAUSA
pause-resume = Continuar
pause-restart = Reiniciar
pause-learned-label = Aprendido:
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
bending-drill-explanation = Elige objetivos del alcance actual, con preferencia por los que no has probado, controlas con menos seguridad o no practicas hace tiempo. Completa la forma de práctica para avanzar; Saltar, o dejar la armónica en silencio, avanza sin contar en tu contra.
bending-no-note-for-technique = Este agujero no tiene nota para esa técnica.
bending-key-label = Tono
bending-listen-button = 🔊 Escuchar
bending-listen-natural-button = 🔊 Natural
bending-listen-target-button = 🔊 Objetivo
bending-drill-button = 🎲 Ejercicio
bending-adv-toggle = Avanzado
bending-adv-reset = Restablecer
bending-adv-tolerance = Tolerancia
bending-adv-hold = Sostener para aprobar
bending-adv-timeout = Tiempo de intento
bending-adv-a4 = Referencia A4
bending-adv-trace = Duración del trazo
bending-adv-smoothing = Suavizado del trazo
bending-adv-subdivision = Pulso por tiempo
bending-adv-stability-heading = Este intento
bending-adv-mean = Media: {$value}
bending-adv-spread = Dispersión: {$value}
bending-adv-best-hold = Mejor sostenido: {$value}
bending-adv-vibrato = Vibrato: {$value}
bending-adv-center = Lengüeta medida: {$value}
bending-adv-clear-center = Olvidar lengüeta medida
bending-setup-button = Configuración
bending-setup-summary = Armónica en {$key} · {$algo}
bending-skip-button = Saltar
bending-progress-none = Aún sin practicar
bending-progress = {$hits} de {$attempts} controlados
bending-scope-button = Alcance
bending-scope-status = Alcance: {$scope}
bending-scope-first = Primeros bends
bending-scope-all = Todos los bends
bending-scope-blow = Bends soplados
bending-scope-over = Overbends
bending-scope-custom-selected = Personalizado (celda seleccionada)
bending-scope-custom-count = Personalizado ({$count} celdas)
bending-shape-button = Práctica
bending-shape-free = Exploración libre
bending-shape-find-hold = Encontrar y sostener
bending-shape-bend-release = Bend y regreso
bending-shape-repeated = Bends repetidos
bending-shape-ladder = Escalera de bends
bending-shape-overbend-response = Respuesta de overbend
bending-shape-status = {$shape} · {$phase}
bending-phase-waiting = esperando
bending-phase-travel = ve al objetivo
bending-phase-holding = sostén
bending-phase-returning = vuelve a natural
bending-phase-complete = completo
bending-play-it-target = Tócala — objetivo {$note}
bending-wrong-pitch = Oigo {$note} — toca el agujero {$hole} seleccionado
bending-signal-unstable = Señal inestable — mantén la nota firme
bending-rail-natural = Natural
bending-rail-target = Objetivo
bending-rail-natural-note = Natural {$note}
bending-rail-target-note = Objetivo {$note}
bending-metric-distance = Distancia {$value}
bending-metric-stability = Estabilidad {$value}
bending-metric-hold = Sostén {$value}
bending-check-natural-button = Comprobar nota natural
bending-check-natural-idle = Opcional: comprueba la nota natural {$note} antes del bend
bending-check-natural-listening = Mantén estable la nota natural {$note}…
bending-check-natural-ready = ✓ La nota natural {$note} se detecta claramente
bending-in-tune = ✓ Afinado  ({$note})
bending-cents-sharp = ↑ {$cents} cents agudo  (objetivo {$note})
bending-cents-flat = ↓ {$cents} cents grave  (objetivo {$note})
bending-detect-label = Detectar
bending-tempo-decrease = Reducir tempo
bending-tempo-increase = Aumentar tempo
bending-target-label = Objetivo: Agujero {$hole} · {$technique}
bending-technique-blow = Soplo
bending-technique-draw = Aspiración
bending-technique-bend-half = Bend de ½ tono
bending-technique-bend-whole = Bend de 1 tono
bending-technique-bend-three-half = Bend de 1½ tonos
bending-technique-overblow = Overblow
bending-technique-overdraw = Overdraw
bending-technique-hint-blow = Sopla de forma estable por el agujero con una presión suave y relajada.
bending-technique-hint-draw = Aspira de forma estable por el agujero con una presión suave y relajada.
bending-technique-hint-draw-bend-half = Aspira suavemente y mueve la parte posterior de la lengua para bajar medio tono. Moldea el aire; no aspires con más fuerza.
bending-technique-hint-draw-bend-whole = Aspira suavemente y mueve más la parte posterior de la lengua para bajar un tono. Moldea el aire; no aspires con más fuerza.
bending-technique-hint-draw-bend-three-half = Aspira suavemente y profundiza la posición de la lengua para bajar un tono y medio. Moldea el aire; no aspires con más fuerza.
bending-technique-hint-blow-bend-half = Sopla suavemente y ajusta la parte posterior de la lengua para bajar medio tono. Moldea el aire; no soples con más fuerza.
bending-technique-hint-blow-bend-whole = Sopla suavemente y mueve más la lengua para bajar un tono. Moldea el aire; no soples con más fuerza.
bending-technique-hint-blow-bend-three-half = Sopla suavemente y profundiza la posición de la lengua para bajar un tono y medio. Moldea el aire; no soples con más fuerza.
bending-technique-hint-overblow = Empieza soplando suavemente y estrecha la cavidad oral hasta que la lengüeta de soplo se cierre y suene la de aspiración. Tanto el bloqueo de lengua como el fruncido pueden funcionar; evita la fuerza.
bending-technique-hint-overdraw = Empieza aspirando suavemente y estrecha la cavidad oral hasta que la lengüeta de aspiración se cierre y suene la de soplo. Tanto el bloqueo de lengua como el fruncido pueden funcionar; evita la fuerza.
bending-technique-hint-over-unsupported = El agujero {$hole} no admite overblow ni overdraw en esta disposición.

# Jam Session
jam-loop-button = ↻ Bucle
jam-loop-off = Bucle: apagado
jam-loop-on = Bucle: encendido
jam-end-after-chorus-button = Terminar después de este chorus
jam-keep-playing = Seguir tocando
jam-ending-after-chorus = Terminando después de este chorus…
jam-ended = Terminado — reinicia o sal
jam-form-position = Chorus {$chorus} · Compás {$bar}
jam-hole-map-hint = Tu armónica  ·  dorado = tono del acorde ahora mismo  ·  verde = nota de la escala de blues  ·  soplo arriba / aspiración abajo
jam-call-response-button = ⇄ Pregunta y Respuesta
jam-call-response-off = Pregunta y Respuesta: apagado
jam-call-response-on = Pregunta y Respuesta: encendido
jam-call-response-listen = Escucha…
jam-call-response-your-turn = Tu turno
jam-call-density-button = Fraseo
jam-call-density-sparse = Fraseo: espaciado
jam-call-density-conversational = Fraseo: conversado
jam-call-density-busy = Fraseo: denso
jam-adaptive-band-button = Banda adaptativa
jam-adaptive-band-on = Banda adaptativa: activada
jam-adaptive-band-off = Banda adaptativa: desactivada
jam-detected-blow = Agujero {$hole} soplo
jam-detected-draw = Agujero {$hole} aspiración
jam-detected-none = —
jam-midi-track-mute-tooltip = Haz clic para silenciar/activar esta pista
jam-rhythm-guide = Guía de Ritmo
jam-guides-button = Guías
jam-guides-off = Guías: ocultas
jam-guides-on = Guías: visibles
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
results-increase-latency = Aumentar el retraso de entrada a {$ms}ms
results-decrease-latency = Reducir el retraso de entrada a {$ms}ms
results-score = Puntuación: {$points}
results-best-score = Mejor puntuación
results-accuracy-caption = de precisión
results-observation-technique = {$technique}: {$hits} de {$total} acertaron — ahí están los próximos puntos
results-observation-missed = {$misses} de {$total} notas ni sonaron — baja el ritmo con la Velocidad de práctica o Esperar la nota
results-observation-late = El {$pct}% de tus aciertos llegaron tarde — anticipa la línea de acierto, o aplica el ajuste de tiempo de abajo
results-observation-early = El {$pct}% de tus aciertos llegaron pronto — deja que la nota llegue a la línea, o aplica el ajuste de tiempo de abajo
results-observation-leaky = Solo {$clean} de {$total} ataques fueron limpios — se cuela un agujero vecino; cierra más la embocadura
results-observation-solid = Nada destaca — puedes pasar a una canción más difícil
results-timing = Tiempo (media {$ms}ms)
results-timing-early = pronto {$n}
results-timing-on-time = a tiempo {$n}
results-timing-late = tarde {$n}
results-lesson-reached = Esta vez: {$pct}%
results-retry = Reintentar
results-practice-missed = Practicar el tramo fallado
results-continue = Continuar

# Calibración de latencia
calibration-title = Calibración de Latencia
calibration-mic-label = Mic
calibration-instructions = Toca cualquier nota en cada pulso — el juego mide cuánto tarda el micrófono en detectar el sonido.
calibration-mean-offset-placeholder = Desfase medio: —
calibration-mean-offset = Desfase medio: {$sign}{$ms}ms
calibration-suggested-placeholder = Actual: —   →   Sugerido: —
calibration-suggested = Actual: {$current}ms   →   Sugerido: {$suggested}ms
calibration-get-ready = Prepárate…
calibration-hits-recorded = {$hits} / {$total} golpes registrados
calibration-complete = ¡Calibración completada!
calibration-start = Empezar
calibration-apply = Aplicar
calibration-try-again = Intentar de nuevo
calibration-cancel = ← Cancelar

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
tutorial-title-mode-select = Selector de canciones
tutorial-body-mode-select = Explora todas las canciones, ordénalas por banda, dificultad, nombre o género, búscalas y elige 2D o 3D aquí.
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
help-first-run = Primeros pasos
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
harp-summary-diatonic = Diatónica
harp-summary-chromatic = Cromática
harp-summary-holes = {$n} agujeros
harp-summary-position = {$pos} posición
harp-banner-use = Usa una armónica en {$key}
harp-banner-key = tono de {$key}
harp-banner-fallback = Tocando en {$key}
harp-row-blow = soplo
harp-row-draw = aspiración
harp-row-overblow = overblow
harp-row-overdraw = overdraw
harp-row-slide = slide
# El selector de pista en la página de comprobación de la armónica, visible
# solo para un archivo importado con más de una parte tocable.
harp-check-track = Parte a tocar
harp-check-track-option = {$name} — {$notes} notas, {$percent}% tocable
harp-check-track-unnamed = Pista {$index}
# Encabezado sobre los cinco niveles de entrenamiento de una lección, con
# cuánto de la escalera se ha completado (el medidor de dominio).
lesson-training-heading = Entrenamiento — {$percent}% dominado
# Los cinco niveles de entrenamiento: el nombre de cada uno y lo que pide al jugador.
lesson-training-tier-isolate = Aislar
lesson-training-tier-isolate-about = La técnica sola, despacio, en un solo agujero.
lesson-training-tier-consolidate = Consolidar
lesson-training-tier-consolidate-about = El mismo ejercicio, un poco más rápido.
lesson-training-tier-vary = Variar
lesson-training-tier-vary-about = Todos los agujeros de la lección, en todas las profundidades que alcanzan.
lesson-training-tier-in-context = En Contexto
lesson-training-tier-in-context-about = La técnica dentro de una frase musical, en corcheas.
lesson-training-tier-interleave = Intercalar
lesson-training-tier-interleave-about = Un orden impredecible, para que no se pueda tocar de memoria.
# La meta de un nivel de entrenamiento: la línea de meta de la lección, luego el tempo y la duración del nivel.
lesson-training-goal = {$goal}, a {$bpm} BPM durante {$bars} compases
lesson-training-start = Empezar Entrenamiento

# The skill-tree view of the curriculum: one row per track.
lesson-tree-title = Árbol de Habilidades
lesson-tree-find-next = Buscar la siguiente lección
lesson-tree-broken = Este plan de estudios no se puede dibujar: {$error}
lesson-tree-unit-progress = {$done}/{$needed}
# El medidor de dominio de una pista sobre el árbol: cuánto de las escaleras de entrenamiento se ha completado.
lesson-tree-track-mastery = {$track} — {$percent}%
# La cola de repaso de calentamiento sobre el árbol: la etiqueta y un entrenamiento pendiente como "lección · nivel".
lesson-tree-warmup = Calentamiento:
lesson-tree-review-due = ↻ Repaso pendiente: vuelve a tocar su nivel más alto para mantener la habilidad
lesson-tree-warmup-item = {$lesson} · {$tier}
# La racha de práctica sobre el árbol, mostrada a partir de dos días.
lesson-tree-streak = {$days} días seguidos de práctica
lesson-tree-needs = Necesita: {$lessons}
lesson-tree-optional = Optativa

# Scored play — technique cue beside a note head, and the technique coach row
cue-target = → {$note}
cue-vibrato = vib {$rate}/s
cue-wah = wah {$rate}/s
coach-follow-pulse = Sigue
coach-bend-more = Dobla más
coach-on-target = Mantén
coach-too-far = Te pasaste
coach-swing-more = Más amplio
coach-faster = Más rápido
coach-slower = Más lento
coach-on-rate = Bien
coach-rate = {$rate}/s

song-clear-search = Borrar
song-results = {$count} canciones
song-none-selected = Selecciona una canción
song-no-results = No hay canciones coincidentes. Prueba menos palabras o borra la búsqueda.
song-picker-keys = Arriba/Abajo: Seleccionar   Enter: Jugar   /: Buscar   Esc: Volver
song-result-one = 1 canción

song-update = Actualizar canciones: {$name}
