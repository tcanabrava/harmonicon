# Harmonicon — Español (es-ES) UI strings.
#
# Mantén las claves sincronizadas con assets/locales/en-US/main/ui.ftl.

app-title = Harmonicon

# Menú principal
menu-play = Jugar
menu-song-editor-2 = Editor de Canciones
menu-options = Opciones
menu-credits = Créditos
menu-quit = Salir

# Menú de juego
play-song = Tocar Canción
jam-session = Sesión Jam
bending-trainer = Entrenador de Bends

# Selección de modo
select-mode = Seleccionar Modo
play-2d = Jugar en 2D
play-3d = Jugar en 3D

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

# Lección: forma de las manos / wah
lesson-hand-wah-title = La Forma de las Manos y el Wah
lesson-hand-wah-body =
    Tus manos son el control de timbre de la armónica. Ahuécalas detrás del instrumento formando una cámara de aire sellada, y abre y cierra el sello para que "hable": "ua".
    Sujeta la armónica entre el pulgar y el índice de una mano, y sella la otra mano detrás, como una almeja.
    Copa cerrada = sonido oscuro, apagado. Copa abierta = brillante y fuerte. Abrir la copa rítmicamente mientras suena una nota produce el clásico wah-wah.
    En este ejercicio, sostén cada nota con firmeza y abre-y-cierra la copa unas dos veces por segundo — el juego escucha ese pulso en tu sonido.
    Mantén la respiración constante; solo se mueven las manos. Si no registra nada, aprieta el sello de la copa — casi todo el efecto vive en el último centímetro del cierre.

# Lección: leer la rejilla de 12 compases
lesson-twelve-bar-title = Leer la Rejilla del Blues de 12 Compases
lesson-twelve-bar-body =
    Casi toda canción de blues sigue el mismo ciclo de 12 compases — aprende a leerlo una vez y podrás seguir cualquier jam de blues del planeta.
    Cada celda de la rejilla es un compás de cuatro tiempos. Los números romanos nombran los acordes: I es el acorde de casa, IV el viaje intermedio, V la tensión del regreso.
    El esquema clásico: cuatro compases de I, dos de IV, dos de I, uno de V, uno de IV y dos compases finales de I (el último suele cambiarse a V para lanzar el siguiente chorus — el "turnaround").
    Cuéntalo en voz alta: "UN dos tres cuatro, DOS dos tres cuatro..." — doce compases, y el ciclo vuelve a empezar.
    Verás esta rejilla en vivo en la Jam Session, donde el compás actual se ilumina mientras suena la base. Después de esta lección, abre una Jam Session y solo mira pasar unos ciclos, contando, antes de tocar una sola nota.
