// Compile with typst compile docs/research/chord-recognition.typ (Typst 0.15).
#set document(title: "Recognizing simultaneous notes in an instrument trainer",
  author: "Harmonicon project", keywords: ("multi-pitch estimation", "piano", "guitar", "harmonica"))
#let ink = rgb("#203847")
#let teal = rgb("#006a70")
#let muted = rgb("#4d646d")
#let line-color = rgb("#c3d2d4")
#let sans = "TeX Gyre Heros"
#set page(paper: "a4", margin: (x: 24mm, top: 22mm, bottom: 22mm),
  footer: context [
    #line(length: 100%, stroke: 0.6pt + line-color)
    #v(1pt)
    #set text(font: sans, size: 8pt, fill: muted)
    Harmonicon project #h(1fr) Chord recognition #h(1fr) #counter(page).display("1 / 1", both: true)
  ])
#set text(font: "Libertinus Serif", size: 11.6pt, lang: "en", fill: ink)
#set par(leading: 0.61em, spacing: 0.75em)
#set heading(numbering: none)
#show heading: set text(font: sans, fill: ink)
#show heading.where(level: 1): it => block(width: 100%, above: 0pt, below: 9pt)[
  #line(length: 24mm, stroke: 2pt + teal)
  #v(3pt)
  #text(font: sans, size: 22pt, weight: "bold", it.body)
]
#show heading.where(level: 2): set text(size: 13pt, fill: teal)
#show link: set text(fill: teal)
#show figure.caption: it => align(left, text(size: 10.2pt, it))
#set figure(gap: 5pt)
#let cite(n) = link(label("ref-" + str(n)), text(font: sans, size: 8.7pt, fill: teal)[\[#n\]])
#let tab(columns, ..cells) = {
  set text(font: sans, size: 9.6pt)
  show table.cell.where(y: 0): set text(weight: "bold", fill: white)
  table(columns: columns, align: (x, y) => if x == 0 {left} else {right},
    inset: (x: 5pt, y: 5pt), stroke: none,
    fill: (x, y) => if y == 0 {ink} else if calc.even(y) {rgb("#eff4f3")},
    table.header(..cells.pos().slice(0, columns.len())),
    ..cells.pos().slice(columns.len()), table.hline(stroke: 0.6pt + line-color))
}
#let caption(body) = block(above: 8pt, below: 4pt, text(size: 10.2pt, body))
#let inset-note(body) = block(width: 100%, inset: (x: 11pt, y: 7pt),
  fill: rgb("#eef5f3"), stroke: (left: 2pt + teal), body)
#text(font: sans, size: 10pt, fill: teal, weight: "bold")[Harmonicon project  /  Research report]
#v(3pt)
#text(font: sans, size: 30pt, weight: "bold", tracking: -0.5pt)[Recognizing simultaneous notes in an instrument trainer]
#v(2pt)
#text(size: 15pt)[Streaming pitch detection, sampled timbre, and fretted-chord grading]
#v(2pt)
#text(font: sans, size: 9pt, fill: muted)[22 November 2026]
#v(4pt)
*Abstract.* A music trainer faces two decisions when a player strums a chord. Which pitches sound, and does the performance satisfy the requested voicing? We investigated both questions using public recordings and reproducible audio replays. On five acoustic-guitar performances by one GuitarSet player, a Rust implementation of the immediate output of the pretrained JIT-MPE model raised pooled frame-level note F1 from 62.8% with spectral peak picking to 84.7%. On three 30-second MAESTRO piano excerpts, exact chord-frame recall rose from 1% on each excerpt to 45%, 14%, and 12% against key-down labels. The ranking reversed on mixtures of isolated recorded harmonica reeds, where exact chord-frame recall was 85% for spectral peak picking and 53% for JIT-MPE. Replacing idealized harmonic weights with SoundFont measurements helped on those reeds and hurt on recorded guitar. A separate change to fretted-chord grading accepted nine of 17 correct guitar trials, up from two, and accepted one of 423 wrong targets, up from zero. These results establish improvements over named Harmonicon baselines. A broader comparison with published transcription systems under matched output deadlines remains open.

#figure(
  image("chord-recall.svg", width: 100%, alt: "Exact chord recall by take. JIT-MPE leads on all five guitar takes and all three piano excerpts. FFT leads on the reed mixtures."),
  caption: [Exact pitch sets recovered at each analysis frame, using only frames with at least two reference pitches. Each guitar row is one take from player 05. Piano uses key-down labels. The reed row pools 36 mixtures of ten recorded reeds.],
  placement: none,
)
#pagebreak()
= What the trainer measures

#inset-note[*Open E minor* #h(1em) E2 · B2 · E3 · G3 · B3 · E4 \
Losing E4 leaves both the bass and minor third audible. Losing G3 removes the only pitch that distinguishes this voicing from E major. Both omissions are transcription errors. They carry different consequences for a chord exercise.]

== Pitch estimation and timbre

*Spectral peak picking (FFT).* The baseline locates peaks in a Hann-windowed fast Fourier transform, interpolates their frequencies between bins, and removes candidates near integer multiples of stronger peaks. This can suppress the upper note of a played octave when its frequency coincides with a lower string’s overtone. *Nonnegative matrix factorization (NMF)* fits the spectrum with one template per candidate pitch. The generic dictionary assigns amplitudes proportional to $1/h$ to harmonics $h = 1,…,8$. The detector performs 50 multiplicative updates and returns up to six active notes. Harmonic evidence across partials has a long history in multi-pitch estimation, including Klapuri’s harmonic-summation method #cite(1).

*Streaming neural detector.* We ported Kobayashi’s released JIT-MPE S, seed 1 immediate presence head to Rust #cite(2). Two asymmetric windows spanning approximately 85 and 341 ms end at the current analysis time. Each contributes 300 log-frequency bands. A harmonic convolutional encoder and recurrent network update every 20 ms. We apply the released register calibration, select active pitches across MIDI 21–108, and refine their frequencies from spectral partials. Re-exporting the pinned upstream weights and calibration reproduced the bundled 340,448 bytes exactly. A regression test compares selected Rust logits with PyTorch values to within 0.001 at 44.1 kHz. Our contribution is the streaming integration and its evaluation. Model training and architecture are due to Kobayashi.

*SoundFont experiment.* We rendered dry isolated semitones at MIDI velocities 52, 80, and 108 using FluidSynth and measured the first eight partials over 120–420 ms. For each pitch, we normalized the three profiles by their respective peaks and averaged them. We exchanged only the harmonic weights in a paired offline dictionary fit. Both conditions used the same level gate and six-note limit. GeneralUser GS 2.0.3 and Levi’s Cavaquinho supplied ten presets for harmonica, guitar (acoustic and electric), bass, cavaquinho, piano, flute, clarinet, violin, and viola #cite(3). Rendered exercises used different velocities (64 and 96, with alternating accents). The recording transfer experiment used harmonica and acoustic guitar.

== Measurement

We replayed mono audio in 4,096-sample chunks with a 2,048-sample hop. At 44.1 kHz these correspond to 92.9 ms and 46.4 ms. For each chunk we queried reference MIDI pitches at its end with zero timing tolerance. *Exact chord-frame recall* is the fraction of frames containing at least two distinct expected pitches for which the detected set equals the reference set, including octaves. A single missing or extra pitch makes a frame incorrect. Note precision and recall count individual frame–pitch pairs. Pooled F1 is the harmonic mean of precision and recall after summing true positives, false positives, and false negatives across recordings. Repeated unisons on two strings count as one pitch. A *trainer acceptance* counts once per clip and requested shape if the grader registers a completed hold. Free transcription receives audio and instrument range. Only the exercise grader receives the target pitches.

#pagebreak()
= Recorded guitar and harmonica

GuitarSet provides aligned note annotations derived with a hexaphonic pickup and a separate microphone signal #cite(4). We replayed the microphone channel for all five accompaniment takes in the upstream JIT-MPE test split. They cover five progressions played by player 05. The upstream split code assigns players 00–03 to training, 04 to development, and 05 to test, using disjoint progression IDs. Its paper also uses clips from its separate GuitarSet development partition to select delay heads. Our five takes come from the code’s test partition of a corpus familiar to the model #cite(2). The five performances supply 11,913 annotated frame–pitch pairs.

#caption[*Table 1.* GuitarSet transcription. Note metrics pool frame–pitch counts. Exact recall averages the five track percentages printed by the benchmark, giving each performance equal weight.]
#tab((1fr, 0.75fr, 0.75fr, 0.65fr, 0.95fr), [Detector], [Precision], [Recall], [Note F1], [Exact recall],
  [FFT], [54.5%], [74.1%], [62.8%], [12.4%],
  [Generic NMF], [53.2%], [72.9%], [61.5%], [9.6%],
  [JIT-MPE], [81.2%], [88.6%], [84.7%], [41.6%])

The largest neural gain was in precision. Across the five takes, FFT emitted 7,380 spurious pitch observations and missed 3,084. JIT-MPE emitted 2,441 and missed 1,363. Its exact recall ranged from 32% to 71% across takes. Median onset error among detected notes ranged from 36 to 54 ms across takes. This timing measure excludes missed notes and measures alignment in the replay. Device capture and display would add delays in a live session.

For harmonica, we mixed ten individually recorded soft blow reeds from a Hohner Special 20 in C #cite(5). Eight adjacent triads, seven octaves, and three four-hole voicings were rendered twice, at bass gains of 1.0 and 0.4. These 36 mixtures reuse the same reed recordings. Raw exact chord-frame recall was 85% for FFT, 70% for generic NMF, and 53% for JIT-MPE. FFT’s errors included returning only the lower pitch of a played octave. Simultaneous human breath, reed interaction under changing pressure, draw chords, and bends remain outside this experiment.

#caption[*Table 2.* Timbre transfer. Counts pool exact frames over all annotated chord frames. The two columns use the same offline estimator.]
#tab((1.23fr, 1fr, 1fr), [Audio], [Generic weights], [SoundFont weights],
  [Harmonica reed mixtures], [545/776 (70.2%)], [589/776 (75.9%)],
  [GuitarSet microphone], [325/3,471 (9.4%)], [126/3,471 (3.6%)])

The harmonica profile recovered 44 additional complete frames. The steel-guitar profile lost 199. The weight swap establishes that the template choice matters. Differences in attack, string position, microphone response, or articulation could explain the guitar result, but the experiment varied none of those factors independently. SoundFont renders cover all nine instrument families as regression fixtures. Evidence for transfer to recordings covers the two rows in Table 2.

*Two-seed ensemble.* Mean exact recall on six development takes rose from 53.7% to 55.2% when we averaged calibrated outputs from two seeds. On the five comparison takes it fell from 41.6% to 41.2%. One guitarist appeared in both sets. We retained the single seed.

#pagebreak()
= Piano key release and sustain

We took the first 30 seconds of the first, middle, and last entries of the ordered MAESTRO v3.0.0 test split #cite(6). The recordings are acoustic-piano performances with aligned key and pedal MIDI. We scored identical audio against two references. *Keys* ends a note on MIDI key release. *Pedal* extends released notes while sustain controller CC64 is at least 64, ending them on pedal release or same-pitch reattack. This binary rule approximates sounding duration. It omits decay, partial pedaling, and sostenuto.

#caption[*Table 3.* Exact chord-frame recall on individual piano excerpts, rounded to whole percentages. Each cell gives keys / pedal. The rows are presented in performance-year order.]
#tab((1.6fr, 0.8fr, 0.8fr, 0.9fr), [Performance], [FFT], [Generic NMF], [JIT-MPE],
  [Mozart, K. 280 (2004)], [1% / 1%], [5% / 5%], [45% / 48%],
  [Scriabin, Op. 63 (2009)], [1% / 1%], [1% / 1%], [14% / 6%],
  [Chopin, Op. 53 (2013)], [1% / 4%], [1% / 1%], [12% / 14%])

JIT-MPE led on all three excerpts under both definitions. On Scriabin, adding pedal-held notes raised JIT-MPE note precision from 57% to 87% and lowered recall from 51% to 35%. More detected notes agreed with the longer reference, yet the model missed more of the pitches that reference required. A single rendered GeneralUser piano exercise yielded 88% exact recall for JIT-MPE. The concert sample covers only 90 seconds.

#figure(
  image("piano-sustain.svg", width: 100%, alt: "Piano roll from 16 to 19 seconds of the Chopin excerpt, with extended sustain labels spanning the gaps between successive key presses."),
  caption: [Two references for the same three seconds of Chopin. Each horizontal segment is a pitch. A sustain extension starts when a key lifts while the pedal is down. These are MIDI-derived labels. The acoustic decay of the strings was not measured.],
)

#inset-note[*A hard limit on dense piano.* With pedal labels, 9/602 Mozart, 217/559 Scriabin, and 276/508 Chopin chord frames contain more than six distinct expected pitches. A six-output NMF detector cannot recover these sets, even with perfect templates. Under key-down labels the corresponding counts are 0/568, 0/407, and 17/366. Even perfect six-note choices could recover at most 232/508 (45.7%) of Chopin’s pedal-labeled chord frames. The cap alone cannot explain the observed 1% recall.]

#pagebreak()
= What counts as a correct strum

The original grader required all written pitches within the default six-cent tolerance. We introduced a 25-cent tolerance floor and allowed one missing upper pitch if another expected octave of that pitch class was detected. The written bass must sound. If every written pitch is present, one extra in-tune pitch may sound above the voicing when it lies exactly an octave above a written pitch. The two exceptions cannot be combined. Qualifying simultaneous frames must persist for the configured hold (default 0.5 s). Separate plucks do not accumulate. The rule operates after FFT, NMF, or JIT-MPE.

#caption[*Table 4.* Clip–target acceptances on 55 guitar clips, each scored against eight open shapes. The same 17 matching and 423 mismatched trials were replayed in every condition.]
#tab((1.23fr, 1fr, 1fr), [Detector and rule], [Matching accepted], [Mismatched accepted],
  [FFT, original], [0/17], [0/423], [FFT, revised], [3/17], [0/423],
  [JIT-MPE, original], [2/17], [0/423], [JIT-MPE, revised], [9/17], [1/423])

*Where grading helped.* We inspected the Fender FA-15 recordings during development, including 47 labeled chords and one noise clip #cite(7). For JIT-MPE, the tolerance and missing-doubling changes together raised accepts from 2/16 to 5/16. Adding the upper-octave exception raised this to 8/16 with 0/368 mismatched accepts. On seven separate nylon-guitar clips #cite(8), the revised rule credited the sole full E-minor upstroke. It also credited a partial C shape as a full C. The full result in Table 4 still misses eight correct strums. Repeated judgments of one clip and adjacent frames share evidence, so we report descriptive counts. The recordings do not establish a false-acceptance rate for new players or validate the rule on cavaquinho.

== A partial C that earned credit

#tab((1.4fr, 1fr, 1fr, 1fr, 1fr, 1fr), [Sounding string], [5 · A], [4 · D], [3 · G], [2 · B], [1 · E],
  [Taught shape], [C3], [E3], [G3], [C4], [E4],
  [Played strings], [C3], [E3], [G3], [silent], [silent])

#v(3pt)
The partial shape contains the bass, third, and fifth but omits two upper pitches. Presented with just C3, E3, and G3, the revised grader would reject the trial because it allows one missing doubling. The replay credited the clip after pitch detection. Its clip-level score does not establish which upper pitch the detector reported at the qualifying instant. Freesound identifies the strings played #cite(8).

#pagebreak()
= What the published work offers

*FFT and octave errors.* Klapuri scores a candidate by summing its weighted harmonics in a whitened spectrum. One variant estimates a pitch, subtracts its estimated contribution, and scores the residual #cite(1). Harmonicon currently drops peaks close to stronger integer multiples. Compare the shipped rule with harmonic summation alone and then with cancellation, at the same hop and instrument range. Score true upper octaves and spurious octaves separately on the seven recorded-reed octave pairs and on guitar.

*Piano attack, release, and pedal.* Basic Pitch predicts onset and frame activity as separate outputs #cite(9). Kong et al. regress onset and offset times and infer sustain-pedal events #cite(10). Compare a pedal-aware model with the existing presence head against both MAESTRO reference policies #cite(6). Stratify by polyphony, especially above six pitches. Score repeated-note onsets, key releases, and pedal releases separately. For a live comparison, log how much future audio each method actually consumed.

*When to revise a prediction.* JIT-MPE estimates the same frame again after 20, 40, 80, and 160 ms of additional audio #cite(2). Test its released 40-ms revision alongside the shipped immediate head. Preserve the first output and timestamp any correction for the same frame. Compare exact recall and time to correct feedback at matched deadlines. Blending two outputs into the current frame would test a different hypothesis.

*Grading the written voicing.* Ubisoft’s lead audio designer describes giving the bass root and the third particular weight, and listening for color tones such as sevenths #cite(11). Test the written shape against a wrong bass, missing third, and partial-C competitor using the same detector output. Freeze a false-acceptance budget on development players, then evaluate new players and instruments. The diary describes the musical criteria and leaves the signal-processing implementation unspecified.

*Learning timbre.* YourMT3+ uses cross-dataset stem augmentation for multi-instrument transcription #cite(12). For a single-instrument trainer, test the narrower idea of mixing attack and sustain examples from several banks, holding one bank, performer, and microphone out. Evaluate on actual breath-played harmonica and cavaquinho chords as well as strummed guitar. Whether any of these changes improves Harmonicon under a live deadline remains an experimental question.

#pagebreak()
= Reproducing the measurements

We evaluated Harmonicon revision `f9a4ba39`. It includes the Rust model, SoundFonts with checksums and licenses, and the guitar and reed fixture generators. The accompanying piano script requires the MAESTRO v3.0.0 CSV and the selected WAV/MIDI pairs, using zero-based test rows 0, 88, and 176 in release order. The following commands run from the repository root with Rust, uv, FFmpeg, and FluidSynth available. Replace the MAESTRO input path with the extracted dataset directory. The sound and model assets retain their upstream terms.

#set raw(tab-size: 2)
#block(fill: rgb("#f3f5f6"), inset: 7pt, width: 100%)[
#set text(size: 9pt)
```sh
python scripts/guitarset_chord_benchmark.py /tmp/hc-guitar
uv run --script scripts/vcsl_chord_benchmark.py /tmp/hc-reeds/special20
uv run --script scripts/benchmark_soundfonts.py --fixtures /tmp/hc-synth \
  --recordings harmonica /tmp/hc-reeds --recordings acoustic-guitar /tmp/hc-guitar
uv run --script scripts/maestro_chord_benchmark.py \
  /path/to/maestro-v3.0.0 /tmp/hc-piano
for d in /tmp/hc-guitar /tmp/hc-reeds /tmp/hc-synth \
         /tmp/hc-piano/keys /tmp/hc-piano/pedal; do
  cargo run --bin note_bench -- "$d" 0
done
python scripts/guitar_chord_trainer_benchmark.py /tmp/hc-strums
```
]
The strum command runs the revised grader. Its targets are A, Am, C, D, Dm, E, Em, and G. The original and intermediate counts come from development replays preceding the final rule. The seed ensemble was an exploratory build.

== Rebuilding the figures

Save the guitar, piano key-down, and reed `note_bench` reports as text files. The plotting script reads their per-recording summaries and the two Chopin charts. With Matplotlib and Typst installed, run

```sh
python docs/research/plot-chord-study.py guitar.log keys.log reeds.log \
  /tmp/hc-piano docs/research
typst compile docs/research/chord-recognition.typ
```

For the piano density counts, step from sample 4,096 through the end of the file in increments of 2,048. At each window end count distinct pitches whose annotated interval includes that time, including the start and excluding the end. The denominator counts frames with at least two pitches and the numerator counts frames with more than six.

All frame scores in Tables 1 and 3 use raw detector outputs. In Table 1, the three pooled counts (true positive, false positive, false negative) are (8,829, 7,380, 3,084) for FFT, (8,680, 7,638, 3,233) for generic NMF, and (10,550, 2,441, 1,363) for JIT-MPE.

#pagebreak()
= References and recording sources
#set text(size: 11pt)
#let entry(n, body) = block(width: 100%, above: 9pt, below: 7pt)[
  #metadata(n) #label("ref-" + str(n))
  #grid(columns: (22pt, 1fr), text(font: sans, weight: "bold", fill: teal, str(n)), body)
]
#entry(1)[A. Klapuri (2006). _Multiple Fundamental Frequency Estimation by Summing Harmonic Amplitudes._ ISMIR. #link("https://archives.ismir.net/ismir2006/paper/000125.pdf")[Paper].]
#entry(2)[Y. Kobayashi (2026). _JIT-MPE: Just-in-Time Multi-Pitch Estimation under Output Deadlines._ arXiv preprint. #link("https://arxiv.org/abs/2610.33113")[arXiv:2610.33113]. #link("https://github.com/Frieve-A/jit-mpe/tree/08bbd5acbb223962895f847a86c2d47a36179896")[Code and weights]. #link("https://github.com/Frieve-A/jit-mpe/blob/08bbd5acbb223962895f847a86c2d47a36179896/jit_mpe/data/splits.py")[Split definition].]
#entry(3)[S. Christian Collins, _GeneralUser GS_ 2.0.3. Levi Diniz, _Levi’s Cavaquinho._ #link("https://github.com/mrbumpy409/GeneralUser-GS/tree/684543d5e5efaef08d02be50dcda8d552478fa60")[GeneralUser release]. #link("https://musical-artifacts.com/artifacts/3205.json")[Cavaquinho catalog record]. #link("https://web.archive.org/web/20260607050956id_/https://musical-artifacts.com/artifacts/3205/Cavaco.sf2")[Archived bank used].]
#entry(4)[Q. Xi, R. M. Bittner, J. Pauwels, X. Ye, and J. P. Bello (2018). _GuitarSet: A Dataset for Guitar Transcription._ ISMIR. #link("https://doi.org/10.5281/zenodo.3371780")[Dataset v1.1.0]. #link("https://ismir2018.ismir.net/doc/pdfs/188_Paper.pdf")[Proceedings].]
#entry(5)[Versilian Community Sample Library. _Hohner Special 20 in C, soft sustains._ #link("https://github.com/sgossner/VCSL/tree/c1ea7bcc3c7309650ab0da9d15c9cd1fbc4a4c7e/Aerophones/Free%20Aerophones/Harmonica-Hohner-Special20-C")[Recording collection].]
#entry(6)[C. Hawthorne et al. (2019). _Enabling Factorized Piano Music Modeling and Generation with the MAESTRO Dataset._ ICLR. #link("https://magenta.tensorflow.org/datasets/maestro")[Dataset v3.0.0 and paper].]
#entry(7)[rodriler (Hugging Face account). _Isolated Guitar Chords Dataset._ #link("https://huggingface.co/datasets/rodriler/isolated-guitar-chords/tree/07371b25b9cdd830638c4ffd5ed8b8a150d7f696")[Dataset revision].]
#entry(8)[SpeedY (2005). _Nylon Guitar Open Chords._ Freesound. #link("https://freesound.org/people/SpeedY/packs/472/")[Recording pack]. #link("https://freesound.org/people/SpeedY/sounds/8470/")[Partial C]. #link("https://freesound.org/people/SpeedY/sounds/8493/")[Full E minor].]
#entry(9)[R. M. Bittner et al. (2022). _A Lightweight Instrument-Agnostic Model for Polyphonic Note Transcription and Multipitch Estimation._ ICASSP. #link("https://doi.org/10.1109/ICASSP43922.2022.9746549")[Proceedings]. #link("https://arxiv.org/abs/2203.09893")[Open paper].]
#entry(10)[Q. Kong, B. Li, X. Song, Y. Wan, and Y. Wang (2021). _High-resolution Piano Transcription with Pedals by Regressing Onset and Offset Times._ IEEE/ACM TASLP. #link("https://doi.org/10.1109/TASLP.2021.3121991")[Journal article]. #link("https://arxiv.org/abs/2010.01815")[Open paper].]
#entry(11)[B. Poedy (2022). Ubisoft. _Rocksmith+ Dev Diary, February 2022: Audio Updates._ #link("https://www.ubisoft.com/en-us/game/rocksmith/plus/news-updates/1Zcwvdcw0IwhBEYoCGCQtt/rocksmith-dev-diary-february-2022-audio-updates")[Developer account].]
#entry(12)[S. Chang et al. (2024). _YourMT3+: Multi-instrument Music Transcription with Enhanced Transformer Architectures and Cross-dataset Stem Augmentation._ IEEE MLSP. #link("https://doi.org/10.1109/MLSP58920.2024.10734819")[Proceedings]. #link("https://arxiv.org/abs/2407.04822")[Open paper].]

#v(5pt)
#inset-note[*Source verification.* The five published research papers and conference articles #cite(1) #cite(4) #cite(9) #cite(10) #cite(12), the MAESTRO paper and dataset #cite(6), and the JIT-MPE preprint #cite(2) describe different experiments. The numerical results in this report come from Harmonicon replays and its reference charts. For the bundled cavaquinho, the archived file matches the local SHA-256 recorded in `assets/soundfonts/README.md`. The current catalog file has a different hash #cite(3).]
