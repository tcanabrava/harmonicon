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
jam-generate-key = Tom: %key%
jam-generate-tempo = Andamento: %bpm%
jam-generate-progression = Progressão: %progression%
jam-generate-position = Posição: %position%

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
options-adaptive-difficulty-on = Dificuldade Adaptativa: ativada
options-adaptive-difficulty-off = Dificuldade Adaptativa: desativada
options-fullscreen = Tela cheia
options-fullscreen-on = Tela cheia: ativada
options-fullscreen-off = Tela cheia: desativada
options-pitch-detect = Detecção de tom
options-microphone = Microfone
options-note-labels-button = Rótulos das notas
options-note-labels-arrows = Rótulos das notas: setas
options-note-labels-numbers = Rótulos das notas: números
options-calibrate-input-lag = Calibrar a latência de entrada
theme-back-to-options = ← Voltar às Opções

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
editor-record = ⏺ Gravar
editor-record-stop = ⏹ Parar Gravação
editor-save = 💾 Salvar
editor-load = 📂 Carregar
editor-browse = 📂 Procurar
editor-import-midi = 🎹 Importar MIDI
mod-blow = Soprar
mod-draw = Puxar
mod-bend = Dobrar
mod-overblow = Oversopro
mod-overdraw = Overpuxar
mod-slide = Slide
mod-wah = Wah
mod-vibrato = Vibrato
mod-delete = Apagar
editor-tool-select = ✂ ⟕ Selecionar
editor-tool-erase = ✂ Apagar Trecho
editor-tool-remove = ✂ Remover Trecho
editor-tool-tempo = ♩ Tempo

# Editor de Músicas 2 — rótulos dos campos de metadados
editor-field-tempo = Andamento da Música
editor-field-key = Tom do Gaita
editor-field-position = Posição
editor-field-harmonica = Gaita
editor-field-music = Música de Fundo
editor-field-name = Nome
editor-field-author = Autor
editor-field-midi-track = Faixa MIDI
editor-harmonica-diatonic = ‹ Diatônica (10 buracos) ›
editor-harmonica-chromatic = ‹ Cromática (12 buracos) ›
editor-field-content-kind = Gravação
editor-content-kind-song = ‹ Gravar Música ›
editor-content-kind-lesson = ‹ Gravar Lição ›

# Editor de Músicas 2 — campos exclusivos de lição (mostrados enquanto
# "Gravar Lição" está ativo)
editor-field-lesson-id = ID da Lição
editor-field-lesson-unit = Unidade
editor-field-lesson-explanation = Explicação
editor-field-lesson-prerequisites = Pré-requisitos
editor-field-lesson-pass-criteria = Critério de Aprovação
editor-field-lesson-threshold = Limite
editor-field-lesson-technique = Técnica
editor-field-lesson-progression = Progressão

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
editor-confirm-erase = Apagar do compasso %from% ao %to%? Toda nota nesse trecho será apagada — o resto da música fica exatamente onde está.
editor-confirm-remove = Remover do compasso %from% ao %to%? Toda nota nesse trecho será apagada, e tudo depois vai se deslocar pra frente pra fechar o vazio.

# Editor de Músicas 2 — feedback do modo de prática
practice-no-music = Nenhuma música de fundo definida — toque seguindo a partitura!
practice-prompt = ▶ Toque %note%…
practice-wrong-note = ▶ %got% → precisa de %expected%
practice-hit-perfect = ✓ PERFEITO  %note%  +%pts% pts
practice-hit-good = ✓ BOM  %note%  +%pts% pts
practice-missed = ✗ Perdeu %note%
practice-done = Feito — %hits%/%total% notas  ·  %score% pts
editor-record-status = ⏺ Gravando — %count% notas capturadas

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
editor-record-tooltip = Grava as notas tocadas ao vivo na sua gaita direto na grade — clique de novo (ou Parar) para terminar
mod-blow-tooltip = Definir a nota selecionada como sopro (expirar)
mod-draw-tooltip = Definir a nota selecionada como puxada (inspirar)
mod-bend-tooltip = Alternar a profundidade da dobra da nota selecionada: nenhuma → meio tom → tom inteiro → tom e meio
mod-overblow-tooltip = Definir a nota selecionada como oversopro (técnica avançada de sopro, apenas diatônica)
mod-overdraw-tooltip = Definir a nota selecionada como overpuxada (técnica avançada de puxada, apenas diatônica)
mod-slide-tooltip = Definir a nota selecionada para usar o botão slide (apenas gaitas cromáticas)
mod-wah-tooltip = Alternar a taxa de wah-wah da nota selecionada
mod-vibrato-tooltip = Alternar a taxa de vibrato da nota selecionada
mod-delete-tooltip = Apagar a nota selecionada
editor-tool-select-tooltip = Click a point on the timeline then click a side (or click-drag a range)
editor-tool-erase-tooltip = Clique num ponto da linha do tempo e depois num dos lados (ou clique e arraste um trecho) para apagar as notas dali, deixando um vazio
editor-tool-remove-tooltip = Clique num ponto da linha do tempo e depois num dos lados (ou clique e arraste um trecho) para apagar as notas dali e deslocar tudo depois pra frente, fechando o vazio
editor-tool-tempo-tooltip = Clique na régua para adicionar uma mudança de andamento ali, ou clique numa já existente para removê-la
editor-harmonica-toggle-tooltip = Clique para alternar entre gaita Diatônica e Cromática
editor-content-kind-toggle-tooltip = Clique para alternar entre gravar uma música comum e uma lição do currículo
editor-lesson-form-tooltip = Campos do currículo para lesson.json — usados apenas enquanto "Gravar Lição" está ativo
editor-field-lesson-pass-criteria-tooltip = Clique para alternar como esta lição é avaliada — Nenhum, Precisão, Técnica, Aderência à Escala, Aderência a Notas do Acorde, Disciplina de Frase
editor-field-lesson-technique-tooltip = Clique para alternar qual técnica é avaliada — usado apenas quando o Critério de Aprovação é Técnica
editor-field-lesson-progression-tooltip = Clique para alternar a progressão de acompanhamento de uma lição baseada em jam — Nenhuma, Padrão, Quick-Change, Menor
editor-field-key-tooltip = Clique para alternar entre os tons da gaita
editor-field-position-tooltip = Clique para alternar entre as posições de execução
editor-browse-tooltip = Escolher um arquivo de áudio de música de fundo para esta partitura
editor-import-midi-tooltip = Carregar um arquivo MIDI e escolher uma faixa para colocar na grade de notas — Salvar então grava uma trilha de fundo a partir das outras faixas
editor-silence-track-label = Silêncio
editor-silence-track-tooltip = O intervalo, em segundos, entre cada par de notas consecutivas

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
lesson-goal-chord-tone-adherence = Meta: %pct%% das notas como notas do acorde
lesson-goal-phrase-discipline = Meta: %pct%% das notas tocadas fora de uma pausa — deixe espaço
lesson-complete-banner = LIÇÃO CONCLUÍDA
lesson-failed-banner = Meta não atingida — releia a lição e tente de novo

# Lições — títulos das unidades (chave vem do campo "unit" de cada lesson.json)
lesson-unit-blowing = Unidade 1 · Soprando a Gaita
lesson-unit-rhythm = Unidade 2 · Contando o Blues
lesson-unit-blues = Unidade 3 · Vocabulário do Blues

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

# Lição: pergunta e resposta
lesson-question-answer-title = Pergunta e Resposta
lesson-question-answer-body =
    Esta lição não é sobre o que você toca — é sobre o que você não toca. Toque por dois compassos, depois pare de verdade por dois compassos, alternando pela forma inteira.
    Deixar um silêncio de verdade é o ponto: uma frase que recebe uma resposta precisa de espaço pra resposta, e esse espaço só existe se você parar de perguntar.
    É tentador continuar dedilhando durante o descanso — resista. O mapa de furos e seus próprios ouvidos sabem a diferença entre um descanso e uma nota sustentada.
    Esta é a mesma Jam Session aberta das outras lições de improvisação; toque as frases ou escalas que fizerem sentido nos seus dois compassos, depois solte a gaita de verdade.
    Quando sentir que está pronto pra parar, abra o menu de pausa e aperte "Finalizar Lição" — o jogo julga quanto do que você tocou caiu fora dessas janelas de descanso.

# Jogo — contagem regressiva, legenda, dicas do diagrama da harmônica
gameplay-get-ready = PREPARE-SE
gameplay-legend-blow = ■ SOPRO
gameplay-legend-draw = ■ SUGADA
harmonica-overlay-hint-view = Harmônica  ·  acende conforme você toca
harmonica-overlay-hint-select = Harmônica  ·  clique numa nota para selecioná-la
gameplay-chart-info = Tom: %key%  ♩ = %bpm%  %time_sig%
gameplay-chart-author = Partitura: %author%
gameplay-techniques-toggle = %arrow% TÉCNICAS

# Menu de pausa
pause-quit-song = Sair da música
pause-finish-lesson = Concluir lição
pause-wait-for-note-button = ⏸ Esperar nota
pause-wait-for-note-on = Esperar nota: ligado
pause-wait-for-note-off = Esperar nota: desligado
pause-speed = Velocidade: %pct%%
pause-adaptive-difficulty-button = Dificuldade adaptativa
pause-adaptive-difficulty-on = Dificuldade adaptativa: ligada
pause-adaptive-difficulty-off = Dificuldade adaptativa: desligada
pause-phrase-section = Seção: %name% — Aprendido: %pct%%
pause-phrase-no-sections = Nenhuma frase nesta música
pause-drag-section-hint = Clique numa seção na barra de progresso acima para selecioná-la
pause-notes-update-hint = As notas são atualizadas ao vivo — retome para vê-las
pause-clear-loop = Limpar repetição
pause-loop-off = Repetição: desligada
pause-loop-range = Repetição: %start%s–%end%s
pause-drag-loop-hint = Arraste na barra de progresso acima para definir um intervalo de repetição

# Overlay do metrônomo
metronome-click-off = clique: desligado
metronome-click-on = clique: ligado
metronome-feel-straight = ritmo: reto
metronome-feel-shuffle = ritmo: shuffle

# Treinador de Bends
bending-drill-off = Exercício: desligado
bending-drill-on = Exercício: ligado · sequência %streak%
bending-hint = Esc para voltar  ·  M silencia o clique  ·  feel alterna reto/shuffle
bending-no-note-for-technique = Este furo não tem nota para essa técnica.
bending-key-label = Tom: %key%
bending-listen-button = 🔊 Ouvir
bending-drill-button = 🎲 Exercício
bending-play-it-target = Toque — alvo %note%
bending-in-tune = ✓ Afinado  (%note%)
bending-cents-sharp = ↑ %cents% cents agudo  (alvo %note%)
bending-cents-flat = ↓ %cents% cents grave  (alvo %note%)
bending-detect-label = Detectar

# Jam Session
jam-loop-button = ↻ Loop
jam-loop-off = Loop: desligado
jam-loop-on = Loop: ligado
jam-hole-map-hint = Sua harmônica  ·  dourado = tom do acorde agora  ·  verde = nota da escala de blues  ·  sopro em cima / sugada embaixo
jam-call-response-button = 🗣 Pergunta & Resposta
jam-call-response-off = Pergunta & Resposta: desligado
jam-call-response-on = Pergunta & Resposta: ligado
jam-call-response-listen = Escute…
jam-call-response-your-turn = Sua vez

# Tela de resultados
results-song-complete = MÚSICA CONCLUÍDA
results-by-technique = Por técnica
results-new-best = ★ NOVO RECORDE! ★
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
results-increase-latency = Aumentar a latência de entrada para %ms%ms
results-decrease-latency = Diminuir a latência de entrada para %ms%ms
results-score = Pontuação: %points%
results-best-score = Melhor pontuação

# Calibração de latência
calibration-title = Calibração de Latência
calibration-mean-offset-placeholder = Deslocamento médio: —
calibration-mean-offset = Deslocamento médio: %sign%%ms%ms
calibration-suggested-placeholder = Atual: —   →   Sugerido: —
calibration-suggested = Atual: %current%ms   →   Sugerido: %suggested%ms

# Opções
options-input-lag = Atraso de entrada

# Tour guiado do tutorial (menu::tutorial)
tutorial-step = Passo %n% de %total%
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
