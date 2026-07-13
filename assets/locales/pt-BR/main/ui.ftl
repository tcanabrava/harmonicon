# Harmonicon — Português do Brasil (pt-BR) UI strings.
#
# Mantenha as chaves em sincronia com assets/locales/en-US/main/ui.ftl.

app-title = Harmonicon

# Menu principal
menu-play = Tocar
menu-song-editor-2 = Editor de Músicas
menu-options = Opções
menu-credits = Créditos
menu-quit = Sair

# Menu de tocar
play-song = Tocar Música
jam-session = Jam Session
bending-trainer = Treino de Bends

# Seleção de modo
select-mode = Selecionar Modo
play-2d = Tocar em 2D
play-3d = Tocar em 3D

# Seleção de música / artista
select-artist = Selecionar Artista
select-song = Selecionar Música
no-songs-found = Nenhuma música encontrada. Adicione pastas em assets/songs/<artista>/<música>/

# Opções
options-title = Opções
options-language = Idioma

# Compartilhado
back = ← Voltar

# Editor de Músicas 2 — botões de transporte e painel de modificadores
editor-mode-edit = ✎ Editar
editor-mode-perform = 🎵 Apresentar
editor-lock = 🔒 Bloquear
editor-play = ▶ Tocar
editor-pause = ⏸ Pausar
editor-stop = ■ Parar
editor-practice = 🎤 Praticar
editor-save = 💾 Salvar
editor-load = 📂 Carregar
editor-browse = 📂 Procurar
mod-blow = Soprar
mod-draw = Puxar
mod-bend = Dobrar
mod-overblow = Oversopro
mod-overdraw = Overpuxar
mod-slide = Slide
mod-wah = Wah
mod-vibrato = Vibrato
mod-delete = Apagar

# Editor de Músicas 2 — rótulos dos campos de metadados
editor-field-tempo = Andamento da Música
editor-field-key = Tom do Gaita
editor-field-position = Posição
editor-field-harmonica = Gaita
editor-field-music = Música de Fundo
editor-field-name = Nome
editor-field-author = Autor
editor-harmonica-diatonic = ‹ Diatônica (10 buracos) ›
editor-harmonica-chromatic = ‹ Cromática (12 buracos) ›

# Editor de Músicas 2 — títulos dos diálogos de arquivo
dialog-save-chart = Salvar partitura
dialog-load-chart = Carregar partitura
dialog-select-music = Selecionar música de fundo

# Editor de Músicas 2 — mensagens de validação de arrastar
drag-denied-bend = Este buraco não suporta esta profundidade de dobra
drag-denied-overblow = Oversopro está disponível apenas nos buracos 1–6
drag-denied-overdraw = Overpuxar está disponível apenas nos buracos 7–10
drag-denied-overlap = Já existe uma nota aqui

# Editor de Músicas 2 — feedback do modo de prática
practice-no-music = Nenhuma música de fundo definida — toque seguindo a partitura!
practice-prompt = ▶ Toque %note%…
practice-wrong-note = ▶ %got% → precisa de %expected%
practice-hit-perfect = ✓ PERFEITO  %note%  +%pts% pts
practice-hit-good = ✓ BOM  %note%  +%pts% pts
practice-missed = ✗ Perdeu %note%
practice-done = Feito — %hits%/%total% notas  ·  %score% pts

# Editor de Músicas 2 — dicas dos botões
editor-back-tooltip = Sair do editor e voltar ao menu principal
editor-mode-edit-tooltip = Mudar para o modo Editar — posicione, mova e edite notas na grade
editor-mode-perform-tooltip = Mudar para o modo Apresentar — toque ou pratique a partitura
editor-lock-tooltip = Bloquear a grade para evitar edições acidentais durante a revisão
editor-save-tooltip = Salvar esta partitura em um arquivo .harpchart
editor-load-tooltip = Carregar uma partitura de um arquivo .harpchart
editor-play-tooltip = Iniciar ou retomar a reprodução da partitura
editor-pause-tooltip = Pausar a reprodução onde está
editor-stop-tooltip = Parar a reprodução e voltar o cursor ao início
editor-practice-tooltip = Modo prática — toque junto com sua gaita e receba feedback ao vivo
mod-blow-tooltip = Definir a nota selecionada como sopro (expirar)
mod-draw-tooltip = Definir a nota selecionada como puxada (inspirar)
mod-bend-tooltip = Alternar a profundidade da dobra da nota selecionada: nenhuma → meio tom → tom inteiro → tom e meio
mod-overblow-tooltip = Definir a nota selecionada como oversopro (técnica avançada de sopro, apenas diatônica)
mod-overdraw-tooltip = Definir a nota selecionada como overpuxada (técnica avançada de puxada, apenas diatônica)
mod-slide-tooltip = Definir a nota selecionada para usar o botão slide (apenas gaitas cromáticas)
mod-wah-tooltip = Alternar a taxa de wah-wah da nota selecionada
mod-vibrato-tooltip = Alternar a taxa de vibrato da nota selecionada
mod-delete-tooltip = Apagar a nota selecionada
editor-harmonica-toggle-tooltip = Clique para alternar entre gaita Diatônica e Cromática
editor-field-key-tooltip = Clique para alternar entre os tons da gaita
editor-field-position-tooltip = Clique para alternar entre as posições de execução
editor-browse-tooltip = Escolher um arquivo de áudio de música de fundo para esta partitura

# Lições — menu, leitor, veredito nos resultados
menu-lessons = Lições
no-lessons-found = Nenhuma lição encontrada. Adicione pastas em assets/lessons/<unidade>/<lição>/
lesson-locked = bloqueada
lesson-passed = Concluída
lesson-start = Começar a Lição
lesson-mark-done = Marcar como Concluída
lesson-goal-accuracy = Meta: %pct%% de precisão geral
lesson-goal-technique = Meta: %pct%% de precisão nas notas de %technique%
lesson-goal-finish = Meta: tocar até o fim
lesson-complete-banner = LIÇÃO CONCLUÍDA
lesson-failed-banner = Meta não atingida — releia a lição e tente de novo

# Lições — títulos das unidades (chave vem do campo "unit" de cada lesson.json)
lesson-unit-blowing = Unidade 1 · Soprando a Gaita
lesson-unit-rhythm = Unidade 2 · Contando o Blues

# Lição: nota única
lesson-single-note-title = Tocando uma Nota Só
lesson-single-note-body =
    O maior obstáculo do iniciante na gaita: tirar uma nota limpa em vez de um acorde de vizinhas.
    Faça bico como se fosse assobiar, ou diga a sílaba "tu" — a abertura deve ser pouco maior que um furo.
    Relaxe: a gaita entra fundo entre os lábios, apoiada na parte interna úmida, não presa pela borda seca.
    Incline o fundo da gaita levemente para cima e deixe o queixo cair, para o ar sair lento e quente, vindo da barriga.
    Neste exercício, notas longas nos furos 4, 5 e 6 deslizam até a linha de acerto. Sopre cada uma com calma — não importa o volume, importa a pureza.
    Se ouvir duas notas ao mesmo tempo, não force: estreite um pouco a abertura e desacelere o sopro.

# Lição: formato das mãos / wah
lesson-hand-wah-title = O Formato das Mãos e o Wah
lesson-hand-wah-body =
    Suas mãos são o controle de timbre da gaita. Feche-as em concha atrás do instrumento formando uma câmara de ar vedada, e abra e feche a vedação para ela "falar": "uá".
    Segure a gaita entre o polegar e o indicador de uma mão, e vede a outra mão atrás, como uma concha.
    Concha fechada = som escuro, abafado. Concha aberta = claro e forte. Abrir a concha em ritmo enquanto a nota soa produz o clássico wah-wah.
    Neste exercício, sustente cada nota com firmeza e abra-e-feche a concha cerca de duas vezes por segundo — o jogo escuta esse pulso no seu som.
    Mantenha o sopro constante; só as mãos se movem. Se nada registrar, aperte a vedação da concha — quase todo o efeito mora no último centímetro do fechamento.

# Lição: lendo a grade de 12 compassos
lesson-twelve-bar-title = Lendo a Grade do Blues de 12 Compassos
lesson-twelve-bar-body =
    Quase toda música de blues segue o mesmo ciclo de 12 compassos — aprenda a lê-lo uma vez e você acompanha qualquer roda de blues do planeta.
    Cada célula da grade é um compasso de quatro tempos. Os algarismos romanos nomeiam os acordes: I é o acorde de casa, IV a viagem do meio, V a tensão do retorno.
    O desenho clássico: quatro compassos de I, dois de IV, dois de I, um de V, um de IV e dois compassos finais de I (o último costuma virar V para lançar o próximo chorus — o "turnaround").
    Conte em voz alta: "UM dois três quatro, DOIS dois três quatro..." — doze compassos, e o ciclo recomeça.
    Você verá essa grade ao vivo na Jam Session, onde o compasso atual acende conforme a base toca. Depois desta lição, abra uma Jam Session e apenas observe alguns ciclos passarem, contando junto, antes de tocar qualquer nota.
