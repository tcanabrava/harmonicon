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
select-mode = Selecionar Modo
play-2d = Tocar em 2D
play-3d = Tocar em 3D

# Gerar Jam (base sintetizada, sem precisar de uma música)
jam-generate-title = Gerar uma Base de Jam
jam-generate-start = Começar a Jam
jam-generate-key = Tom
jam-generate-tempo = Andamento
jam-generate-progression = Progressão
jam-generate-position = Posição
jam-generate-scale = Escala
jam-generate-genre = Gênero

# Créditos
credits-back-to-menu = Voltar ao Menu

# Seleção de música / artista
select-artist = Selecionar Artista
select-song = Selecionar Música
no-songs-found = Nenhuma música encontrada. Adicione pastas em assets/songs/<artista>/<música>/

# Opções
options-title = Opções
options-language = Idioma
options-adaptive-difficulty = Dificuldade Adaptativa
options-adaptive-difficulty-tooltip = Ajusta automaticamente quantas notas da música são mostradas de uma vez, de acordo com o seu desempenho.
options-fullscreen = Tela cheia
options-fullscreen-tooltip = Joga em tela cheia em vez de em uma janela.
options-colorblind-palette = Paleta para daltonismo
options-colorblind-palette-tooltip = Usa um par fixo de cores sopro/sucção seguro para daltonismo, em vez das cores de nota do tema atual.
options-zoom = Zoom
options-zoom-tooltip = Ajusta o tamanho de toda a interface.
options-zoom-label = Zoom: {$percent}%
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
options-calibrate-input-lag = Calibrar a latência de entrada
options-calibrate-input-lag-tooltip = Mede a latência de áudio do seu equipamento e a aplica automaticamente.
options-back-tooltip = Volta ao menu principal.
options-button-style = Botões de ação
options-button-style-tooltip = Como os botões de ação do Editor de Músicas mostram ícone e texto.
options-button-style-icon-only = Somente ícone
options-button-style-text-beside-icon = Texto ao lado do ícone
options-button-style-text-only = Somente texto
theme-back-to-options = ← Voltar às Opções

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
mod-delete = Apagar
editor-tool-select = Selecionar
editor-tool-erase = Apagar Trecho
editor-tool-remove = Remover Trecho
editor-tool-tempo = Tempo

# Editor de Músicas 2 — rótulos dos campos de metadados
editor-field-tempo = Andamento da Música
editor-field-time-signature = Fórmula de Compasso
editor-field-key = Tom do Gaita
editor-field-position = Posição
editor-field-harmonica = Gaita
editor-field-music = Música de Fundo
editor-field-name = Nome
editor-field-author = Autor
editor-field-midi-track = Faixa MIDI
editor-field-midi-track-tooltip = Qual faixa do arquivo MIDI importado colocar na grade.
editor-field-scale = Escala
editor-field-scale-tooltip = Contra qual escala o tom vermelho de "fora da escala" na grade é medido.
editor-field-text-tooltip = Clique para editar; digite um valor e depois clique fora ou pressione Enter para confirmar.
editor-harmonica-diatonic = ‹ Diatônica (10 buracos) ›
editor-harmonica-chromatic = ‹ Cromática (12 buracos) ›
editor-field-content-kind = Gravação
editor-content-kind-song = ‹ Gravar Música ›
editor-content-kind-lesson = ‹ Gravar Lição ›
editor-field-snap-mode = Encaixe da Grade
editor-snap-mode-sixteenth = ‹ Semicolcheias retas ›
editor-snap-mode-shuffle = ‹ Shuffle (colcheias swingadas) ›
editor-snap-mode-triplet = ‹ Tercinas de colcheia ›

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
mod-delete-tooltip = Apagar a nota selecionada
editor-tool-select-tooltip = Clique num ponto da linha do tempo e depois num lado (ou clique e arraste para selecionar um intervalo)
editor-tool-erase-tooltip = Clique num ponto da linha do tempo e depois num dos lados (ou clique e arraste um trecho) para apagar as notas dali, deixando um vazio
editor-tool-remove-tooltip = Clique num ponto da linha do tempo e depois num dos lados (ou clique e arraste um trecho) para apagar as notas dali e deslocar tudo depois pra frente, fechando o vazio
editor-tool-tempo-tooltip = Clique na régua para adicionar uma mudança de andamento ali, ou clique numa já existente para removê-la
editor-harmonica-toggle-tooltip = Clique para alternar entre gaita Diatônica e Cromática
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
lesson-mark-done = Marcar como Concluída
lesson-goal-accuracy = Meta: {$pct}% de precisão geral
lesson-goal-technique = Meta: {$pct}% de precisão nas notas de {$technique}
lesson-goal-finish = Meta: tocar até o fim
lesson-goal-scale-adherence = Meta: {$pct}% das notas dentro da escala ou melhor
lesson-goal-chord-tone-adherence = Meta: {$pct}% das notas como notas do acorde
lesson-goal-phrase-discipline = Meta: {$pct}% das notas tocadas fora de uma pausa — deixe espaço
lesson-complete-banner = LIÇÃO CONCLUÍDA
lesson-failed-banner = Meta não atingida — releia a lição e tente de novo

# Lições — títulos das unidades (chave vem do campo "unit" de cada lesson.json)
lesson-unit-blowing = Unidade 1 · Soprando a Gaita
lesson-unit-rhythm = Unidade 2 · Contando o Blues
lesson-unit-blues = Unidade 3 · Vocabulário do Blues
lesson-unit-scales = Unidade 4 · Escalas e Improvisação
lesson-unit-jazz = Unidade 5 · Jazz
lesson-unit-navigation = Unidade 6 · Encontrando o Caminho

# Lição: nota única
lesson-single-note-title = Tocando uma Nota Só
lesson-single-note-body =
    O maior obstáculo do iniciante na gaita: tirar uma nota limpa em vez de um acorde de vizinhas.
    Faça bico como se fosse assobiar, ou diga a sílaba "tu" — a abertura deve ser pouco maior que um furo.
    Relaxe: a gaita entra fundo entre os lábios, apoiada na parte interna úmida, não presa pela borda seca.
    Incline o fundo da gaita levemente para cima e deixe o queixo cair, para o ar sair lento e quente, vindo da barriga.
    Neste exercício, notas longas nos furos 4, 5 e 6 deslizam até a linha de acerto. Sopre cada uma com calma — não importa o volume, importa a pureza.
    Se ouvir duas notas ao mesmo tempo, não force: estreite um pouco a abertura e desacelere o sopro.

# Lição: várias notas (acordes)
lesson-multiple-notes-title = Tocando Várias Notas Juntas
lesson-multiple-notes-body =
    Uma nota só não é o único alvo — algumas levadas do blues soam de propósito dois ou três furos juntos, como um acorde.
    Alargue a embocadura para cobrir os furos que quer e nenhum além deles; o mesmo controle de sopro que deu uma nota limpa agora dá um grupo controlado delas.
    Acordes de sopro ficam em furos vizinhos: os furos 1-2-3 soprados juntos soam um alegre acorde de Dó maior.
    Acordes de sugado funcionam do mesmo jeito: os furos 2-3-4 sugados juntos soam um acorde de Sol maior.
    Neste exercício, as levadas de acorde deslizam até a linha de acerto — o jogo escuta cada nota do acorde soando no mesmo instante, não uma depois da outra.
    Se só parte do acorde registrar, você provavelmente não está cobrindo todos os furos por igual; alargue a embocadura em vez de soprar mais forte.

# Lição: bloqueio de língua (instrucional)
lesson-tongue-blocking-title = Bloqueio de Língua
lesson-tongue-blocking-body =
    Até agora você deu forma às notas com os lábios (fazendo bico) — o bloqueio de língua é a outra embocadura clássica: cubra vários furos com a boca, e apoie a língua achatada na gaita para bloquear todos menos um.
    Tire a língua de um furo e ele soa sozinho, exatamente como uma nota única de bico — o microfone não consegue mesmo distinguir as duas técnicas, então esta lição não pode verificar qual você está usando.
    O que o bloqueio de língua destrava e o bico não consegue: afaste a língua de dois furos das pontas ao mesmo tempo (bloqueando só os do meio) e você ganha uma divisão de oitava — duas notas, uma oitava de distância, soando juntas.
    Também deixa você bater a língua contra um furo em ritmo para um pulso percussivo tipo "tchaca-tchaca", e trocar de canto da boca no meio de uma frase sem perder a vedação do ar.
    Tente a lição de divisão de oitava a seguir — é a recompensa concreta e mensurável dessa técnica: o jogo consegue ouvir se as duas notas da divisão soam juntas, mesmo sem conseguir ouvir o bloqueio de língua em si.

# Lição: divisão de oitava (bloqueio de língua)
lesson-octave-split-title = Divisão de Oitava
lesson-octave-split-body =
    O bloqueio de língua deixa tocar dois furos ao mesmo tempo, abafando os furos entre eles — o clássico é a divisão de oitava.
    Apoie a língua achatada sobre a gaita, cobrindo os dois furos do meio, e deixe o ar passar só pelo furo de cada lado.
    Nos furos 1 e 4 soprados juntos você ouve Dó4 e Dó5 — a mesma nota, uma oitava acima. Os furos 2 e 5, e os furos 3 e 6, funcionam do mesmo jeito.
    Neste exercício, os dois furos de cada divisão precisam soar juntos, igual a um acorde — o bloqueio de língua em si não dá pra verificar pelo microfone, mas a oitava que ele produz dá.
    Se só ouvir uma nota, confira se a língua está cobrindo os furos do meio por inteiro, em vez de ficar torta para um lado.

# Lição: slides
lesson-slides-title = Slides
lesson-slides-body =
    Duas técnicas diferentes dividem o nome "slide" na gaita — este exercício cobre as duas.
    A primeira é um slide físico: mova a gaita de lado pela embocadura, de um furo pro outro, mantendo a vedação sem quebrar em vez de parar e recomeçar o sopro a cada nota. Neste exercício, deslize suavemente pelos furos 4-5-6 soprados — o jogo escuta três notas comuns, mas a técnica está em como você conecta elas, não só em acertá-las.
    A segunda é a liberação de bend: ataque uma nota já dobrada pra baixo, e deixe ela subir suavemente até a nota natural — um choro clássico do blues. Neste exercício, dobre o furo 2 sugado meio tom pra baixo e segure, depois libere suavemente até a nota natural; o jogo valida a nota dobrada no momento em que você a toca.
    Mantenha o ar constante nos dois — o slide deve soar como um sopro contínuo só, não uma série de ataques separados.

# Lição: formato das mãos / wah
lesson-hand-wah-title = O Formato das Mãos e o Wah
lesson-hand-wah-body =
    Suas mãos são o controle de timbre da gaita. Feche-as em concha atrás do instrumento formando uma câmara de ar vedada, e abra e feche a vedação para ela "falar": "uá".
    Segure a gaita entre o polegar e o indicador de uma mão, e vede a outra mão atrás, como uma concha.
    Concha fechada = som escuro, abafado. Concha aberta = claro e forte. Abrir a concha em ritmo enquanto a nota soa produz o clássico wah-wah.
    Neste exercício, sustente cada nota com firmeza e abra-e-feche a concha cerca de duas vezes por segundo — o jogo escuta esse pulso no seu som.
    Mantenha o sopro constante; só as mãos se movem. Se nada registrar, aperte a vedação da concha — quase todo o efeito mora no último centímetro do fechamento.

# Lição: respiração
lesson-breathing-title = Respiração e Notas Longas
lesson-breathing-body =
    Notas longas e estáveis são a base de tudo o que vem depois — antes de bends, vibrato ou licks rápidos, seu ar precisa estar calmo e controlado.
    Respire pelo diafragma, não pelo peito: deixe a barriga expandir ao inspirar, e mantenha os ombros relaxados e parados.
    Neste exercício, os furos 1 a 4 (sopro e sugada) sustentam por três a quatro tempos cada — respire através da gaita, não empurre o ar.
    Uma nota trêmula ou vazando não pontua como limpa; uma nota firme e estável pontua, mesmo em volume baixo.
    Se o ar acabar no meio da nota, você está usando mais do que precisa — relaxe e deixe a gaita fazer menos esforço por mais som.

# Lição: primeiro bend
lesson-first-bend-title = Seu Primeiro Bend: 4 Sugado
lesson-first-bend-body =
    O bend de meio-tom no 4 sugado é o clássico primeiro bend que todo gaitista aprende — abaixe levemente a língua e o queixo enquanto suga, como se dissesse "iii" deslizando para "óoo".
    Não trave a garganta; o bend vem do formato dentro da boca, não de apertar mais forte.
    Neste exercício, o 4 sugado normal alterna com o 4 sugado com bend — escute a altura caindo meio tom cada vez que você faz o bend.
    Pratique no Treinador de Bends (no menu Jogar) se quiser ouvir a nota-alvo e conferir sua afinação antes de voltar aqui.
    Um bend impreciso ainda conta se chegar perto o suficiente — a precisão vem com a prática, então não busque a perfeição no primeiro dia.

# Lição: bends profundos
lesson-deep-bends-title = Bends Profundos: 2 e 3 Sugado
lesson-deep-bends-body =
    Os furos 2 e 3 sugados são onde o blues de 2ª posição realmente vive — os dois podem baixar mais que o furo 4, meio tom e um tom inteiro.
    Quanto mais fundo o bend, mais para trás a língua e o queixo precisam ir — pense na vogal deslizando de "i" pra "o" pra "u".
    Neste exercício, o 2 sugado faz meio tom e depois um tom inteiro, e o 3 sugado faz o mesmo — escute duas alturas distintas abaixo da nota natural em cada furo.
    Essas são as duas notas mais expressivas de toda a gaita — as notas "azuis" que dão a voz do instrumento.
    Se o bend de tom inteiro não descer o suficiente, não force com pressão — relaxe ainda mais a garganta; a tensão sufoca o bend em vez de aprofundá-lo.

# Lição: vibrato
lesson-vibrato-title = Vibrato
lesson-vibrato-body =
    O vibrato adiciona uma pequena oscilação a uma nota sustentada — um leve movimento de altura ou volume que faz uma nota longa soar viva em vez de parada.
    A fonte clássica é o diafragma: um pulso suave "rá-rá-rá" na respiração, o mesmo músculo do exercício de respiração.
    Neste exercício, sustente cada nota com firmeza e deixe um pulso lento (cerca de quatro a cinco vezes por segundo) ondular por ela — o jogo escuta essa oscilação.
    Rápido demais soa como um tremor; lento demais soa como notas separadas. Busque uma onda suave e uniforme.
    Se nada registrar, exagere o pulso mais do que parece natural no início — você sempre pode suavizar depois que o microfone confirmar que ele está lá.

# Lição: articulação
lesson-articulation-title = Articulação: Tonguing Ta-Ka
lesson-articulation-body =
    Tonguing é como você separa notas com clareza sem mover o sopro ou a embocadura — diga "ta" ou "ka" com a língua em cada nota nova, como apertar um interruptor.
    "Ta-ka" alterna a frente e o fundo da língua, permitindo articular notas repetidas rápidas sem cansar o ar.
    Neste exercício, o mesmo furo se repete em colcheias constantes — o jogo não escuta sua língua diretamente, mas uma sequência de notas presas, sem tonguing, só pontua o primeiro ataque. Rearticular cada uma é o que faz o resto valer.
    Comece devagar e exagerado; a velocidade vem depois, a clareza vem primeiro.
    Se suas notas embaçarem numa nota longa só na pista, você não está parando o ar completamente entre elas — uma língua mais firme resolve.

# Lição: chamada e resposta
lesson-call-response-title = Chamada e Resposta
lesson-call-response-body =
    Isso é chamada e resposta: o jogo toca uma frase curta, e depois é sua vez de tocar de volta.
    Escute a demonstração sintetizada — uma sequência de uma, duas, depois três notas — e repita exatamente o que ouviu, no seu próprio tempo; o jogo congela e espera por você, o quanto for preciso.
    Não tem pressa nem relógio correndo contra você aqui: só a nota importa, não o tempo.
    Se tocar a nota errada, não tem problema — o jogo só continua esperando até você acertar, então escute de novo na sua cabeça e tente outra vez.
    Essa é a mesma habilidade de "ouvir e tocar" que você vai usar numa jam com outros músicos: alguém toca uma frase, você responde.

# Lição: improvisação
lesson-improvisation-title = Improvisando sobre o Blues
lesson-improvisation-body =
    Agora é juntar tudo: a forma de 12 compassos, a escala de blues e suas próprias escolhas, tocadas ao vivo numa jam de verdade.
    Esta lição abre uma Jam Session normal — a grade de 12 compassos e o mapa de furos da sua gaita mudam de cor ao vivo enquanto você toca: dourado significa que você tocou uma nota do acorde que está soando agora, verde significa que você está dentro da escala de blues, âmbar significa que saiu dela.
    Isso é 2ª posição: sua gaita em Dó toca no tom de Sol, o clássico esquema cross-harp do blues — o furo 2 sugado é sua nota de base.
    Não há melodia fixa pra acertar; toque o que quiser sobre os acordes e deixe seu ouvido seguir a cor do mapa de furos.
    Quando sentir que está pronto pra parar, abra o menu de pausa e aperte "Finish Lesson" — o jogo conta quantas das suas notas caíram dentro da escala ou em cima de um acorde e julga o exercício por isso.
    Mire em verde e dourado na maior parte do tempo; uma nota âmbar de vez em quando é normal, até expressiva — só não more lá.

# Lição: lendo a grade de 12 compassos
lesson-twelve-bar-title = Lendo a Grade do Blues de 12 Compassos
lesson-twelve-bar-body =
    Quase toda música de blues segue o mesmo ciclo de 12 compassos — aprenda a lê-lo uma vez e você acompanha qualquer roda de blues do planeta.
    Cada célula da grade é um compasso de quatro tempos. Os algarismos romanos nomeiam os acordes: I é o acorde de casa, IV a viagem do meio, V a tensão do retorno.
    O desenho clássico: quatro compassos de I, dois de IV, dois de I, um de V, um de IV e dois compassos finais de I (o último costuma virar V para lançar o próximo chorus — o "turnaround").
    Conte em voz alta: "UM dois três quatro, DOIS dois três quatro..." — doze compassos, e o ciclo recomeça.
    Você verá essa grade ao vivo na Jam Session, onde o compasso atual acende conforme a base toca. Depois desta lição, abra uma Jam Session e apenas observe alguns ciclos passarem, contando junto, antes de tocar qualquer nota.

# Lição: usando os pés
lesson-using-your-feet-title = Usando os Pés
lesson-using-your-feet-body =
    Um bom senso de tempo não vem de olhar pra tela — vem do seu corpo. Bata o pé em cada tempo, e deixe esse pulso físico guiar sua tocada em vez de correr atrás das notas que deslizam na tela.
    Antes de começar, conte "1, 2, 3, 4" em voz alta algumas vezes no andamento do exercício, batendo o pé em cada número, até ficar automático em vez de contado.
    Neste exercício, um pulso constante de semínimas desliza no furo 4 — continue batendo o pé o tempo todo, mesmo entre as notas, e deixe cada sopro/sugada cair exatamente numa batida.
    A janela de tempo aqui é mais apertada que nos outros exercícios de propósito: esta lição é inteiramente sobre cair no tempo, não sobre nota ou técnica.
    Se estiver sempre adiantado ou atrasado, não olhe pra pista — feche os olhos e siga só o pé.

# Lição: contando de quatro
lesson-counting-four-title = Contando de Quatro
lesson-counting-four-body =
    Toda habilidade de ritmo daqui pra frente se apoia num único hábito: contar o tempo em voz alta, ou pelo menos na cabeça, enquanto você toca.
    Conte "1, 2, 3, 4" com firmeza junto do metrônomo antes de começar, e continue contando depois que as notas começarem — não pare de contar só porque está tocando.
    Neste exercício, uma nota cai em cada tempo, depois só nos tempos 1 e 3, depois só no tempo 1 — os espaços ficam maiores, mas sua contagem interna nunca deve pular.
    Se perder o tempo, não adivinhe — pare, reinicie a contagem do 1, e volte a entrar na próxima cabeça de compasso.
    Este é o hábito mais útil de todo esse currículo: tudo, da forma de 12 compassos ao turnaround, depende de sempre saber exatamente onde está o tempo 1.

# Lição: contando os compassos
lesson-bar-counting-title = Contando os Compassos
lesson-bar-counting-body =
    Agora conte compassos em vez de tempos: este exercício percorre a forma completa de 12 compassos, uma nota-raiz no tempo 1 de cada compasso, para você sentir as trocas de acorde chegarem sem precisar olhar.
    Isso é 2ª posição: sua gaita em Dó toca no tom de Sol, então o 2 sugado é a raiz do acorde I, o 4 soprado é a raiz do acorde IV, e o 4 sugado é a raiz do acorde V.
    Observe a grade de 12 compassos acender conforme cada compasso toca — combine o que você ouve e toca com o que vê, depois tente contar de olhos fechados.
    O desenho é quatro compassos de I, dois de IV, dois de I, um de V, um de IV, um de I e um de V — o mesmo formato de "Lendo a Grade do Blues de 12 Compassos".
    Se você cair na raiz errada, provavelmente perdeu a contagem em algum ponto do meio — o jeito de corrigir é sempre o mesmo: pare, reconte a partir do compasso 1 no próximo chorus.

# Lição: o turnaround
lesson-turnaround-title = O Turnaround
lesson-turnaround-body =
    O turnaround é os últimos dois compassos da forma de 12 compassos — o momento em que a música se inclina de volta para o topo do próximo chorus, e a parte que todo gaitista de blues precisa sentir chegando.
    Este exercício descansa quase a forma inteira de propósito: não há nada pra tocar até o compasso 12, então o único jeito de acertar é continuar contando em silêncio o caminho todo.
    Quando o compasso 12 chegar, toque a raiz do acorde V; depois, bem no topo do próximo chorus, toque a raiz do acorde I — isso é o turnaround resolvendo pra casa.
    Se você tocar no silêncio antes do compasso 12, perdeu a contagem em algum ponto anterior — não há nota pra perseguir ali, só o tempo pra manter.
    Essa é a mesma chegada que você vai precisar ouvir em jams de verdade: o turnaround costuma ser o único momento em que uma banda inteira se realinha junta.

# Lição: sensação de shuffle
lesson-shuffle-feel-title = Sensação de Shuffle
lesson-shuffle-feel-body =
    A maior parte do blues não fica em colcheias retas e uniformes — ele balança, com um bounce "shuffle" longo-curto no lugar.
    Diga "tam-TAM, tam-TAM" pra sentir a proporção: a primeira metade de cada par dura cerca do dobro da segunda.
    Esta partitura declara sensação de shuffle, então a batida do metrônomo balança junto das notas — escute a batida, não só as notas, pra travar no balanço.
    Neste exercício, pares longo-curto alternam sopro e sugada no furo 4 — encaixe a nota longa bem no tempo e deixe a nota curta saltar dela.
    Se seus pares saírem uniformes em vez de balançados, você provavelmente ainda está contando colcheias retas na cabeça — tente contar o shuffle como uma tercina, segurando os dois primeiros tempos juntos.

# Lição: chug do trem
lesson-train-chug-title = Trem: O Chug
lesson-train-chug-body =
    O chug é o clássico som de trem da gaita — e secretamente um exercício de ritmo e controle de respiração disfarçado.
    Alterne um acorde soprado e um acorde sugado nos furos 1-2-3, firme e uniforme, como uma locomotiva lenta ganhando vapor.
    Respire o ritmo em vez de usar a língua: deixe a própria respiração fazer "huft... puft... huft... puft", não uma língua batendo pra ligar e desligar.
    Neste exercício, o acorde alterna em colcheias constantes num andamento lento e paciente — todas as notas de cada acorde precisam soar juntas pra contar.
    Se só parte do acorde registrar, abra a embocadura de forma uniforme nos três furos em vez de apertar mais de um lado.

# Lição: trem rolando
lesson-train-rolling-title = Trem: Rolando
lesson-train-rolling-body =
    Agora o trem sai da estação: o mesmo chug que você acabou de aprender, mas acelerando gradualmente conforme ganha velocidade.
    Não corra atrás da velocidade — deixe ela crescer naturalmente, do mesmo jeito que um trem de verdade não pula direto pra velocidade máxima.
    Esta partitura é a primeira do currículo construída sobre um mapa de andamento em vez de um tempo fixo — as notas são posicionadas por tick, e a base realmente acelera embaixo de você.
    Continue respirando o padrão huff-puff da lição anterior; só o andamento muda, não o formato da sua respiração.
    Se você ficar pra trás conforme acelera, isso é normal nas primeiras tentativas — o objetivo é ficar solto, não rígido, enquanto o andamento muda.

# Lição: apito do trem
lesson-train-whistle-title = Trem: O Apito
lesson-train-whistle-body =
    Todo chug de trem precisa de um apito — um acorde longo e chorado de duas notas que corta através do ritmo do chug.
    O apito fica nos furos 4 e 5 sugados juntos, sustentado por bastante tempo, com um wah trabalhado nele — a mesma técnica de mão em concha da lição do wah.
    Neste exercício, choruses de chug alternam com um acorde de apito sustentado — mantenha o chug firme, depois abra pro apito e deixe sua mão fazer o "wah" enquanto você sustenta a nota.
    O apito precisa do acorde (duas notas soando juntas) e do pulso de wah ao mesmo tempo — se um deles falhar, confira se você está segurando os dois furos igualmente enquanto sua mão continua se movendo.
    Isso combina tudo dos exercícios de chug com a técnica de hand-wah — um bom sinal de que você está pronto pra levar os dois pra uma jam de verdade.

# Lição: escala de blues
lesson-blues-scale-title = A Escala de Blues
lesson-blues-scale-body =
    Sete notas, subindo e descendo: 2 sugado, 3 sugado bendado, 4 soprado, 4 sugado bendado, 4 sugado, 5 sugado, 6 soprado.
    Essa é a mesma escala de blues em 2ª posição de onde vem cada frase desta unidade — e a maior parte da gaita blues.
    Você já tem os dois bends do exercício de bends profundos; esta lição é sobre encadeá-los numa única forma que você toca sem pensar.
    Toque devagar no início, ouvindo como as notas bendadas se encaixam entre as naturais, em vez de substituí-las.
    Assim que essa escala ficar familiar sob seus dedos, tudo mais nesta unidade é só esse mesmo punhado de notas rearranjado.

# Lição: primeiras frases
lesson-first-licks-title = Primeiras Frases
lesson-first-licks-body =
    Três frases curtas, três notas cada, todas tiradas da escala de blues que você acabou de aprender — sem bends ainda.
    Cada uma toca como demonstração, depois espera você repeti-la de volta, exatamente como no exercício de pergunta-e-resposta.
    Não são só exercícios — são frases reais de blues, do tipo que você vai buscar por instinto assim que estiverem sob seus dedos.
    Leve o tempo que precisar em cada resposta; o jogo espera por você, então não há pressa pra chegar lá.
    Assim que conseguir tocar as três de memória, tente misturá-las numa jam session e veja como se sentem sobre os acordes.

# Lição: frases bendadas
lesson-bent-licks-title = Frases Bendadas
lesson-bent-licks-body =
    Agora as frases ganham voz: três frases construídas em torno dos bends de 3 e 4 sugado, as notas "choronas" da escala de blues.
    Cada uma toca como demonstração, depois espera você repeti-la — o mesmo padrão de pergunta-e-resposta da lição anterior, mas cada frase se apoia num bend.
    Preste atenção na diferença entre um bend limpo e um trêmulo; um bend firme e sustentado é o que dá caráter a essas frases.
    Se uma frase parecer fora de alcance, volte pro exercício de bends profundos por alguns minutos e retorne — geralmente o bend em si, não a frase, é o ponto difícil.
    Essas são as mesmas notas choronas que você vai ouvir em quase todo solo de gaita blues — fique confortável com elas aqui e elas vão aparecer em todo lugar.

# Lição: frases sobre os acordes
lesson-licks-over-changes-title = Frases Sobre os Acordes
lesson-licks-over-changes-body =
    Um chorus completo de 12 compassos, mas em vez de só raízes ou uma escala corrida, cada acorde ganha sua própria frase curta: uma forma sobre o acorde I, outra sobre o IV, outra sobre o V, e a virada pra fechar.
    Isso combina o exercício de contagem de compassos com suas novas frases — você precisa saber onde está na forma e ter a frase certa pronta pra ela.
    A sobreposição de frases marca cada linha de 4 compassos pra você ver a forma da estrutura enquanto toca.
    Se perder o fio, volte pra escala de blues que já conhece em vez de travar — acertar algo sobre o acorde certo é melhor que não tocar nada.
    Toque essa algumas vezes até as frases começarem a parecer que pertencem aos acordes sobre os quais estão, não só notas que você está recitando em ordem.

# Lição: improvisação sobre notas do acorde
lesson-chord-tone-improv-title = Improvisação Sobre Notas do Acorde
lesson-chord-tone-improv-body =
    O exercício de improvisação te julgou por ficar na escala de blues. Este eleva a régua: caia especificamente numa nota do acorde conforme cada acorde muda, não só em qualquer lugar seguro da escala.
    Ele abre o mesmo tipo de Jam Session aberta — o mapa de furos recolore dourado pra uma nota do acorde, verde pra dentro da escala, âmbar pra fora dela — mas dessa vez dourado é o alvo, não só uma surpresa boa.
    Tente antecipar a mudança um tempo antes: saiba que o acorde IV está chegando e tenha sua nota alvo pronta antes dele chegar, em vez de reagir depois do fato.
    Ainda não há melodia fixa — toque o que quiser, só faça mais coisas caírem em dourado do que o exercício anterior pedia.
    Quando sentir que está pronto pra parar, abra o menu de pausa e aperte "Finalizar Lição" pra o jogo somar sua fração de notas do acorde.

# Lição: blues menor
lesson-minor-blues-improv-title = Blues Menor
lesson-minor-blues-improv-body =
    Mesma jam aberta, mesma gaita em Dó, mas a progressão de base muda pra um blues menor — a 3ª bemol é a nota de repouso agora, não só uma cor de passagem.
    Isso muda o que "estar na escala" e "cair numa nota do acorde" significam sob seus dedos, mesmo sem trocar de gaita ou de posição.
    Deixe-se levar pelo som mais sombrio e melancólico que a progressão menor traz — é um clima diferente do blues maior que você vem tocando, não um erro pra corrigir.
    O mapa de furos ainda recolore ao vivo exatamente como nas outras lições de jam; confie na cor, não no que você esperaria de um blues maior.
    Quando sentir que está pronto pra parar, abra o menu de pausa e aperte "Finalizar Lição" — esta é julgada do mesmo jeito que o exercício de improvisação original, aderência à escala contra a escala de blues menor.

# Lição: improvisação com mudança rápida
lesson-quick-change-improv-title = Improvisação com Mudança Rápida
lesson-quick-change-improv-body =
    Mesma jam aberta e a mesma escala de blues da lição de improvisação original, mas a progressão de base muda para o "quick change" — o compasso 2 vai pro acorde IV em vez de ficar no I.
    Isso significa que o chão harmônico muda sob você dois compassos mais cedo do que na forma padrão de 12 compassos que você já conhece.
    Nada muda sobre quais notas "estão na escala" — esta lição é julgada do mesmo jeito que a lição de improvisação original — mas prestar atenção nessa troca de acorde mais cedo vai afiar seu senso de onde você está na forma.
    Se você se perder, a grade de 12 compassos e o mapa de furos continuam ao vivo e refletem a forma quick change em tempo real.
    Quando sentir que está pronto pra parar, abra o menu de pausa e aperte "Finalizar Lição" — julgada do mesmo jeito que o exercício de improvisação original, aderência à escala contra a escala de blues.

# Lição: pergunta e resposta
lesson-question-answer-title = Pergunta e Resposta
lesson-question-answer-body =
    Esta lição não é sobre o que você toca — é sobre o que você não toca. Toque por dois compassos, depois pare de verdade por dois compassos, alternando pela forma inteira.
    Deixar um silêncio de verdade é o ponto: uma frase que recebe uma resposta precisa de espaço pra resposta, e esse espaço só existe se você parar de perguntar.
    É tentador continuar dedilhando durante o descanso — resista. O mapa de furos e seus próprios ouvidos sabem a diferença entre um descanso e uma nota sustentada.
    Esta é a mesma Jam Session aberta das outras lições de improvisação; toque as frases ou escalas que fizerem sentido nos seus dois compassos, depois solte a gaita de verdade.
    Quando sentir que está pronto pra parar, abra o menu de pausa e aperte "Finalizar Lição" — o jogo julga quanto do que você tocou caiu fora dessas janelas de descanso.

# Lição: escala maior
lesson-major-scale-title = A Escala Maior
lesson-major-scale-body =
    Oito notas, subindo e descendo: 1 sopro, 1 sugado, 2 sopro, 2 sugado dobrado um tom inteiro, 3 sopro, 3 sugado dobrado um tom inteiro, 3 sugado, 4 sopro.
    A gaita de blues vive na escala de blues, mas a escala maior comum é a base da qual até esse som de blues se afasta ao dobrar as notas.
    Duas dessas notas precisam dos dobramentos de um tom inteiro da lição de dobramentos profundos, caindo em Fá e Lá em vez das notas naturais que esses furos soam sem dobrar nada.
    Toque devagar, prestando atenção em exatamente onde a nota precisa cair — um dobramento raso fica curto e ainda soa como a nota natural por baixo dele.
    Quando você conseguir ouvir a diferença entre uma nota natural e sua vizinha dobrada, está pronto pra usar essa escala em melodias de verdade, não só neste exercício.

# Lição: escala pentatônica menor
lesson-minor-pentatonic-scale-title = Escala Pentatônica Menor
lesson-minor-pentatonic-scale-body =
    Seis notas, subindo e descendo: 2 sugado, 3 sugado dobrado, 4 sopro, 4 sugado, 5 sugado, 6 sopro — a escala de blues que você já conhece, sem a nota azul (a 5ª bemol).
    Essa é a escala pentatônica menor, uma das escalas mais usadas no blues, rock e música folk — e nessa gaita, é só a escala de blues sem uma nota.
    Como ela deixa de fora a nota que exige o ouvido mais afiado (a 5ª bemol fica numa posição incômoda entre as outras duas), muitas vezes é mais fácil tocar limpo numa frase rápida do que com a escala de blues completa.
    Repare como ela ainda soa completa e bluesy sozinha — a 5ª bemol é um tempero, não uma obrigação.
    Quando essa forma ficar tão automática quanto a escala de blues completa, tente alternar entre as duas no meio de uma frase — é exatamente a escolha que improvisadores de verdade fazem na hora.

# Lição: escala country
lesson-country-scale-title = A Escala Country
lesson-country-scale-body =
    Nove notas, subindo e descendo, usando só notas naturais de sopro e sugado — sem dobrar nada: 1 sopro, 1 sugado, 2 sopro, 2 sugado, 4 sopro, 4 sugado, 5 sopro, 6 sopro, 6 sugado.
    Essa é a escala pentatônica maior, apelidada de "escala Country" na pedagogia da gaita porque é o som do country, do folk e da música old-time — brilhante e aberta, ao contrário do blues.
    Repare no salto entre 2 sugado e 4 sopro: essa gaita simplesmente não tem uma nota natural no meio nessa oitava, e é exatamente por isso que a escala pula ali.
    Como nada aqui precisa de dobramento, essa é uma ótima escala pra usar cedo no seu aprendizado, ou sempre que você quiser um som alegre e nada bluesy em vez da cor mais sombria da escala de blues.
    Compare como isso soa diferente das frases de escala de blues do resto desta unidade — mesmo instrumento, mesma tonalidade, um clima completamente diferente.

# Lição: improvisação na escala maior
lesson-major-scale-improv-title = Improvisação na Escala Maior
lesson-major-scale-improv-body =
    Outra Jam Session aberta, mas dessa vez o mapa de furos e o julgamento de "Finalizar Lição" são calibrados pela escala maior comum que você acabou de aprender, não pela escala de blues.
    Tudo que você tocar é medido contra Dó maior (as notas da lição da escala maior) em vez da hexatônica de blues que as outras lições de jam deste jogo usam.
    Vai parecer diferente sob seus dedos: sem notas azuis pra se apoiar, sem 3ª ou 5ª dobrada por efeito — só as sete notas retas da escala maior, dentro ou fora.
    Use os dois dobramentos de tom inteiro da lição da escala maior à vontade; eles continuam sendo notas válidas, só que caem em notas da escala maior (Fá e Lá) em vez de notas de blues.
    Quando sentir que está pronto pra parar, abra o menu de pausa e aperte "Finalizar Lição" — julgada por quanto do que você tocou ficou dentro da escala maior.

# Lição: improvisação pentatônica menor
lesson-minor-pentatonic-improv-title = Improvisação Pentatônica Menor
lesson-minor-pentatonic-improv-body =
    Mais uma Jam Session aberta, dessa vez calibrada pela escala pentatônica menor da lição que você acabou de terminar — a escala de blues sem a 5ª bemol.
    Tudo que você tocar é medido contra essa forma de seis-notas-menos-uma, em vez da hexatônica de blues completa que as outras lições de jam usam.
    Você vai perceber que ela perdoa menos — não tem nota azul pra te salvar se uma frase escapar, só as cinco notas pentatônicas limpas, dentro ou fora.
    Essa é exatamente a escala que músicos de verdade buscam quando querem algo que sempre soa "seguro" sobre quase qualquer acorde — essa confiabilidade é o motivo de praticá-la sozinha.
    Quando sentir que está pronto pra parar, abra o menu de pausa e aperte "Finalizar Lição" — julgada por quanto do que você tocou ficou dentro da escala pentatônica menor.

# Lição: círculo das quintas
lesson-circle-of-fifths-title = O Círculo das Quintas
lesson-circle-of-fifths-body =
    O diagrama acima organiza as 12 tonalidades de forma que cada vizinha fica a uma quinta justa de distância — o clássico "círculo das quintas." A tonalidade da sua gaita de referência fica no topo, marcada "harp."
    Cada posição que você vem tocando é só um número de passos no sentido horário ao redor desse círculo a partir da tonalidade da sua gaita. A 2ª posição (cross harp — o som da maioria da gaita de blues) é exatamente um passo no sentido horário. A 3ª posição são dois passos.
    É por isso que a 2ª posição funciona do jeito que funciona: a tonalidade da jam é uma quinta justa acima da tonalidade da gaita, e uma quinta é o intervalo mais consonante depois da oitava — o que também explica por que a gaita de blues se apoia tanto nas notas de sugar nessa posição.
    Existem posições além da 3ª também — 4ª, 5ª, e até 12ª (um passo no sentido *contrário* ao redor do círculo) — cada uma com seu próprio caráter, de mais clara a mais escura e exótica quanto mais longe de casa você viaja.
    Você não precisa decorar esse diagrama — só lembrar da ideia: escolha uma gaita, e toda posição que ela pode tocar já está desenhada pra você, um passo de cada vez, ao redor desse círculo.

# Lição: jam do círculo das quintas
lesson-circle-of-fifths-jam-title = Jam do Círculo das Quintas
lesson-circle-of-fifths-jam-body =
    Hora de realmente tocar o círculo, não só ler sobre ele. Essa é uma Jam Session aberta numa única gaita de C fixa — sem trocar de gaita, nunca.
    A cada poucos compassos, o jogo chama uma nova posição: 1ª, depois 2ª, depois 3ª, repetindo em ciclo. Observe a bússola ao lado do diagrama da sua gaita — a tonalidade destacada muda pra mostrar qual está ativa agora.
    Trocar de posição numa única gaita significa mudar onde suas frases resolvem, não quais buracos existem. A 1ª posição descansa no buraco 4 (sopro); a 2ª se apoia nas notas de sugar e descansa nelas; a 3ª muda de novo. Os buracos são os mesmos — o que conta como "casa" é que muda.
    Não entre em pânico se estiver no meio de uma frase quando a chamada mudar — termine seu pensamento, depois trabalhe seu caminho até as notas de descanso da nova posição.
    Quando sentir que está pronto pra parar, abra o menu de pausa e aperte "Finalizar Lição" — julgada por quanto do que você tocou ficou dentro de qualquer posição que estava sendo chamada no momento.

# Lição: colcheias swingadas
lesson-swing-eighths-title = Colcheias Swingadas
lesson-swing-eighths-body =
    Bem-vindo à unidade de jazz. Mesmo balanço de colcheias longa-curta do shuffle-feel, mas a janela de tempo é mais apertada — o swing de jazz recompensa precisão, não só as notas certas.
    Dois pares de notas dessa vez em vez de um: 4 sopro/4 sugado, depois 5 sopro/5 sugado, alternando. Mantenha o mesmo balanço longa-curta em cada par ao trocar entre eles.
    Se o shuffle-feel pareceu confortável, isso deve parecer familiar, mas exigir mais — preste atenção se você está caindo bem em cima do tempo swingado ou só perto dele.
    Esse senso de tempo mais apertado atravessa o resto desta unidade; as lições de ii-V-I e blues de jazz que vêm a seguir assumem que você já consegue swingar colcheias com limpeza.
    Vá devagar antes de acelerar — um swing limpo num andamento moderado vale mais que um bagunçado num andamento rápido.

# Lição: notas do acorde ii-V-I
lesson-ii-v-i-chord-tones-title = Notas do Acorde ii-V-I
lesson-ii-v-i-chord-tones-body =
    A cadência central do vocabulário de jazz: um acorde ii menor com sétima, um acorde V dominante com sétima, e um acorde I maior com sétima, cada um arpejado por vez — Dm7, depois G7, depois Cmaj7.
    Duas dessas notas precisam dos dobramentos de um tom inteiro da lição de dobramentos profundos (2 sugado dobrado pro Fá, 3 sugado dobrado pro Lá) — os mesmos dobramentos que você já usou na escala maior, agora aplicados pra desenhar um acorde em vez de uma escala.
    Repare como cada arpejo tem seu próprio caráter: Dm7 soa em suspenso, G7 puxa em direção à resolução, e Cmaj7 finalmente chega em casa — esse puxão é o ponto inteiro dessa cadência.
    Essa é a espinha dorsal harmônica por baixo de uma quantidade enorme do repertório de jazz; assim que essas três formas estiverem sob seus dedos, você vai começar a ouvir esse mesmo movimento ii-V-I em todo lugar.
    Toque devagar no começo, focando em cair limpo em cada nota do acorde em vez de correr pelas mudanças.

# Lição: a forma do blues de jazz
lesson-jazz-blues-form-title = A Forma do Blues de Jazz
lesson-jazz-blues-form-body =
    Mesma Jam Session aberta das outras lições de improvisação, mas a progressão de base agora é o blues de jazz completo — a forma padrão de 12 compassos com uma volta ii-V-I de verdade construída nos últimos compassos.
    O compasso 8 substitui por uma dominante secundária, depois os compassos 9 e 10 percorrem o próprio ii-V que você acabou de aprender na lição anterior, resolvendo pro I antes da volta final em V.
    Isso é julgado do mesmo jeito que a improvisação sobre notas do acorde: caia especificamente numa nota do acorde conforme as mudanças acontecem, não só em qualquer lugar da escala — mas agora as mudanças acontecem mais rápido e de um jeito menos previsível nessa volta.
    O mapa de furos e a grade de 12 compassos continuam ao vivo e refletem a forma do blues de jazz em tempo real, então confie neles se você se perder de onde a volta começa.
    Quando sentir que está pronto pra parar, abra o menu de pausa e aperte "Finalizar Lição" — o jogo soma quanto do que você tocou caiu numa nota do acorde.

# Lição: fundamentos do slide cromático
lesson-chromatic-slide-basics-title = Fundamentos do Slide Cromático
lesson-chromatic-slide-basics-body =
    Sua primeira lição de gaita cromática: uma escala cromática completa, um semitom de cada vez, subindo e descendo — usando o botão de slide pra preencher cada nota que o padrão de sopro/sugado sozinho pula.
    Diferente de um dobramento diatônico, o slide é mecânico, não de embocadura: aperte o botão pra uma nota deslizada, solte pra uma natural, e a altura muda instantânea e precisamente.
    Cerca de metade desse trecho precisa do slide apertado — repare como ele sempre é usado pra alcançar a nota específica um semitom acima do que aquele furo já dá sem apertar nada.
    Essa escala é de propósito só técnica, não melodia — assim que apertar o slide bem na nota certa parecer automático, você está pronto pra usá-lo musicalmente, não só mecanicamente.
    Tome seu tempo pra encaixar o momento do slide com o início da nota — um slide apertado um pouco tarde ainda registra como a nota errada, sem o slide.
# Unit 6 — hole navigation and register changes

lesson-landmarks-title = Pontos de Referência: Furos 1, 4, 7 e 10
lesson-landmarks-body =
    A gaita não tem trastes e você não consegue vê-la enquanto toca, então a navegação é pelo tato e pelo ouvido.
    Numa gaita em C, soprar nos furos 1, 4, 7 e 10 dá sempre um dó — a mesma nota, uma oitava acima a cada vez. Domine esses quatro e você sempre sabe onde está.
    Deslize ao longo da gaita em vez de afastá-la: os cantos da boca continuam encostados, e assim você conta os furos pelo tato.
    Este exercício sobe os quatro pontos de referência e volta. Sem pressa — compare cada nota com a anterior e escute a oitava.

lesson-middle-register-title = O Registro Médio: Furos 4 a 6
lesson-middle-register-body =
    Os furos 4 a 6 são onde vive a maioria das melodias, e onde a gaita se comporta de forma mais simples: soprar e aspirar se alternam formando dó ré mi fá sol lá.
    Este é o registro a automatizar primeiro — todo o resto é medido a partir dele.
    Mantenha o ar suave e constante. Uma nota do registro médio precisa de muito menos ar do que a maioria dos iniciantes usa.
    Toque subindo e descendo. A virada no topo é onde se perde o fio, então vá devagar.

lesson-hole-jumps-title = Saltando Entre Furos
lesson-hole-jumps-body =
    Mover dois ou três furos não é o mesmo que ir ao furo vizinho: é preciso interromper o fluxo de ar, mover e recomeçar.
    Deslizar faz soar todos os furos do caminho, e um salto limpo vira um borrão. Pare o ar por um instante enquanto se move — uma pausa mínima incomoda muito menos que um borrão.
    Este exercício salta entre os furos de referência, todos dó, então um pouso errado fica evidente assim que você o ouve.
    Se cair no furo vizinho, não corrija deslizando. Pare e coloque a nota de novo.

lesson-crossing-three-four-title = Cruzando os Furos 3 e 4
lesson-crossing-three-four-body =
    Do furo 3 para o 4 está a emenda entre o registro grave e o médio, e é o primeiro ponto em que a gaita deixa de parecer regular.
    Subindo você toca sol, si, dó, ré — mas o salto de sol para si é maior que os passos ao redor, o que faz o movimento parecer mais longo do que é.
    Mantenha a posição da boca idêntica e deixe a gaita se mover. A maioria exagera aqui e cai no furo 5.
    Toque subindo e descendo até o tamanho do movimento deixar de surpreender.

lesson-crossing-six-seven-title = Cruzando os Furos 6 e 7
lesson-crossing-six-seven-body =
    Esta é a que pega todo mundo. Até o furo 6, aspirar é mais agudo que soprar. Do furo 7 para cima isso se inverte: soprar é mais agudo que aspirar.
    Cruze a emenda e seus instintos ficam de repente ao contrário.
    Subindo por sol, lá, si, dó você toca soprar 6, aspirar 6, depois aspirar 7 e soprar 7 — o aspirar e o soprar trocam de ordem exatamente na travessia.
    Não há truque além de saber que vem aí e treinar até a inversão ser o que suas mãos esperam. Vá devagar e diga os números dos furos para si mesmo.

lesson-low-register-title = O Registro Grave: Furos 1 a 3
lesson-low-register-body =
    O grave é mais escuro e encorpado, e menos tolerante: as palhetas são mais longas, então pedem ar mais lento e quente e demoram um instante a mais para falar.
    Apressá-las dá uma nota abafada, ou nada. Baixe o queixo e pense "ó" em vez de "i".
    Este exercício sobe dó ré mi sol si dó. O sol vem do aspirar 2 e não do soprar 3 — os dois dão a mesma altura numa gaita em C, e escolher um mantém cada nota do exercício distinta.
    Mantenha o ar lento. Se uma nota travar, alivie em vez de forçar.

lesson-high-register-title = O Registro Agudo: Furos 7 a 10
lesson-high-register-body =
    Os quatro furos do topo querem o oposto dos quatro de baixo: as palhetas são curtas, então precisam de menos ar e de uma boca um pouco mais fechada.
    Soprar forte aqui em cima soa estridente e pode travar uma palheta.
    Lembre da inversão — do furo 7 para cima, aspirar fica abaixo de soprar, então esta sequência alterna aspirar-soprar o tempo todo, e não soprar-aspirar.
    Alivie conforme sobe. Se uma nota aguda apitar, é ar demais, não ar de menos.

lesson-register-leaps-title = Saltando Entre os Registros
lesson-register-leaps-body =
    O fecho da unidade: os três registros num só exercício, saltando entre eles como faz um solo de verdade.
    Cada registro quer um ar diferente — lento e quente embaixo, suave em cima — então um salto é também uma mudança de respiração, não só de posição.
    Os dós de referência são sua rede de segurança. Se perder o lugar, ache o mais próximo e recomece dali.
    Faça a primeira passada devagar o bastante para colocar cada nota com intenção. A velocidade vem depois da precisão, nunca antes.


# Jogo — contagem regressiva, legenda, dicas do diagrama da harmônica
gameplay-get-ready = PREPARE-SE
gameplay-legend-blow = ■ SOPRO
gameplay-legend-draw = ■ SUGADA
harmonica-overlay-hint-view = Harmônica  ·  acende conforme você toca
harmonica-overlay-hint-select = Harmônica  ·  clique numa nota para selecioná-la
gameplay-chart-info = Tom: {$key}  ♩ = {$bpm}  {$time_sig}
gameplay-chart-author = Partitura: {$author}
gameplay-techniques-toggle = {$arrow} TÉCNICAS

# Menu de pausa
pause-quit-song = Sair da música
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
bending-no-note-for-technique = Este furo não tem nota para essa técnica.
bending-key-label = Tom
bending-listen-button = 🔊 Ouvir
bending-drill-button = 🎲 Exercício
bending-play-it-target = Toque — alvo {$note}
bending-in-tune = ✓ Afinado  ({$note})
bending-cents-sharp = ↑ {$cents} cents agudo  (alvo {$note})
bending-cents-flat = ↓ {$cents} cents grave  (alvo {$note})
bending-detect-label = Detectar
bending-section-setup = Configuração
bending-section-target = Alvo de Prática
bending-section-drill = Exercício
bending-section-tempo = Andamento
bending-tempo-decrease = Diminuir andamento
bending-tempo-increase = Aumentar andamento

# Jam Session
jam-loop-button = ↻ Loop
jam-loop-off = Loop: desligado
jam-loop-on = Loop: ligado
jam-hole-map-hint = Sua harmônica  ·  dourado = tom do acorde agora  ·  verde = nota da escala de blues  ·  sopro em cima / sugada embaixo
jam-call-response-button = ⇄ Pergunta & Resposta
jam-call-response-off = Pergunta & Resposta: desligado
jam-call-response-on = Pergunta & Resposta: ligado
jam-call-response-listen = Escute…
jam-call-response-your-turn = Sua vez
jam-midi-track-mute-tooltip = Clique para silenciar/ativar esta faixa
jam-rhythm-guide = Guia de Ritmo
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
results-hits = Acertos
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
results-avg-timing-offset = Desvio médio de tempo
results-increase-latency = Aumentar a latência de entrada para {$ms}ms
results-decrease-latency = Diminuir a latência de entrada para {$ms}ms
results-score = Pontuação: {$points}
results-best-score = Melhor pontuação

# Calibração de latência
calibration-title = Calibração de Latência
calibration-instructions = Toque qualquer nota em cada batida — o jogo mede o atraso com que o microfone detecta o som.
calibration-mean-offset-placeholder = Deslocamento médio: —
calibration-mean-offset = Deslocamento médio: {$sign}{$ms}ms
calibration-suggested-placeholder = Atual: —   →   Sugerido: —
calibration-suggested = Atual: {$current}ms   →   Sugerido: {$suggested}ms

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
tutorial-title-mode-select = Selecionar Modo
tutorial-body-mode-select = Escolha 2D (uma pista de notas rolando) ou 3D (uma harmônica que você toca junto).
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
# O seletor de faixa na página de verificação da gaita, exibido apenas para
# um arquivo importado com mais de uma parte tocável.
harp-check-track = Parte a tocar
harp-check-track-option = {$name} — {$notes} notas, {$percent}% tocável
harp-check-track-unnamed = Faixa {$index}
# Cabeçalho acima dos cinco níveis de treino de uma lição, com quanto da
# escada foi concluído (o medidor de domínio).
lesson-training-heading = Treino — {$percent}% dominado

# The skill-tree view of the curriculum: one row per track.
lesson-tree-title = Árvore de Habilidades
lesson-tree-broken = Este currículo não pode ser desenhado: {$error}
lesson-tree-unit-progress = {$done}/{$needed}
lesson-tree-needs = Precisa de: {$lessons}
lesson-tree-optional = Eletiva
lesson-track-tone = Tom
lesson-track-hand = Wah de Mão
lesson-track-tongue = Bloqueio de Língua
lesson-track-bend = Bends
lesson-track-vibrato = Vibrato
lesson-track-slide = Registro
lesson-track-time = Tempo
lesson-track-form = Forma
lesson-track-train = Ritmo de Trem
lesson-track-scales = Escalas
lesson-track-theory = Teoria
lesson-track-harmony = Harmonia
lesson-track-vocabulary = Vocabulário
lesson-track-improv = Improvisação
lesson-track-navigation = Navegação
lesson-track-positions = Posições
lesson-track-ear = Treinamento Auditivo
lesson-unit-advanced = Eletivas avançadas
lesson-unit-rhythm-lab = Laboratório de ritmo
lesson-unit-positions = Posições aplicadas
lesson-unit-ear-training = Treinamento auditivo
lesson-match-heard-note-title = Iguale uma nota ouvida
lesson-match-heard-note-body = Ouça a chamada, encontre e repita cada altura. A pista de notas fica escondida: use o ouvido, teste um orifício próximo se necessário e memorize o som quando encontrá-lo.
lesson-melodic-motion-title = Sobe, desce ou repete
lesson-melodic-motion-body = Antes de tocar, diga o movimento que ouviu: mais alto, mais baixo ou repetido. Depois imite. Reconhecer a direção reduz a busca antes de identificar as notas exatas.
lesson-hidden-short-phrase-title = Repita uma frase escondida
lesson-hidden-short-phrase-body = Guarde a frase inteira na memória, incluindo direção e ritmo, e toque sem pistas visuais. Ouça o contorno antes de procurar orifícios individuais.
lesson-find-tonic-title = Encontre a tônica
lesson-find-tonic-body = Ouça qual nota faz cada frase parecer completa e resolva nela. Neste exercício com harmônica em Dó, o centro é um Dó soprado, mas encontre-o pelo som antes de usar o número do orifício.
lesson-first-position-landings-title = Chegadas na primeira posição
lesson-first-position-landings-body = Na primeira posição, Dó é o centro em uma harmônica em Dó. Toque as notas do acorde de Dó maior e termine cada frase em um Dó soprado.
lesson-second-position-landings-title = Chegadas na segunda posição
lesson-second-position-landings-body = Na segunda posição, Sol vira o centro. Apoie-se no 2 aspirado e nas notas do acorde de Sol; ouça como a mesma harmônica agora resolve para outro centro.
lesson-third-position-landings-title = Chegadas na terceira posição
lesson-third-position-landings-body = A terceira posição coloca Ré no centro e dá uma cor menor às notas aspiradas naturais. Use o 4 e o 8 aspirados como referências de tônica e termine em Ré.
lesson-eighth-note-triplets-title = Tercinas de colcheia
lesson-eighth-note-triplets-body = Divida cada pulso em três partes iguais. Conte “um-tri-pla” e mantenha os ataques igualmente espaçados antes de acelerar.
lesson-straight-vs-shuffle-title = Reto e shuffle
lesson-straight-vs-shuffle-body = Colcheias retas dividem o pulso igualmente. O shuffle usa a primeira e a terceira parte de uma tercina. Ouça a mudança e coloque a nota curta tarde o bastante para criar o swing.
lesson-syncopation-rests-title = Síncopes e pausas
lesson-syncopation-rests-body = Comece no “e” entre os tempos numerados. Continue contando em cada intervalo: uma pausa tem duração exata e dá forma ao próximo contratempo.
lesson-sixteenth-articulation-title = Articulação de semicolcheias
lesson-sixteenth-articulation-body = Encaixe quatro ataques limpos em um pulso. Use a língua leve e o ar constante para iniciar cada nota com clareza sem tensionar a mandíbula.
lesson-tempo-stability-title = Estabilidade do andamento
lesson-tempo-stability-body = Acompanhe o metrônomo enquanto ele acelera e desacelera. Ouça o próximo pulso antes de tocar e ajuste-se imediatamente em vez de manter o andamento anterior.
lesson-high-blow-bends-title = Bends soprados agudos: furos 8–10
lesson-high-blow-bends-body =
    O bend muda de direção no registro agudo: os furos 8, 9 e 10 fazem bend na nota soprada. Comece no 8. Sustente o 8 soprado, diminua o espaço dentro da boca e eleve o meio da língua até a altura cair meio tom. Mantenha o ar uniforme; soprar com mais força dificulta o controle.

    Repita o mesmo movimento de meio tom nos furos 9 e 10. O furo 10 também pode descer um tom inteiro, por isso o exercício dá mais tempo a esse alvo. Ouça primeiro a nota natural, vá deliberadamente até a nota com bend e volte sem um novo ataque. A frase final conecta os três furos de forma musical.
lesson-overblows-title = Oversopros
lesson-overblows-body = Oversopros fecham a palheta normal de sopro para que a palheta aspirada soe acima da altura habitual. Comece suavemente nos orifícios 4, 5 e 6 e volte à palheta natural sem forçar o ar.
lesson-overdraws-title = Overpuxadas
lesson-overdraws-body = Overpuxadas aplicam o mesmo controle de palheta à aspiração no registro agudo. Trabalhe suavemente nos orifícios 7–10 e compare cada nota com a natural vizinha antes de ligá-las.
lesson-unit-tongue-block = Eletivas de bloqueio de língua
lesson-tongue-slap-title = Golpe de língua
lesson-tongue-slap-body = Comece soando vários orifícios e coloque a língua no pente até restar apenas a nota alvo. O acorde breve antes da nota limpa cria o golpe. O microfone não pode confirmar a embocadura, então marque a instrução depois de praticar devagar.
lesson-tongue-pull-title = Levantamento de língua
lesson-tongue-pull-body = Comece com a nota isolada pelo bloqueio, levante a língua brevemente para expor o acorde e volte. Mantenha o ar contínuo e o movimento pequeno.
lesson-side-pull-title = Levantamento lateral
lesson-side-pull-body = Mantenha a língua ancorada enquanto levanta um lado brevemente, acrescentando um acorde estreito sem perder a nota principal. Pratique o movimento sem ar antes de tocar.
lesson-tongue-rake-title = Varredura de língua
lesson-tongue-rake-body = Sopre ou aspire um acorde enquanto varre os orifícios com a língua e termine em uma nota limpa. Mantenha a mandíbula parada para a língua produzir o movimento.
lesson-tongue-flutter-title = Vibração de língua
lesson-tongue-flutter-body = Alterne rapidamente a língua sobre os orifícios bloqueados com ar constante. Comece com pulsos lentos e regulares; acelere apenas se a nota principal e o acorde continuarem claros.
lesson-moving-octaves-title = Oitavas móveis
lesson-moving-octaves-body = Mantenha um bloqueio de três orifícios deixando as duas notas externas soarem e deslize a forma inteira. O exercício pontua se as duas alturas da oitava permanecem presentes em cada chegada.
lesson-unit-ornaments = Ornamentos
lesson-track-ornaments = Ornamentos
lesson-slow-warble-title = Warble lento
lesson-slow-warble-body = Alterne dois orifícios vizinhos sem interromper o ar. Comece devagar para as duas alturas ficarem claras; a partitura avalia o padrão audível, independentemente de mover a cabeça, as mãos ou a harmônica.
lesson-fast-shake-title = Shake rápido
lesson-fast-shake-body = Comprima a alternância entre orifícios vizinhos em semicolcheias regulares. Mantenha o movimento pequeno e preserve duas alturas distintas ao aumentar a velocidade.
lesson-trill-cells-title = Células de trinado
lesson-trill-cells-body = Acrescente uma alternância curta com a nota vizinha superior e volte claramente à nota principal. Trate o ornamento como parte medida da frase.
lesson-glissandi-title = Glissandos
lesson-glissandi-body = Percorra orifícios consecutivos em velocidade uniforme e chegue ao destino escrito. O microfone pontua o caminho e a chegada; mantenha constante a pressão do ar.
lesson-unit-expression = Expressão
lesson-hand-vibrato-title = Vibrato de mão
lesson-hand-vibrato-body = Abra e feche a concha das mãos ao redor da harmônica para pulsar o timbre e o volume, mantendo a nota estável. O método de produção é instrucional.
lesson-throat-vibrato-title = Vibrato de garganta
lesson-throat-vibrato-body = Sustente uma nota relaxada e introduza pulsos suaves e regulares na garganta sem cortar o ar. Comece devagar e pare se o pescoço tensionar.
lesson-diaphragm-vibrato-title = Vibrato de diafragma
lesson-diaphragm-vibrato-body = Use pulsos pequenos e regulares de pressão do ar com boca e garganta relaxadas. Busque uma onda controlada sobre uma nota contínua.
lesson-vibrato-control-title = Controle da velocidade do vibrato
lesson-vibrato-control-body = Iguale alvos lentos de três pulsos e rápidos de cinco pulsos. O jogo mede a frequência da oscilação da altura, não o método físico nem a profundidade exata.
lesson-dynamic-shaping-title = Crescendo e diminuendo
lesson-dynamic-shaping-body = Sustente uma altura enquanto cresce gradualmente de suave a forte e depois inverta a forma. A pontuação atual não avalia honestamente o envelope de volume; ouça e grave sua prática.
lesson-long-tone-control-title = Controle de notas longas
lesson-long-tone-control-body = Sustente cada nota por toda a duração com um centro estável. Comece confortavelmente, relaxe os ombros e reserve ar para terminar sem apertar.
lesson-unit-accompaniment = Acompanhamento
lesson-track-accompaniment = Acompanhamento
lesson-chord-rhythm-title = Ritmo de acordes
lesson-chord-rhythm-body = Coloque acordes curtos e equilibrados no pulso e solte-os claramente para apoiar a música sem ocupá-la demais.
lesson-behind-singer-title = Tocando atrás de uma voz
lesson-behind-singer-body = Trate a voz como primeiro plano. Toque suavemente apenas quando houver espaço, evite o registro dela e deixe palavras importantes livres. Essa escolha de escuta é instrucional.
lesson-vocal-fills-title = Preenchimentos vocais
lesson-vocal-fills-body = Espere durante a frase vocal imaginária e responda ao intervalo com um lick curto. Termine antes da próxima frase começar.
lesson-trading-fours-title = Troca de quatro compassos
lesson-trading-fours-body = Alterne quatro compassos tocando com quatro ouvindo. Use o turno silencioso para ouvir um parceiro imaginário e responder depois.
lesson-unit-chromatic = Harmônica cromática
lesson-chromatic-button-coordination-title = Coordenação do botão
lesson-chromatic-button-coordination-body = Pressione e solte a chave sem parar o ar nem mover a boca. Alterne notas naturais e elevadas até o botão coincidir exatamente com a mudança de altura.
lesson-enharmonic-slide-choices-title = Escolhas enarmônicas
lesson-enharmonic-slide-choices-body = A mesma altura pode ter mais de uma escrita ou caminho. Escolha o caminho que conecte a frase e prepare a próxima nota. A pontuação não distingue alturas equivalentes, então esta lição é instrucional.
lesson-chromatic-fragments-title = Fragmentos cromáticos
lesson-chromatic-fragments-body = Pratique células curtas de semitons em vez da escala inteira. Ouça cada forma como um gesto e sincronize a chave com o ar.
lesson-legato-slide-phrasing-title = Fraseado legato com chave
lesson-legato-slide-phrasing-body = Conecte notas naturais e elevadas sem intervalo. Mantenha o ar constante enquanto o botão muda a palheta sonora.
lesson-chromatic-jazz-study-title = Luzes cruzadas
lesson-chromatic-jazz-study-body = Aplique swing, fragmentos cromáticos e coordenação limpa da chave neste estudo original escrito para Harmonicon.
lesson-unit-musicianship = Musicalidade
lesson-intervals-title = Intervalos
lesson-intervals-body = Ouça e toque a distância de Dó até uma segunda, terça, quarta, quinta e oitava. Diga a distância antes da segunda nota.
lesson-chord-construction-title = Construção de acordes
lesson-chord-construction-body = Construa uma tríade empilhando graus alternados: fundamental, terça e quinta. Acrescente o próximo grau para formar um acorde de sétima.
lesson-guide-tones-title = Notas guia
lesson-guide-tones-body = Terças e sétimas identificam o acorde e conduzem suavemente ao próximo. Siga essas notas em um contorno ii–V–I.
lesson-arpeggio-scale-title = Do arpejo à escala
lesson-arpeggio-scale-body = Apresente o acorde com fundamental, terça e quinta e conecte esses pontos com notas vizinhas da escala.
lesson-transposition-title = Transposição
lesson-transposition-body = A transposição move cada altura pelo mesmo intervalo. Trocar a tonalidade da harmônica costuma preservar a digitação; trocar a posição preserva a harmônica e muda o centro tonal.
lesson-song-form-listening-title = Escutando a forma
lesson-song-form-listening-body = Acompanhe gravações sem tocar e marque seções repetidas, contrastes, retornos e turnarounds. Confira sua análise com a gravação e marque a lição.
lesson-unit-practice = Habilidades de prática
lesson-track-practice = Prática
lesson-slow-practice-title = Prática lenta
lesson-slow-practice-body = Reduza a velocidade até controlar cada repetição e depois aumente um pequeno passo. A velocidade resulta de repetições precisas.
lesson-loop-isolation-title = Isolamento de loop
lesson-loop-isolation-body = Selecione a frase mais curta que contenha o problema, inclua um pouco de contexto e repita até obter várias execuções limpas antes de ampliar o loop.
lesson-spaced-review-title = Revisão espaçada
lesson-spaced-review-body = Retome o material difícil depois de uma pausa. Misture uma habilidade recente com uma antiga e anote o que precisa da próxima revisão.
lesson-self-recording-title = Auto-gravação
lesson-self-recording-body = Grave uma tomada inteira, ouça sem o instrumento e anote um problema de tempo e um de timbre. Repita com uma única mudança concreta.
lesson-mixed-review-title = Revisão mista
lesson-mixed-review-body = Alterne chegadas de posição, bends, vibrato, mudanças de andamento e movimento de registro. As mudanças testam a memória.
