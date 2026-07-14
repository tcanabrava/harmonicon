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
lesson-goal-scale-adherence = Meta: %pct%% das notas dentro da escala ou melhor
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
