// Compile from the repository root with typst compile docs/research/chord-recognition.typ
#set document(title: "Recognizing simultaneous notes in an instrument trainer",
  author: "Harmonicon project", keywords: ("multi-pitch estimation", "piano", "guitar", "harmonica"))
#set page(paper: "a4", margin: (x: 25mm, top: 22mm, bottom: 22mm),
  numbering: "1", number-align: right)
#set text(font: "Libertinus Serif", size: 11pt, lang: "en")
#set par(leading: 0.63em, spacing: 0.8em)
#set heading(numbering: "1.1")
#let ink = rgb("#193e50")
#show heading: set text(font: "TeX Gyre Heros", fill: ink)
#show heading.where(level: 1): set text(size: 16pt)
#show heading.where(level: 2): set text(size: 12pt)
#show link: set text(fill: ink)
#let tab(columns, ..cells) = {
  set text(size: 10pt)
  table(columns: columns, align: (x, y) => if x == 0 {left} else {right},
    inset: (x: 5pt, y: 4pt), stroke: none,
    fill: (x, y) => if y == 0 {rgb("#e6eef1")},
    table.hline(stroke: 0.7pt + ink), ..cells, table.hline(stroke: 0.5pt + ink))
}
#let caption(body) = block(above: 8pt, below: 4pt, text(size: 10pt, body))

#text(font: "TeX Gyre Heros", size: 25pt, weight: "bold", fill: ink)[Recognizing simultaneous notes in an instrument trainer]

#text(size: 15pt)[A comparison of streaming pitch detectors, SoundFont templates, and chord-voicing rules]
#v(3pt)
Harmonicon project · Technical report · 22 November 2026

*Abstract.* A music trainer faces two decisions when a player strums a chord. Which pitches sound, and does the performance satisfy the requested voicing? We investigated both questions using public recordings and reproducible audio replays. On five acoustic-guitar performances by one GuitarSet player, a Rust implementation of the immediate output of the pretrained JIT-MPE model raised pooled frame-level note F1 from 62.8% with spectral peak picking to 84.7%. On three 30-second MAESTRO piano excerpts, exact chord-frame recall rose from 1% on each excerpt to 45%, 14%, and 12% against key-down labels. The ranking reversed on mixtures of isolated recorded harmonica reeds, where exact chord-frame recall was 85% for spectral peak picking and 53% for JIT-MPE. Replacing idealized harmonic weights with SoundFont measurements helped on those reeds and hurt on recorded guitar. A separate change to fretted-chord grading accepted nine of 17 correct guitar trials, up from two, and accepted one of 423 wrong targets, up from zero. These results establish improvements over named Harmonicon baselines. A broader comparison with published transcription systems under matched output deadlines remains open.

= Research questions

An open E-minor guitar chord usually sounds E2, B2, E3, G3, B3, and E4. If E4 decays early, the bass and the minor third can still identify the taught harmony. If G3 disappears instead, the remaining pitches can no longer distinguish E minor from E major. A pitch detector makes an error in either case if all six notes are still annotated. A teacher has a reason to treat the cases differently.

We asked whether a causal learned detector recovers more simultaneous pitches than two spectral baselines, whether sampled timbre transfers from a synthesized preset to a physical instrument, and whether a grader can use the written voicing to recover valid strums without routinely crediting incorrect ones. Each comparison uses the same audio on both sides. The grader has access to the target. The transcription comparisons have access only to the instrument range.

A broader transcription system can also predict which instrument played each note. YourMT3+ uses a transformer and cross-dataset stem augmentation for this task [12]. The immediate question for a player waiting for feedback is narrower. A prediction that recovers the right pitches a second later could help produce a score, yet arrive too late to judge the current strum. We therefore keep the audio timestamp, the decision rule, and the availability of future audio explicit.

#pagebreak()
= Methods

== Pitch estimation and timbre

*Spectral peak picking (FFT).* The baseline locates peaks in a Hann-windowed fast Fourier transform, interpolates their frequencies between bins, and removes candidates near integer multiples of stronger peaks. This can suppress the upper note of a played octave when its frequency coincides with a lower string’s overtone. *Nonnegative matrix factorization (NMF)* fits the spectrum with one template per candidate pitch. The generic dictionary assigns amplitudes proportional to $1/h$ to harmonics $h = 1,…,8$. The detector performs 50 multiplicative updates and returns up to six active notes. Harmonic evidence across partials has a long history in multi-pitch estimation, including Klapuri’s harmonic-summation method [1].

*Streaming neural detector.* We ported Kobayashi’s released JIT-MPE S, seed 1 immediate presence head to Rust [2]. Two asymmetric windows spanning approximately 85 and 341 ms end at the current analysis time. Each contributes 300 log-frequency bands. A harmonic convolutional encoder and recurrent network update every 20 ms. We apply the released register calibration, select active pitches across MIDI 21–108, and refine their frequencies from spectral partials. The bundled model occupies 340,448 bytes. A regression test compares selected Rust logits with PyTorch values to within 0.001 at 44.1 kHz. Our contribution is the streaming integration and its evaluation. Model training and architecture are due to Kobayashi.

*SoundFont experiment.* We rendered dry isolated semitones at MIDI velocities 52, 80, and 108 using FluidSynth and measured the first eight partials over 120–420 ms. For each pitch, we normalized the three profiles by their respective peaks and averaged them. We exchanged only the harmonic weights in a paired offline dictionary fit. Both conditions used the same level gate and six-note limit. GeneralUser GS 2.0.3 and Levi’s Cavaquinho supplied ten presets for harmonica, guitar (acoustic and electric), bass, cavaquinho, piano, flute, clarinet, violin, and viola [3]. Rendered exercises used different velocities (64 and 96, with alternating accents). The recording transfer experiment used harmonica and acoustic guitar.

== Measurement

We replayed mono audio in 4,096-sample chunks with a 2,048-sample hop. At 44.1 kHz these correspond to 92.9 ms and 46.4 ms. For each chunk we queried reference MIDI pitches at its end with zero timing tolerance. *Exact chord-frame recall* is the fraction of frames containing at least two distinct expected pitches for which the detected set equals the reference set, including octaves. A single missing or extra pitch makes a frame incorrect. Note precision and recall count individual frame–pitch pairs. Pooled F1 is the harmonic mean of precision and recall after summing true positives, false positives, and false negatives across recordings. Repeated unisons on two strings count as one pitch. A *trainer acceptance* counts once per clip and requested shape if the grader registers a completed hold. It has a different denominator from frame recall.

#pagebreak()
= Recorded guitar and harmonica

GuitarSet provides aligned note annotations derived with a hexaphonic pickup and a separate microphone signal [4]. We replayed the microphone channel for all five accompaniment takes in the upstream JIT-MPE test split. They cover five progressions played by player 05. The upstream model used other GuitarSet material in training [2]. This tests transfer to different takes from a familiar corpus. The five performances supply 11,913 annotated frame–pitch pairs.

#caption[*Table 1.* GuitarSet transcription. Note metrics pool frame–pitch counts. Exact recall averages the five track percentages printed by the benchmark, giving each performance equal weight.]
#tab((1fr, 0.75fr, 0.75fr, 0.65fr, 0.95fr), [Detector], [Precision], [Recall], [Note F1], [Exact recall],
  [FFT], [54.5%], [74.1%], [62.8%], [12.4%],
  [Generic NMF], [53.2%], [72.9%], [61.5%], [9.6%],
  [JIT-MPE], [81.2%], [88.6%], [84.7%], [41.6%])

The largest neural gain was in precision. Across the five takes, FFT emitted 7,380 spurious pitch observations and missed 3,084. JIT-MPE emitted 2,441 and missed 1,363. Its exact recall ranged from 32% to 71% across takes. Median onset error among detected notes ranged from 36 to 54 ms across takes. This timing measure excludes missed notes and measures alignment in the replay. Device capture and display would add delays in a live session.

For harmonica, we mixed ten individually recorded soft blow reeds from a Hohner Special 20 in C [5]. Eight adjacent triads, seven octaves, and three four-hole voicings were rendered twice, at bass gains of 1.0 and 0.4. These 36 mixtures reuse the same reed recordings. Raw exact chord-frame recall was 85% for FFT, 70% for generic NMF, and 53% for JIT-MPE. FFT’s errors included returning only the lower pitch of a played octave. Simultaneous human breath, reed interaction under changing pressure, draw chords, and bends remain outside this experiment.

#caption[*Table 2.* Timbre transfer. Counts pool exact frames over all annotated chord frames. The two columns use the same offline estimator.]
#tab((1.23fr, 1fr, 1fr), [Audio], [Generic weights], [SoundFont weights],
  [Harmonica reed mixtures], [545/776 (70.2%)], [589/776 (75.9%)],
  [GuitarSet microphone], [325/3,471 (9.4%)], [126/3,471 (3.6%)])

The harmonica profile recovered 44 additional complete frames. The steel-guitar profile lost 199. The weight swap establishes that the template choice matters. Differences in attack, string position, microphone response, or articulation could explain the guitar result, but the experiment varied none of those factors independently. SoundFont renders cover all nine instrument families as regression fixtures. Evidence for transfer to recordings covers the two rows in Table 2.

#pagebreak()
= Piano transcription and guitar grading

== Piano key release and sustain

We took the first 30 seconds of the first, middle, and last entries of the ordered MAESTRO v3.0.0 test split [6]. The recordings are acoustic-piano performances with aligned key and pedal MIDI. We scored identical audio against two references. *Keys* ends a note on MIDI key release. *Pedal* extends released notes while sustain controller CC64 is at least 64, ending them on pedal release or same-pitch reattack. This binary rule approximates sounding duration. It omits decay, partial pedaling, and sostenuto.

#caption[*Table 3.* Exact chord-frame recall on individual piano excerpts, rounded to whole percentages. Each cell gives keys / pedal. The rows are presented in performance-year order.]
#tab((1.6fr, 0.8fr, 0.8fr, 0.9fr), [Performance], [FFT], [Generic NMF], [JIT-MPE],
  [Mozart, K. 280 (2004)], [1% / 1%], [5% / 5%], [45% / 48%],
  [Scriabin, Op. 63 (2009)], [1% / 1%], [1% / 1%], [14% / 6%],
  [Chopin, Op. 53 (2013)], [1% / 4%], [1% / 1%], [12% / 14%])

JIT-MPE led on all three excerpts under both definitions. On Scriabin, adding pedal-held notes raised JIT-MPE note precision from 57% to 87% and lowered recall from 51% to 35%. More detected notes agreed with the longer reference, yet the model missed more of the pitches that reference required. A single rendered GeneralUser piano exercise produced 88% exact recall with JIT-MPE. These 90 seconds of concert audio show how much that estimate can change with repertoire and labeling.

== Fretted-chord acceptance

The original grader required all written pitches within the default six-cent tolerance. We introduced a 25-cent tolerance floor and allowed one missing upper pitch if another expected octave of that pitch class was detected. The written bass must sound. If every written pitch is present, one extra in-tune pitch may sound above the voicing when it lies exactly an octave above a written pitch. The two exceptions cannot be combined. Qualifying simultaneous frames must persist for the configured hold (default 0.5 s). Separate plucks do not accumulate. The rule operates after FFT, NMF, or JIT-MPE.

#caption[*Table 4.* Clip–target acceptances on 55 guitar clips, each scored against eight open shapes. The same 17 matching and 423 mismatched trials were replayed in every condition.]
#tab((1.23fr, 1fr, 1fr), [Detector and rule], [Matching accepted], [Mismatched accepted],
  [FFT, original], [0/17], [0/423], [FFT, revised], [3/17], [0/423],
  [JIT-MPE, original], [2/17], [0/423], [JIT-MPE, revised], [9/17], [1/423])

#pagebreak()
= Interpretation and untested improvements

*Where grading helped.* We inspected the Fender FA-15 recordings during development, including 47 labeled chords and one noise clip [7]. For JIT-MPE, the tolerance and missing-doubling changes together raised accepts from 2/16 to 5/16. Adding the upper-octave exception raised this to 8/16 with 0/368 mismatched accepts. On seven separate nylon-guitar clips [8], the revised rule credited the sole full E-minor upstroke. It also credited a partial C shape as a full C. The played pitches themselves spelled C major. A microphone can identify that harmony while leaving the prescribed fingering ambiguous. The full result in Table 4 still misses eight correct strums. Repeated judgments of one clip and adjacent frames share evidence, so we report descriptive counts. The recordings do not establish a false-acceptance rate for new players or validate the rule on cavaquinho.

*An alternative we tried.* Averaging calibrated outputs from two released model seeds lifted mean exact recall on six development GuitarSet takes from 53.7% to 55.2%. On the five comparison takes, it changed from 41.6% to 41.2%, while requiring two network evaluations. We retained the single seed. The development takes included six players, one of whom also played the five comparison takes. The small differences are descriptive. These exploratory runs give no basis for choosing an ensemble on future players.

*Relation to published methods.* Basic Pitch jointly models onsets, note activity, and multipitch contours [9]. Kong et al. estimate piano onsets, offsets, and pedal events [10]. JIT-MPE explicitly measures the benefit of revising predictions at later output deadlines [2]. Their event-level and deadline-dependent results answer different questions from exact chord frames or credited exercises. Ubisoft reports adjusting chord judgment after Rocksmith+ players identified overly strict passages [11]. That observation motivates measuring both missed and wrongly credited strums. Its public account does not specify an algorithm we could reproduce. We have no matched comparison establishing an advance over the published systems.

*Three experiments that could test for such an advance follow.*

+ *Use the written voicing as a hypothesis.* Compare spectral evidence for the target with evidence for its nearest competitors, including a wrong bass, omitted third, or an added string. Compare against the current rules using identical frozen detector outputs. Set a false-acceptance budget on development players, freeze the threshold, and measure correct accepts on entirely new players. Include the partial-C case as a designated hard negative.
+ *Learn which timbres transfer.* Compare generic templates, the single-bank sustain profiles, and attack/sustain profiles learned from several independent banks. Fine-tuning on real performances is a further untested arm. Hold out banks, performers, and microphones. Test actual breath-played harmonica chords and cavaquinho alongside guitar. Report per-instrument changes with octave and bass errors counted separately.
+ *Model piano decay at a fixed deadline.* Compare the immediate model, a fixed-delay revision, and a pedal-aware tracker using the full MAESTRO test split and both reference policies. Run Basic Pitch and a pedal-aware piano baseline on the same recordings, declaring each model’s future-audio access and training overlap. Report onset/offset F1, exact frame recall by polyphony, and time to usable feedback. Resample whole performances or players for uncertainty. Add a live capture-to-display measurement before drawing conclusions about playability.

#pagebreak()
= Reproducibility and sources

We evaluated Harmonicon revision `f9a4ba39`. It includes the Rust model, SoundFonts with checksums and licenses, and the guitar and reed fixture generators. The accompanying piano script requires the MAESTRO v3.0.0 CSV and the selected WAV/MIDI pairs, using zero-based test rows 0, 88, and 176 in release order. The following commands run from the repository root with Rust, uv, FFmpeg, and FluidSynth available. Replace the MAESTRO input path with the extracted dataset directory. The sound and model assets retain their upstream terms.

#set raw(tab-size: 2)
#block(fill: rgb("#f3f5f6"), inset: 7pt, width: 100%)[
#set text(size: 8pt)
```sh
python scripts/guitarset_chord_benchmark.py /tmp/hc-guitar
uv run --script scripts/vcsl_chord_benchmark.py /tmp/hc-reeds/special20
uv run --script scripts/benchmark_soundfonts.py --fixtures /tmp/hc-synth \
  --recordings harmonica /tmp/hc-reeds --recordings acoustic-guitar /tmp/hc-guitar
uv run --script scripts/maestro_chord_benchmark.py /path/to/maestro-v3.0.0 /tmp/hc-piano
for d in /tmp/hc-guitar /tmp/hc-reeds /tmp/hc-synth /tmp/hc-piano/keys /tmp/hc-piano/pedal; do
  cargo run --bin note_bench -- "$d" 0
done
python scripts/guitar_chord_trainer_benchmark.py /tmp/hc-strums
```
]
The strum command runs the revised grader. Its targets are A, Am, C, D, Dm, E, Em, and G. The original and intermediate counts come from development replays preceding the final rule. The seed ensemble was an exploratory build. All frame scores in Tables 1 and 3 use raw detector outputs. In Table 1, the three pooled counts (true positive, false positive, false negative) are (8,829, 7,380, 3,084) for FFT, (8,680, 7,638, 3,233) for generic NMF, and (10,550, 2,441, 1,363) for JIT-MPE.

#heading(level: 2, numbering: none)[References]
#set text(size: 9.7pt)
#set par(spacing: 0.45em)
+ A. Klapuri (2006). _Multiple Fundamental Frequency Estimation by Summing Harmonic Amplitudes._ ISMIR. #link("https://www.ee.columbia.edu/~dpwe/papers/Klap06-multif0.pdf")[Paper].
+ Y. Kobayashi (2026). _JIT-MPE: Just-in-Time Multi-Pitch Estimation under Output Deadlines._ Preprint. #link("https://arxiv.org/abs/2610.33113")[arXiv:2610.33113]. #link("https://github.com/Frieve-A/jit-mpe/tree/08bbd5acbb223962895f847a86c2d47a36179896")[Model revision].
+ S. Christian Collins, _GeneralUser GS_ 2.0.3. Levi Diniz, _Levi’s Cavaquinho._ #link("https://github.com/mrbumpy409/GeneralUser-GS/tree/684543d5e5efaef08d02be50dcda8d552478fa60")[GeneralUser release]. #link("https://musical-artifacts.com/artifacts/3205")[Cavaquinho].
+ Q. Xi, R. M. Bittner, J. Pauwels, X. Ye, and J. P. Bello (2018). _GuitarSet: A Dataset for Guitar Transcription._ ISMIR. #link("https://doi.org/10.5281/zenodo.3371780")[Dataset v1.1.0].
+ Versilian Community Sample Library. _Hohner Special 20 in C, soft sustains._ #link("https://github.com/sgossner/VCSL/tree/c1ea7bcc3c7309650ab0da9d15c9cd1fbc4a4c7e/Aerophones/Free%20Aerophones/Harmonica-Hohner-Special20-C")[Recording collection].
+ C. Hawthorne et al. (2019). _Enabling Factorized Piano Music Modeling and Generation with the MAESTRO Dataset._ ICLR. #link("https://magenta.tensorflow.org/datasets/maestro")[Dataset v3.0.0 and paper].
+ rodriler. _Isolated Guitar Chords._ #link("https://huggingface.co/datasets/rodriler/isolated-guitar-chords/tree/07371b25b9cdd830638c4ffd5ed8b8a150d7f696")[Dataset revision].
+ SpeedY. _Nylon Guitar Open Chords._ Freesound. #link("https://freesound.org/people/SpeedY/packs/472/")[Recording pack].
+ R. M. Bittner et al. (2022). _A Lightweight Instrument-Agnostic Model for Polyphonic Note Transcription and Multipitch Estimation._ ICASSP. #link("https://arxiv.org/abs/2203.09893")[Paper].
+ Q. Kong, B. Li, X. Song, Y. Wan, and Y. Wang (2021). _High-resolution Piano Transcription with Pedals by Regressing Onset and Offset Times._ IEEE/ACM TASLP. #link("https://arxiv.org/abs/2010.01815")[Paper].
+ Ubisoft (2022). _Rocksmith+ Dev Diary, February 2022: Audio Updates._ #link("https://www.ubisoft.com/en-us/game/rocksmith/plus/news-updates/1Zcwvdcw0IwhBEYoCGCQtt/rocksmith-dev-diary-february-2022-audio-updates")[Developer account].
+ S. Chang et al. (2024). _YourMT3+: Multi-instrument Music Transcription with Enhanced Transformer Architectures and Cross-dataset Stem Augmentation._ IEEE MLSP. #link("https://arxiv.org/abs/2407.04822")[Paper].
