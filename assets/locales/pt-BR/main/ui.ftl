# Harmonicon — Português do Brasil (pt-BR) UI strings.
#
# Mantenha as chaves em sincronia com assets/locales/en-US/main/ui.ftl.

app-title = Harmonicon

# Menu principal
menu-play = Tocar
menu-options = Opções
menu-help = Ajuda / Sobre
menu-credits = Créditos
menu-tutorial = Tutorial
menu-quit = Sair

# Menu de tocar
play-song = Tocar Música
menu-create-song = Criar Música
jam-session = Jam Session
bending-trainer = Treino de Bends

# Submenu de Jam Session
jam-session-pick-song = Escolher uma Música
jam-generate = Gerar Jam

# Menu de Ajuda / Sobre
help-about-title = Ajuda / Sobre
help-documentation = Documentação
help-docs-not-found = A documentação ainda não foi gerada localmente — rode `mdbook build` em docs/book/.
menu-about = Sobre
about-title = Sobre o Harmonicon
about-body = Harmonicon é um jogo de ritmo para gaita diatônica e cromática: toque uma gaita de verdade no microfone e seja pontuado em tempo real contra uma partitura, feito para ensinar gaita de blues e jazz através do jogo.
about-version = Versão { $version }

# Seleção de modo
select-mode = Visualização de jogo
play-2d = Tocar em 2D
play-3d = Tocar em 3D

# Gerar Jam (base sintetizada, sem precisar de uma música)
jam-generate-title = Gerar uma Base de Jam
jam-generate-start = Começar a Jam
jam-generate-preparing = Preparando a banda…
progression-standard = Padrão
progression-quick-change = Quick Change
progression-minor-blues = Blues menor
progression-jazz-blues = Jazz Blues
position-1st = 1ª
position-2nd = 2ª
position-3rd = 3ª
position-4th = 4ª
position-5th = 5ª
position-12th = 12ª
scale-1st-position = 1ª posição
scale-2nd-position = 2ª posição
scale-3rd-position = 3ª posição
scale-major-scale = Escala maior
scale-minor-pentatonic = Pentatônica menor
scale-country-scale = Escala country
genre-blues = Blues
genre-jazz = Jazz
genre-rock = Rock
genre-reggae = Reggae
genre-country = Country
jam-generate-key = Tom
jam-generate-tempo = Andamento
jam-generate-progression = Progressão
jam-generate-position = Posição
jam-generate-scale = Escala
jam-generate-genre = Gênero
jam-generate-energy = Energia da banda
band-energy-low = Baixa
band-energy-medium = Média
band-energy-high = Alta

# Download inicial dos pacotes de lições e músicas
sync-title = Obtendo lições e músicas
sync-in-progress = O Harmonicon está baixando suas lições e músicas. O jogo começará assim que estiverem prontas.
sync-repo-downloading = Baixando {$name}
sync-repo-failed = Não foi possível baixar {$name}: {$error}
sync-retry = Tentar novamente
sync-quit = Sair

# Opções → Lições e músicas (repositórios de conteúdo)
content-title = Lições e músicas
content-subtitle = As lições e músicas vêm de repositórios git, que o Harmonicon atualiza quando você pede.
content-back-tooltip = Voltar às Opções
content-check-updates = Procurar atualizações
content-songs = Músicas
content-lessons = Lições
content-empty = Nenhum repositório.
content-add = Adicionar
content-add-hint = Endereço do repositório ou pasta:
content-add-invalid = "{$input}" não é um endereço de repositório git (https ou ssh) nem uma pasta deste computador.
content-add-duplicate = Esse repositório já está na lista.
content-trust-note = Lições e músicas são apenas dados, nunca programas, mas adicione só repositórios em que você confia.
content-update = Atualizar
content-download = Baixar
content-remove = Remover
content-confirm-update = Atualizar {$name} para a versão mais recente?
content-confirm-remove = Remover {$name}? Os arquivos baixados são apagados; seu progresso é mantido.
content-status-downloading = Baixando
content-status-download-failed = Não foi possível baixar: {$error}
content-status-not-installed = Não baixado
content-status-unusable = Não pode ser usado: {$reason}
content-status-local = Versão {$version}, uma pasta deste computador
content-status-checking = Versão {$version}, procurando atualizações
content-status-up-to-date = Versão {$version}, atualizado
content-status-update-available = Versão {$version}, há uma atualização disponível
content-status-check-failed = Versão {$version}, não foi possível procurar atualizações: {$error}
content-status-installed = Versão {$version}
sync-failed = O Harmonicon não conseguiu baixar suas lições e músicas.

# Créditos
credits-back-to-menu = Voltar ao Menu

# Seleção de música / artista
select-artist = Selecionar Música
circle-of-fifths-harp-label = gaita
artist-song-count-one = {$n} música
artist-song-count-many = {$n} músicas
select-song = Selecionar Música
song-search = Buscar
song-sort-band = Banda
song-sort-difficulty = Dificuldade
song-sort-name = Nome da música
song-sort-genre = Gênero
editor-field-genre = Gênero
editor-field-genre-tooltip = O gênero musical da música.
no-songs-found = Nenhuma música encontrada. Adicione pastas em assets/songs/<artista>/<música>/

# Opções
options-title = Opções
options-subtitle-audio = Áudio
options-theme = Tema
options-harmonica = Gaita
options-language = Idioma
options-adaptive-difficulty = Dificuldade Adaptativa
options-adaptive-difficulty-tooltip = Ajusta automaticamente quantas notas da música são mostradas de uma vez, de acordo com o seu desempenho.
options-fullscreen = Tela cheia
options-fullscreen-tooltip = Joga em tela cheia em vez de em uma janela.
options-colorblind-palette = Paleta para daltonismo
options-colorblind-palette-tooltip = Usa um par fixo de cores sopro/sucção seguro para daltonismo, em vez das cores de nota do tema atual.
options-reduced-motion = Menos movimento
options-reduced-motion-tooltip = Para o movimento decorativo da pista — o pulo ao acertar, as caudas animadas das notas — enquanto as notas continuam rolando normalmente.
options-zoom = Zoom
options-music = Música
options-metronome = Metrônomo
options-zoom-tooltip = Ajusta o tamanho de toda a interface.
options-zoom-label = {$percent}%
options-pitch-detect = Detecção de tom
options-microphone = Microfone
options-microphone-tooltip = De qual dispositivo de entrada captar sua gaita.
options-mic-retry-tooltip = Tenta reconectar ao microfone.
options-note-labels = Rótulos das notas
options-note-labels-tooltip = Mostra as notas que caem como números de furo, em vez de setas de sopro/sucção.
options-harmonica-tooltip = Qual modelo de gaita aparece no jogo em 3D.
options-music-volume-tooltip = Volume da faixa de acompanhamento.
options-metronome-volume-tooltip = Volume do clique do metrônomo.
options-theme-tooltip = Muda o tema visual dos menus.
options-content = Lições e músicas
options-content-tooltip = De onde vêm as lições e as músicas, e suas atualizações.
options-calibrate-input-lag = Calibrar a latência de entrada
options-calibrate-input-lag-tooltip = Mede a latência de áudio do seu equipamento e a aplica automaticamente.
options-back-tooltip = Volta ao menu principal.
options-button-style = Botões de ação
options-button-style-tooltip = Como os botões de ação do Editor de Músicas mostram ícone e texto.
options-button-style-icon-only = Somente ícone
options-button-style-text-beside-icon = Texto ao lado do ícone
options-button-style-text-only = Somente texto
theme-back-to-options = ← Voltar às Opções
theme-title = Tema

# Compartilhado
back = ← Voltar

# Editor de Músicas 2 — botões de transporte e painel de modificadores
editor-back-label = Voltar
editor-mode-edit = Editar
editor-mode-record = Gravar
editor-mode-play = Reproduzir
editor-mode-expected = Marcar notas corretas
editor-lock = Bloquear
editor-undo = Desfazer
editor-redo = Refazer
editor-delete = Excluir
editor-copy = Copiar
editor-paste = Colar
editor-metronome = Metrônomo
editor-play = Tocar
editor-pause = Pausar
editor-stop = Parar
editor-practice = Praticar
editor-finish = Concluir
editor-save = Salvar
editor-load = Carregar
editor-browse = 📂 Procurar
editor-import-midi = ♬ Importar MIDI
mod-blow = Soprar
mod-draw = Puxar
mod-bend = Dobrar
mod-overblow = Oversopro
mod-overdraw = Overpuxar
mod-slide = Slide
mod-wah = Wah
mod-vibrato = Vibrato
mod-transpose-up = Transpor acima
mod-transpose-down = Transpor abaixo
mod-delete = Apagar
editor-tool-select = Selecionar
editor-tool-erase = Apagar Trecho
editor-tool-remove = Remover Trecho
editor-tool-tempo = Tempo

# Editor de Músicas 2 — rótulos dos campos de metadados
editor-field-tempo = Andamento da Música
editor-field-pickup = Anacruse (tempos)
editor-field-time-signature = Fórmula de Compasso
editor-field-time-signature-tooltip = Quantos tempos há em um compasso. O número de baixo indica uma duração de nota, por isso é sempre 1, 2, 4, 8 ou 16.
editor-field-key = Tom do Gaita
editor-field-position = Posição
editor-field-harmonica = Gaita
editor-field-music = Música de Fundo
editor-field-name = Nome
editor-field-author = Autor
editor-field-difficulty = Dificuldade
editor-field-difficulty-tooltip = Clique para escolher fácil, intermediário, avançado ou especialista.
editor-field-feel = Sensação da música
editor-field-feel-tooltip = Escolha subdivisão reta ou shuffle para o metrônomo; padrão mantém a escolha do jogador.
editor-field-source = Fonte
editor-field-license = Licença
editor-field-description = Descrição
editor-field-perfect-window = Janela perfeita (ms)
editor-field-good-window = Janela boa (ms)
editor-field-miss-window = Janela de erro (ms)
editor-field-combo-enabled = Combo
editor-field-combo-base = Base do combo
editor-field-combo-step = Incremento do combo
editor-field-combo-max = Máximo do combo
editor-field-combo-decay = Decaimento do combo (ms)
editor-field-loop-type = Tipo de loop
editor-field-loop-repeat = Repetir loop
editor-field-loop-start = Frase inicial do loop
editor-field-loop-end = Frase final do loop
editor-field-midi-track = Faixa MIDI
editor-field-midi-track-tooltip = Qual faixa do arquivo MIDI importado colocar na grade.
editor-field-scale = Escala
editor-field-scale-tooltip = Contra qual escala o tom vermelho de "fora da escala" na grade é medido.
editor-field-text-tooltip = Clique para editar; digite um valor e depois clique fora ou pressione Enter para confirmar.
editor-harmonica-diatonic = ‹ Diatônica (10 buracos) ›
editor-harmonica-paddy-richter = ‹ Paddy Richter (10 buracos) ›
editor-harmonica-country-tuned = ‹ Afinação country (10 buracos) ›
editor-harmonica-natural-minor = ‹ Menor natural (10 buracos) ›
editor-harmonica-chromatic = ‹ Cromática (12 buracos) ›
editor-harmonica-chromatic-16 = ‹ Cromática (16 buracos) ›
editor-field-content-kind = Gravação
editor-content-kind-song = ‹ Gravar Música ›
editor-content-kind-lesson = ‹ Gravar Lição ›
editor-field-snap-mode = Encaixe da Grade
editor-snap-mode-sixteenth = ‹ Semicolcheias retas ›
editor-snap-mode-shuffle = ‹ Shuffle (colcheias swingadas) ›
editor-snap-mode-triplet = ‹ Tercinas de colcheia ›

# Song Editor 2 — sílabas de contagem da régua de tempos. Impressas entre os
# números dos tempos, nos ticks em que o encaixe ativo pode colocar uma nota:
# "e" na metade do tempo (semicolcheias retas) ou na segunda parte da tercina,
# "a" na terceira. O shuffle usa "a", não "e" — é a terceira parte de uma
# tercina sem a segunda.
editor-beat-count-and = e
editor-beat-count-a = a
editor-phrase-marker-tooltip = Frase no pulso {$tick}: {$details}
editor-phrase-editor-title = Frase em {$position}
editor-phrase-editor-close = Fechar o editor de frase
editor-phrase-editor-section = Seção
editor-phrase-editor-chord = Acorde
editor-phrase-editor-groove = Groove
editor-phrase-editor-lyric = Letra
editor-field-twelve-bar-tint = Fundo de Blues de 12 Compassos

# Song Editor 2 — legenda de cores (terceira coluna do formulário)
editor-legend-toggle = Legenda
editor-legend-toggle-tooltip = Mostra ou esconde a coluna de legenda de cores.
editor-legend-notes = Cores das notas (grade)
editor-legend-normal = Nota sopro/aspiração normal
editor-legend-bend = Bend (quanto mais fundo, mais vermelho)
editor-legend-overblow = Overblow
editor-legend-overdraw = Overdraw
editor-legend-slide = Slide (só cromática)
editor-legend-out-of-scale = Tom vermelho = fora da escala da música
editor-legend-selected = Borda dourada = nota selecionada
editor-legend-blow = Sopro
editor-legend-draw = Aspiração
editor-legend-dragging = Ao arrastar uma nota
editor-legend-drag-ok = Posição de destino válida
editor-legend-drag-bad = Inválida (sobreposição ou técnica incompatível)
editor-legend-elsewhere = Em outras partes da tela
editor-legend-tempo-marker = Marcador de mudança de andamento (cabeçalho da grade)
editor-legend-repeat-marker = Ritornelo ou casa
editor-legend-triplet-line = Linha de subdivisão de tercina (tiques 4/8 do tempo)
editor-legend-split-point = Ferramenta Selecionar: ponto de divisão
editor-legend-range-preview = Ferramenta Selecionar: prévia do intervalo
editor-legend-active-button = Botão de modo/ferramenta atualmente ativo
editor-legend-scrollbar-blow = Minimapa da barra de rolagem: nota de sopro
editor-legend-scrollbar-draw = Minimapa da barra de rolagem: nota de aspiração
editor-legend-scrollbar-note = Nota: aqui esse azul/laranja significa sopro/aspiração — um significado diferente das cores das notas acima, que representam a técnica.

# Editor de Músicas 2 — campos exclusivos de lição (mostrados enquanto
# "Gravar Lição" está ativo)
editor-lesson-details-header = Detalhes da Lição
editor-field-lesson-id = ID da Lição
editor-field-lesson-unit = Unidade
editor-field-lesson-explanation = Explicação
editor-field-lesson-prerequisites = Pré-requisitos
editor-field-lesson-pass-criteria = Critério de Aprovação
editor-field-lesson-threshold = Limite
editor-field-lesson-technique = Técnica
editor-field-lesson-progression = Progressão
editor-field-lesson-scale = Escala da lição
editor-field-lesson-path = Percurso

# Editor de Músicas 2 — títulos dos diálogos de arquivo
dialog-save-chart = Salvar partitura
dialog-load-chart = Carregar partitura
dialog-save-lesson = Salvar lição
dialog-load-lesson = Carregar lição
dialog-select-music = Selecionar música de fundo
dialog-select-midi = Selecionar arquivo MIDI
dialog-file-name = Nome do arquivo:
dialog-cancel-esc = Cancelar  (Esc)

# Editor de Músicas 2 — mensagens de validação de arrastar
drag-denied-bend = Este buraco não suporta esta profundidade de dobra
drag-denied-overblow = Oversopro está disponível apenas nos buracos 1–6
drag-denied-overdraw = Overpuxar está disponível apenas nos buracos 7–10
drag-denied-overlap = Já existe uma nota aqui

# Editor de Músicas 2 — confirmação da ferramenta Apagar/Remover da linha do tempo
editor-confirm-erase = Apagar do compasso {$from} ao {$to}? Toda nota nesse trecho será apagada — o resto da música fica exatamente onde está.
editor-confirm-remove = Remover do compasso {$from} ao {$to}? Toda nota nesse trecho será apagada, e tudo depois vai se deslocar pra frente pra fechar o vazio.

# Editor de Músicas 2 — feedback do modo de prática
practice-no-music = Nenhuma música de fundo definida — toque seguindo a partitura!
practice-prompt = ▶ Toque {$note}…
practice-wrong-note = ▶ {$got} → precisa de {$expected}
practice-hit-perfect = ✓ PERFEITO  {$note}  +{$pts} pts
practice-hit-good = ✓ BOM  {$note}  +{$pts} pts
practice-missed = ✗ Perdeu {$note}
practice-done = Feito — {$hits}/{$total} notas  ·  {$score} pts
editor-record-status = ⏺ Gravando — {$count} notas capturadas
editor-count-in-status = ◔ Prepare-se — gravação em {$seconds}s
editor-metronome-tooltip = Ativa/desativa o clique do metrônomo durante Gravar/Tocar/Praticar
editor-save-success = ✓ Salvo: {$path}
editor-save-warning = ‼ Salvo com avisos: {$detail}
editor-save-failed = ✗ Falha ao salvar: {$detail}
editor-load-success = ✓ Carregado: {$path}
editor-load-failed = ✗ Falha ao carregar: {$detail}
editor-midi-import-success = ✓ {$count} notas MIDI importadas para gaita em {$key}
editor-midi-import-warning = ‼ {$count} notas MIDI importadas para gaita em {$key} — {$approximated} aproximadas, {$mixed} acordes com respiração mista, {$duplicate} acordes com furo duplicado
editor-midi-import-failed = ✗ Falha ao importar MIDI: {$detail}

# Editor de Músicas 2 — dicas dos botões
editor-back-tooltip = Sair do editor e voltar ao menu principal
editor-mode-edit-tooltip = Mudar para o modo Editar — posicione, mova e edite notas na grade
editor-mode-record-tooltip = Mudar para o modo Gravar — grave notas da sua gaita direto na grade
editor-mode-play-tooltip = Mudar para o modo Reproduzir — toque ou pratique a partitura
editor-mode-expected-tooltip = Somente builds de desenvolvimento: marque as notas corretas por cima de uma gravação, para o benchmark de detecção de notas (note_bench)
editor-lock-tooltip = Bloquear a grade para evitar edições acidentais durante a revisão
editor-undo-tooltip = Desfazer a última edição (colocar/mover/excluir nota, colar, Apagar/Remover, uma gravação inteira, ...)
editor-redo-tooltip = Refazer a última edição desfeita
editor-delete-tooltip = Excluir a(s) nota(s) selecionada(s)
editor-copy-tooltip = Copiar a(s) nota(s) selecionada(s)
editor-paste-tooltip = Colar as últimas notas copiadas no início da visualização atual
editor-save-tooltip = Salvar esta partitura em um arquivo .harpchart
editor-load-tooltip = Carregar uma partitura de um arquivo .harpchart
editor-play-tooltip = Iniciar ou retomar a reprodução da partitura
editor-pause-tooltip = Pausar a reprodução onde está
editor-stop-tooltip = Parar a reprodução e voltar o cursor ao início
editor-practice-tooltip = Modo prática — toque junto com sua gaita e receba feedback ao vivo
editor-record-play-tooltip = Começa a gravar da posição atual — ou retoma uma gravação pausada
editor-record-stop-tooltip = Termina a gravação — o cursor fica onde parou
editor-finish-tooltip = Conclui a gravação e volta ao início — gravar de novo substitui as notas sobre as quais você tocar
editor-record-detect-label = Detectar
editor-debug-recording-button = Gravação de Depuração
editor-debug-recording-tooltip = Somente builds de desenvolvimento: também grava o áudio bruto do microfone em assets/debug_songs/<song>/ ao salvar, para diagnosticar problemas de detecção de tom depois
editor-debug-recording-erase = Apagar Gravação
editor-debug-recording-erase-tooltip = Descarta o áudio bruto capturado para que a próxima gravação comece do zero
editor-debug-recording-off = Desligado
editor-debug-recording-armed = Pronto — pressione Play para gravar
editor-debug-recording-status = Gravando — {$secs}s capturados
mod-blow-tooltip = Definir a nota selecionada como sopro (expirar)
mod-draw-tooltip = Definir a nota selecionada como puxada (inspirar)
mod-bend-tooltip = Alternar a profundidade da dobra da nota selecionada: nenhuma → meio tom → tom inteiro → tom e meio
mod-overblow-tooltip = Definir a nota selecionada como oversopro (técnica avançada de sopro, apenas diatônica)
mod-overdraw-tooltip = Definir a nota selecionada como overpuxada (técnica avançada de puxada, apenas diatônica)
mod-slide-tooltip = Definir a nota selecionada para usar o botão slide (apenas gaitas cromáticas)
mod-wah-tooltip = Alternar a taxa de wah-wah da nota selecionada
mod-vibrato-tooltip = Alternar a taxa de vibrato da nota selecionada
mod-depth = Profund.
mod-depth-tooltip = Profundidade do vibrato/wah da nota selecionada, ou da próxima nota que você colocar. Clique para avançar ¼ → ½ → ¾ → 1.
mod-call = Chamada
mod-call-tooltip = Marca a frase da nota selecionada como chamada: o jogo a toca primeiro e espera sua resposta.
mod-split = Split
mod-split-tooltip = Marca a frase da nota selecionada como split de bloqueio de língua.
mod-phrase = Frase
mod-phrase-tooltip = Abre os rótulos de seção / acorde / groove da frase da nota selecionada.
mod-transpose-up-tooltip = Transpõe a seleção (ou tudo) um semitom acima — Ctrl+↑, Ctrl+Shift+↑ para uma oitava
mod-transpose-down-tooltip = Transpõe a seleção (ou tudo) um semitom abaixo — Ctrl+↓, Ctrl+Shift+↓ para uma oitava
editor-transposed = ✓ {$count} notas transpostas {$semitones} semitons
editor-transposed-warning = ‼ {$count} notas transpostas {$semitones} semitons — {$kept} mantidas (impossíveis ou lugar ocupado), {$mixed} acordes com respiração mista, {$duplicate} acordes com furo duplicado
editor-technique-skipped = ‼ {$count} das notas selecionadas não aceitam essa técnica e ficaram como estavam
mod-delete-tooltip = Apagar a nota selecionada
editor-tool-select-tooltip = Clique num ponto da linha do tempo e depois num lado (ou clique e arraste para selecionar um intervalo)
editor-tool-erase-tooltip = Clique num ponto da linha do tempo e depois num dos lados (ou clique e arraste um trecho) para apagar as notas dali, deixando um vazio
editor-tool-remove-tooltip = Clique num ponto da linha do tempo e depois num dos lados (ou clique e arraste um trecho) para apagar as notas dali e deslocar tudo depois pra frente, fechando o vazio
editor-tool-tempo-tooltip = Clique na régua para adicionar uma mudança de andamento ali, ou clique numa já existente para removê-la
editor-tool-meter = Compasso
editor-tool-meter-tooltip = Clique na régua para adicionar uma mudança de fórmula de compasso naquele tempo; clique numa mudança para avançar à próxima fórmula, dando a volta completa para removê-la.
editor-tool-repeat = Repetir
editor-tool-repeat-tooltip = Repete os compassos selecionados na régua. Pressione de novo nos mesmos compassos para tocá-los mais uma vez, até quatro vezes, e mais uma para remover a repetição.
editor-tool-ending = Casa
editor-tool-ending-tooltip = Transforma os compassos selecionados numa casa: dentro de um trecho repetido, são tocados todas as vezes menos a última; começando logo depois dele, só na última. Pressione de novo para removê-la.
editor-repeat-needs-selection = Selecione compassos na régua primeiro (ferramenta Selecionar).
editor-ending-needs-repeat = Uma casa fica dentro de um trecho repetido, ou começa exatamente onde ele termina.
editor-harmonica-toggle-tooltip = Clique para alternar entre afinações diatônicas e layouts cromáticos de 12 ou 16 buracos
editor-content-kind-toggle-tooltip = Clique para alternar entre gravar uma música comum e uma lição do currículo
editor-snap-mode-toggle-tooltip = Clique para alternar a subdivisão de tempo em que um clique na grade se encaixa — semicolcheias retas, colcheias shuffle (swingadas) ou tercinas de colcheia retas
editor-lesson-form-tooltip = Campos do currículo para lesson.json — usados apenas enquanto "Gravar Lição" está ativo
editor-lesson-details-toggle-tooltip = Clique para mostrar ou ocultar os campos do currículo da lição
editor-field-lesson-pass-criteria-tooltip = Clique para alternar como esta lição é avaliada — Nenhum, Precisão, Técnica, Aderência à Escala, Aderência a Notas do Acorde, Disciplina de Frase
editor-field-lesson-technique-tooltip = Clique para alternar qual técnica é avaliada — usado apenas quando o Critério de Aprovação é Técnica
editor-field-lesson-progression-tooltip = Clique para alternar a progressão de acompanhamento de uma lição baseada em jam — Nenhuma, Padrão, Quick-Change, Menor
editor-field-lesson-scale-tooltip = Clique para alternar a escala usada para avaliar uma lição de jam
editor-field-lesson-path-tooltip = Clique para escolher se esta lição é obrigatória ou uma ramificação eletiva
editor-field-key-tooltip = Clique para alternar entre os tons da gaita
editor-field-position-tooltip = Clique para alternar entre as posições de execução
editor-browse-tooltip = Escolher um arquivo de áudio de música de fundo para esta partitura
editor-import-midi-tooltip = Carregar um arquivo MIDI e escolher uma faixa para colocar na grade de notas — Salvar então grava uma trilha de fundo a partir das outras faixas
editor-silence-track-label = Silêncio
editor-silence-track-tooltip = O intervalo, em segundos, entre cada par de notas consecutivas

# Lições — menu, leitor, veredito nos resultados
menu-lessons = Lições
no-lessons-found = Nenhuma lição encontrada. Adicione pastas em assets/lessons/<unidade>/<lição>/
lesson-passed = Concluída
lesson-start = Começar a Lição
lesson-widget-metronome-toggle = Iniciar / Parar
lesson-widget-tempo-decrease = − 5 BPM
lesson-widget-tempo-increase = + 5 BPM
lesson-widget-feel-toggle = Reto / Shuffle / Tercinas
lesson-widget-sound-toggle = Som ligado / desligado
lesson-widget-key-previous = Tom anterior
lesson-widget-key-next = Próximo tom
lesson-widget-bar-previous = Compasso anterior
lesson-widget-bar-next = Próximo compasso
lesson-widget-bar-reset = Voltar ao compasso 1
lesson-widget-section-previous = Seção anterior
lesson-widget-section-next = Próxima seção
lesson-widget-section-reset = Voltar à seção 1
lesson-widget-step-previous = Passo anterior
lesson-widget-step-next = Próximo passo
lesson-widget-step-reset = Voltar ao passo 1
lesson-mark-done = Marcar como Concluída
lesson-goal-accuracy = Meta: {$pct}% de precisão geral
lesson-goal-technique = Meta: {$pct}% de precisão nas notas de {$technique}
lesson-goal-finish = Meta: tocar até o fim
lesson-goal-scale-adherence = Meta: {$pct}% das notas dentro da escala ou melhor
lesson-goal-chord-tone-adherence = Meta: {$pct}% das notas como notas do acorde
lesson-goal-phrase-discipline = Meta: {$pct}% das notas tocadas fora de uma pausa — deixe espaço
lesson-complete-banner = LIÇÃO CONCLUÍDA
lesson-failed-banner = Meta não atingida — releia a lição e tente de novo

# Jogo — contagem regressiva, legenda, dicas do diagrama da harmônica
gameplay-get-ready = PREPARE-SE
gameplay-legend-blow = ■ SOPRO
gameplay-legend-draw = ■ SUGADA
harmonica-overlay-hint-view = Harmônica  ·  acende conforme você toca
harmonica-overlay-hint-select = Harmônica  ·  clique numa nota, ou foque o diagrama e use as setas
gameplay-chart-info = Tom: {$key}  ♩ = {$bpm}  {$time_sig}
gameplay-chart-author = Partitura: {$author}
gameplay-techniques-toggle = {$arrow} TÉCNICAS

# Gameplay — the judgment label at the hit line, one per scoring outcome
gameplay-judgment-perfect = PERFEITO!
gameplay-judgment-good = BOM
gameplay-judgment-early = ADIANTADO
gameplay-judgment-late = ATRASADO
gameplay-judgment-no-attack = ERROU
gameplay-judgment-wrong-pitch = NOTA ERRADA
gameplay-judgment-wrong-pitch-detail = esperado {$expected}  ·  ouvido {$heard}
gameplay-judgment-wrong-pitch-detail-unplaceable = esperado {$expected}
gameplay-judgment-incomplete-chord = ACORDE INCOMPLETO
gameplay-judgment-technique = TÉCNICA

# Menu de pausa
# Gameplay — live badges for whichever practice aids are on
gameplay-badge-speed = {$pct}% de velocidade · sem música
gameplay-badge-wait = esperando cada nota
gameplay-badge-loop = loop {$start}s–{$end}s

# Gameplay — the wait-for-note coaching card at the hit line
gameplay-wait-play = Toque {$tab}
gameplay-wait-hearing = ouvindo {$tab}
gameplay-wait-listening = escutando…

pause-group-playback-aids = AJUDAS DE REPRODUÇÃO
pause-group-phrase-practice = PRÁTICA DE TRECHO
pause-quit-song = Sair da música
pause-paused = PAUSADO
pause-resume = Continuar
pause-restart = Recomeçar
pause-learned-label = Aprendido:
pause-finish-lesson = Concluir lição
pause-wait-for-note-button = ⏸ Esperar nota
pause-wait-for-note-on = Esperar nota: ligado
pause-wait-for-note-off = Esperar nota: desligado
pause-speed = Velocidade: {$pct}%
pause-adaptive-difficulty-button = Dificuldade adaptativa
pause-adaptive-difficulty-on = Dificuldade adaptativa: ligada
pause-adaptive-difficulty-off = Dificuldade adaptativa: desligada
pause-phrase-section = Seção: {$name} — Aprendido: {$pct}%
pause-phrase-no-sections = Nenhuma frase nesta música
pause-drag-section-hint = Clique numa seção na barra de progresso acima para selecioná-la
pause-notes-update-hint = As notas são atualizadas ao vivo — retome para vê-las
pause-clear-loop = Limpar repetição
pause-loop-off = Repetição: desligada
pause-loop-range = Repetição: {$start}s–{$end}s
pause-drag-loop-hint = Arraste na barra de progresso acima para definir um intervalo de repetição

# Overlay do metrônomo
metronome-click-off = clique: desligado
metronome-click-on = clique: ligado
metronome-feel-straight = ritmo: reto
metronome-feel-shuffle = ritmo: shuffle

# Treinador de Bends
bending-drill-off = Exercício: desligado
bending-drill-on = Exercício: ligado · sequência {$streak}
bending-hint = Esc para voltar  ·  M silencia o clique  ·  feel alterna reto/shuffle
bending-drill-explanation = Escolhe alvos do escopo atual, preferindo os que você ainda não tentou, controla com menos segurança ou não pratica há algum tempo. Complete a forma de prática para avançar; Pular, ou deixar a gaita em silêncio, avança sem contar contra você.
bending-no-note-for-technique = Este furo não tem nota para essa técnica.
bending-key-label = Tom
bending-listen-button = 🔊 Ouvir
bending-listen-natural-button = 🔊 Natural
bending-listen-target-button = 🔊 Alvo
bending-drill-button = 🎲 Exercício
bending-adv-toggle = Avançado
bending-adv-reset = Redefinir
bending-adv-tolerance = Tolerância
bending-adv-hold = Sustentar para passar
bending-adv-timeout = Tempo da tentativa
bending-adv-a4 = Referência A4
bending-adv-trace = Duração do traço
bending-adv-smoothing = Suavização do traço
bending-adv-subdivision = Pulso por tempo
bending-adv-stability-heading = Esta tentativa
bending-adv-mean = Média: {$value}
bending-adv-spread = Dispersão: {$value}
bending-adv-best-hold = Melhor sustentação: {$value}
bending-adv-vibrato = Vibrato: {$value}
bending-adv-center = Palheta medida: {$value}
bending-adv-clear-center = Esquecer palheta medida
bending-setup-button = Configuração
bending-setup-summary = Gaita em {$key} · {$algo}
bending-skip-button = Pular
bending-progress-none = Ainda não praticado
bending-progress = {$hits} de {$attempts} controlados
bending-scope-button = Escopo
bending-scope-status = Escopo: {$scope}
bending-scope-first = Primeiros bends
bending-scope-all = Todos os bends
bending-scope-blow = Bends soprados
bending-scope-over = Overbends
bending-scope-custom-selected = Personalizado (célula selecionada)
bending-scope-custom-count = Personalizado ({$count} células)
bending-shape-button = Prática
bending-shape-free = Exploração livre
bending-shape-find-hold = Encontrar e sustentar
bending-shape-bend-release = Bend e retorno
bending-shape-repeated = Bends repetidos
bending-shape-ladder = Escada de bends
bending-shape-overbend-response = Resposta de overbend
bending-shape-status = {$shape} · {$phase}
bending-phase-waiting = aguardando
bending-phase-travel = vá ao alvo
bending-phase-holding = sustente
bending-phase-returning = volte ao natural
bending-phase-complete = completo
bending-play-it-target = Toque — alvo {$note}
bending-wrong-pitch = Ouvindo {$note} — toque o furo {$hole} selecionado
bending-signal-unstable = Sinal instável — mantenha a nota firme
bending-rail-natural = Natural
bending-rail-target = Alvo
bending-rail-natural-note = Natural {$note}
bending-rail-target-note = Alvo {$note}
bending-metric-distance = Distância {$value}
bending-metric-stability = Estabilidade {$value}
bending-metric-hold = Sustentação {$value}
bending-check-natural-button = Verificar nota natural
bending-check-natural-idle = Opcional: verifique a nota natural {$note} antes do bend
bending-check-natural-listening = Segure a nota natural {$note} com estabilidade…
bending-check-natural-ready = ✓ A nota natural {$note} está sendo detectada claramente
bending-in-tune = ✓ Afinado  ({$note})
bending-cents-sharp = ↑ {$cents} cents agudo  (alvo {$note})
bending-cents-flat = ↓ {$cents} cents grave  (alvo {$note})
bending-detect-label = Detectar
bending-tempo-decrease = Diminuir andamento
bending-tempo-increase = Aumentar andamento
bending-target-label = Alvo: Furo {$hole} · {$technique}
bending-technique-blow = Sopro
bending-technique-draw = Aspiração
bending-technique-bend-half = Bend de ½ tom
bending-technique-bend-whole = Bend de 1 tom
bending-technique-bend-three-half = Bend de 1½ tom
bending-technique-overblow = Overblow
bending-technique-overdraw = Overdraw
bending-technique-hint-blow = Sopre de forma estável pelo furo com pressão suave e relaxada.
bending-technique-hint-draw = Aspire de forma estável pelo furo com pressão suave e relaxada.
bending-technique-hint-draw-bend-half = Aspire suavemente e mova a parte de trás da língua para baixar meio tom. Molde o ar; não aspire com mais força.
bending-technique-hint-draw-bend-whole = Aspire suavemente e mova mais a parte de trás da língua para baixar um tom. Molde o ar; não aspire com mais força.
bending-technique-hint-draw-bend-three-half = Aspire suavemente e aprofunde a posição da língua para baixar um tom e meio. Molde o ar; não aspire com mais força.
bending-technique-hint-blow-bend-half = Sopre suavemente e ajuste a parte de trás da língua para baixar meio tom. Molde o ar; não sopre com mais força.
bending-technique-hint-blow-bend-whole = Sopre suavemente e mova mais a língua para baixar um tom. Molde o ar; não sopre com mais força.
bending-technique-hint-blow-bend-three-half = Sopre suavemente e aprofunde a posição da língua para baixar um tom e meio. Molde o ar; não sopre com mais força.
bending-technique-hint-overblow = Comece com um sopro suave e estreite a cavidade oral até a palheta de sopro fechar e a de aspiração soar. Embocaduras de bico e tongue block podem funcionar; evite força.
bending-technique-hint-overdraw = Comece com uma aspiração suave e estreite a cavidade oral até a palheta de aspiração fechar e a de sopro soar. Embocaduras de bico e tongue block podem funcionar; evite força.
bending-technique-hint-over-unsupported = O furo {$hole} não permite overblow nem overdraw neste layout.

# Jam Session
jam-loop-button = ↻ Loop
jam-loop-off = Loop: desligado
jam-loop-on = Loop: ligado
jam-end-after-chorus-button = Terminar depois deste chorus
jam-keep-playing = Continuar tocando
jam-ending-after-chorus = Terminando depois deste chorus…
jam-ended = Terminou — reinicie ou saia
jam-form-position = Chorus {$chorus} · Compasso {$bar}
jam-hole-map-hint = Sua harmônica  ·  dourado = tom do acorde agora  ·  verde = nota da escala de blues  ·  sopro em cima / sugada embaixo
jam-call-response-button = ⇄ Pergunta & Resposta
jam-call-response-off = Pergunta & Resposta: desligado
jam-call-response-on = Pergunta & Resposta: ligado
jam-call-response-listen = Escute…
jam-call-response-your-turn = Sua vez
jam-call-density-button = Fraseado
jam-call-density-sparse = Fraseado: esparso
jam-call-density-conversational = Fraseado: conversado
jam-call-density-busy = Fraseado: cheio
jam-adaptive-band-button = Banda adaptativa
jam-adaptive-band-on = Banda adaptativa: ligada
jam-adaptive-band-off = Banda adaptativa: desligada
jam-detected-blow = Furo {$hole} sopro
jam-detected-draw = Furo {$hole} sucção
jam-detected-none = —
jam-midi-track-mute-tooltip = Clique para silenciar/ativar esta faixa
jam-rhythm-guide = Guia de Ritmo
jam-guides-button = Guias
jam-guides-off = Guias: ocultos
jam-guides-on = Guias: visíveis
jam-position-label = Posição: {$position}
jam-spectrogram-style-button = ↻ Visualização
jam-spectrogram-style-bars = Barras
jam-spectrogram-style-oscilloscope = Osciloscópio

# Tela de resultados
results-song-complete = MÚSICA CONCLUÍDA
results-by-technique = Por técnica
results-new-best = ◆ NOVO RECORDE! ◆
results-biggest-combo = Maior combo
results-perfect-hits = Acertos perfeitos
results-good-hits = Bons acertos
results-delayed-hits = Acertos atrasados
results-misses = Erros
results-technique-normal = Notas normais
results-technique-bend = Bends
results-technique-vibrato = Vibrato
results-technique-wah = Wah
results-technique-overblow = Overblow
results-technique-overdraw = Overdraw
results-technique-slide = Slide
results-technique-clean-attack = Ataque limpo
results-increase-latency = Aumentar a latência de entrada para {$ms}ms
results-decrease-latency = Diminuir a latência de entrada para {$ms}ms
results-score = Pontuação: {$points}
results-best-score = Melhor pontuação
results-accuracy-caption = de precisão
results-observation-technique = {$technique}: {$hits} de {$total} acertaram — é aí que estão os próximos pontos
results-observation-missed = {$misses} de {$total} notas nem soaram — desacelere com a Velocidade de treino ou o Esperar a nota
results-observation-late = {$pct}% dos seus acertos vieram atrasados — antecipe a linha de acerto, ou aplique o ajuste de tempo abaixo
results-observation-early = {$pct}% dos seus acertos vieram adiantados — deixe a nota chegar à linha, ou aplique o ajuste de tempo abaixo
results-observation-leaky = Só {$clean} de {$total} ataques foram limpos — um furo vizinho está vazando; feche mais a embocadura
results-observation-solid = Nada se destaca — pode partir para uma música mais difícil
results-timing = Tempo (média {$ms}ms)
results-timing-early = adiantados {$n}
results-timing-on-time = no tempo {$n}
results-timing-late = atrasados {$n}
results-lesson-reached = Nesta vez: {$pct}%
results-retry = Repetir
results-practice-missed = Treinar o trecho errado
results-continue = Continuar

# Calibração de latência
calibration-title = Calibração de Latência
calibration-mic-label = Mic
calibration-instructions = Toque qualquer nota em cada batida — o jogo mede o atraso com que o microfone detecta o som.
calibration-mean-offset-placeholder = Deslocamento médio: —
calibration-mean-offset = Deslocamento médio: {$sign}{$ms}ms
calibration-suggested-placeholder = Atual: —   →   Sugerido: —
calibration-suggested = Atual: {$current}ms   →   Sugerido: {$suggested}ms
calibration-get-ready = Prepare-se…
calibration-hits-recorded = {$hits} / {$total} toques registrados
calibration-complete = Calibração concluída!
calibration-start = Começar
calibration-apply = Aplicar
calibration-try-again = Tentar de novo
calibration-cancel = ← Cancelar

# Opções
options-input-lag = Atraso de entrada
options-input-lag-tooltip = Adianta/atrasa as notas detectadas para bater com o atraso de áudio do seu equipamento.

# Tour guiado do tutorial (menu::tutorial)
tutorial-step = Passo {$n} de {$total}
tutorial-skip = Pular Tutorial
tutorial-title-main = Menu Principal
tutorial-body-main = Sua base — vá para Jogar, abra as Opções ou encontre Ajuda / Sobre por aqui.
tutorial-title-play = Jogar
tutorial-body-play = Escolha uma música de verdade, crie uma, comece uma jam, pratique bends ou siga as lições — escolha como quer jogar.
tutorial-title-mode-select = Seletor de músicas
tutorial-body-mode-select = Navegue por todas as músicas, ordene por banda, dificuldade, nome ou gênero, pesquise e escolha 2D ou 3D nesta tela.
tutorial-title-gameplay = Tocando uma Música
tutorial-body-gameplay = As notas caem em direção à linha de acerto — toque a nota certa na harmônica no momento certo para pontuar.
tutorial-title-jam-session-menu = Jam Session
tutorial-body-jam-session-menu = Escolha uma música de verdade para improvisar, ou gere uma base instantânea.
tutorial-title-jam-session = Jam Session
tutorial-body-jam-session = Jogo livre: a grade de 12 compassos e um mapa de furos ao vivo guiam sua improvisação — nada aqui é pontuado.
tutorial-title-bending-trainer = Treinador de Bends
tutorial-body-bending-trainer = Pratique bends isoladamente: escolha um alvo no diagrama, ouça-o e tente igualá-lo.
tutorial-title-options = Opções
tutorial-body-options = Volume, estilo das notas, modelo de harmônica e calibração do microfone ficam aqui.
tutorial-title-theme = Tema
tutorial-body-theme = Escolha um tema visual para os menus — troca fundos e o estilo dos botões.
tutorial-title-lessons = Lições
tutorial-body-lessons = Um currículo guiado: notas únicas, acordes, bends e improviso sobre o blues.
tutorial-title-jam-generate = Gerar Jam
tutorial-body-jam-generate = Gere uma base instantânea em qualquer tom e andamento — sem precisar de uma música.
tutorial-title-song-editor = Editor de Canções
tutorial-body-song-editor = Monte ou edite uma partitura nesta grade, depois toque-a ou pratique junto com ela ao vivo.
tutorial-title-help-about = Ajuda / Sobre
tutorial-body-help-about = Abra a documentação, leia sobre o Harmonicon, refaça este tour ou veja os créditos.

editor-tab-chart = Partitura
editor-tab-details = Detalhes
# Saudação de primeira execução (menu::pages::welcome) — exibida uma única
# vez, quando ainda não existe profile.json.
welcome-title = Bem-vindo ao Harmonicon
welcome-body = Você toca uma gaita de verdade no seu microfone, e o Harmonicon escuta e pontua enquanto as notas passam. Nada funciona até que ele consiga te ouvir, então comece por aí. Uma gaita diatônica em Dó serve para quase tudo aqui.
welcome-setup-mic = Configurar o microfone
welcome-tour = Fazer o tour guiado
welcome-lessons = Começar com uma lição
help-first-run = Primeiros passos
welcome-skip = Pular por enquanto
# Problemas de microfone. `mic-warning-*` é o aviso durante o jogo
# (gameplay::mic_warning_overlay), curto e sem o erro bruto do dispositivo;
# `options-mic-*` é o aviso nas Opções, onde os detalhes é que interessam.
mic-warning-failed = Sem microfone — nada do que você tocar será pontuado. Veja as Opções.
mic-warning-permission = Aguardando permissão do microfone.
options-mic-failed = Sem microfone: {$reason}
options-mic-awaiting-permission = Aguardando permissão do microfone — conceda e tente novamente
# Exibido no seletor ao lado de um detector que só resolve uma nota por vez,
# já que escolhê-lo torna impossível acertar qualquer acorde da partitura.
algo-single-notes-only = só notas isoladas
# Aviso durante o jogo para essa combinação (gameplay::warning_banner).
chord-warning-monophonic = Esta música tem acordes, e o detector de tom escolhido ouve uma nota por vez. Escolha FFT ou NMF nas Opções para pontuá-los.
# "Qual gaita você tem?" (menu::pages::harp_check) — entre escolher a
# música e carregá-la.
harp-check-title = Sua gaita
harp-check-intro = Se a sua for outra, informe aqui e escolha o que a troca deve preservar.
harp-check-key = Tom
harp-check-type = Tipo
harp-check-mapping = Quando a gaita for diferente
harp-check-same-holes = Mesmos furos (a música muda de tom)
harp-check-transpose = Mesma música (os furos mudam)
harp-check-play = Tocar
harp-check-cost-clean = Toca como está escrito nesta gaita.
harp-check-cost-bends = {$count} nota(s) exigem bend
harp-check-cost-overblows = {$count} nota(s) exigem overblow
harp-check-cost-unreachable = {$count} nota(s) não podem ser tocadas nesta gaita
# Nomeia a gaita para a qual a música foi escrita, na página de escolha.
harp-check-chart-harp = Escrita para uma gaita {$kind} em {$key}.
harp-kind-diatonic = diatônica
harp-kind-chromatic = cromática
harp-summary-diatonic = Diatônica
harp-summary-chromatic = Cromática
harp-summary-holes = {$n} furos
harp-summary-position = {$pos} posição
harp-banner-use = Use uma gaita em {$key}
harp-banner-key = tom de {$key}
harp-banner-fallback = Tocando em {$key}
harp-row-blow = sopro
harp-row-draw = sugada
harp-row-overblow = overblow
harp-row-overdraw = overdraw
harp-row-slide = slide
# O seletor de faixa na página de verificação da gaita, exibido apenas para
# um arquivo importado com mais de uma parte tocável.
harp-check-track = Parte a tocar
harp-check-track-option = {$name} — {$notes} notas, {$percent}% tocável
harp-check-track-unnamed = Faixa {$index}
# Cabeçalho acima dos cinco níveis de treino de uma lição, com quanto da
# escada foi concluído (o medidor de domínio).
lesson-training-heading = Treino — {$percent}% dominado
# Os cinco níveis de treino: o nome de cada um e o que ele pede do jogador.
lesson-training-tier-isolate = Isolar
lesson-training-tier-isolate-about = A técnica sozinha, devagar, em um só furo.
lesson-training-tier-consolidate = Consolidar
lesson-training-tier-consolidate-about = O mesmo exercício, um pouco mais rápido.
lesson-training-tier-vary = Variar
lesson-training-tier-vary-about = Todos os furos da lição, em todas as profundidades que alcançam.
lesson-training-tier-in-context = Em Contexto
lesson-training-tier-in-context-about = A técnica dentro de uma frase musical, em colcheias.
lesson-training-tier-interleave = Intercalar
lesson-training-tier-interleave-about = Uma ordem imprevisível, para não dar para tocar de memória.
# A meta de um nível de treino: a linha de meta da lição, depois o andamento e a duração do nível.
lesson-training-goal = {$goal}, a {$bpm} BPM por {$bars} compassos
lesson-training-start = Começar Treino

# The skill-tree view of the curriculum: one row per track.
lesson-tree-title = Árvore de Habilidades
lesson-tree-find-next = Encontrar próxima lição
lesson-tree-broken = Este currículo não pode ser desenhado: {$error}
lesson-tree-unit-progress = {$done}/{$needed}
# O medidor de domínio de uma trilha acima da árvore: quanto das escadas de treino foi concluído.
lesson-tree-track-mastery = {$track} — {$percent}%
# A fila de revisão de aquecimento acima da árvore: o rótulo e um treino pendente como "lição · nível".
lesson-tree-warmup = Aquecimento:
lesson-tree-review-due = ↻ Revisão pendente: toque o nível mais alto de novo para manter a habilidade
lesson-tree-warmup-item = {$lesson} · {$tier}
# A sequência de prática acima da árvore, mostrada a partir de dois dias.
lesson-tree-streak = {$days} dias seguidos de prática
lesson-tree-needs = Precisa de: {$lessons}
lesson-tree-optional = Eletiva

# Scored play — technique cue beside a note head, and the technique coach row
cue-target = → {$note}
cue-vibrato = vib {$rate}/s
cue-wah = wah {$rate}/s
coach-follow-pulse = Acompanhe
coach-bend-more = Dobre mais
coach-on-target = Segure
coach-too-far = Passou
coach-swing-more = Mais amplo
coach-faster = Mais rápido
coach-slower = Mais lento
coach-on-rate = Bom
coach-rate = {$rate}/s
