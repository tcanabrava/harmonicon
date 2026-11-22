// SPDX-License-Identifier: MIT
// JIT-MPE S (Kobayashi, 2026). See assets/licenses/JIT-MPE.txt and scripts/export_jit_mpe.py.

use ndarray::{ArrayView2, ArrayViewMut2, linalg::general_mat_mul};
use std::sync::LazyLock;

const BANDS: usize = 300;
const CHANNELS: usize = 24;
const NOTES: usize = 88;
const WIDTH: usize = 64;

// The weights are embedded in capture-independent storage. All streaming state
// belongs to the detector instance, so multiple inputs never share history.
struct Weights {
    mean: Vec<f32>,
    std: Vec<f32>,
    stem: Linear,
    layers: Vec<(Vec<f32>, Vec<f32>, Linear)>,
    gather: Linear,
    interval: Linear,
    residual: Linear,
    scale: Vec<f32>,
    shift: Vec<f32>,
    input_gates: Linear,
    hidden_gates: Linear,
    presence: Linear,
}

struct Linear {
    weights: Vec<f32>,
    bias: Vec<f32>,
    width: usize,
}

impl Linear {
    fn apply(&self, input: &[f32], rows: usize) -> Vec<f32> {
        let inner = self.weights.len() / self.width;
        let mut output = vec![0.; rows * self.width];
        general_mat_mul(
            1.,
            &ArrayView2::from_shape((rows, inner), input).unwrap(),
            &ArrayView2::from_shape((inner, self.width), &self.weights).unwrap(),
            0.,
            &mut ArrayViewMut2::from_shape((rows, self.width), &mut output).unwrap(),
        );
        for (value, bias) in output.iter_mut().zip(self.bias.iter().cycle()) {
            *value += bias;
        }
        output
    }
}

struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Vec<f32> {
        let (bytes, rest) = self.0.split_at(4 * n);
        self.0 = rest;
        bytes.as_chunks::<4>().0.iter().copied().map(f32::from_le_bytes).collect()
    }
    fn linear(&mut self, inputs: usize, width: usize, biases: usize) -> Linear {
        Linear { weights: self.take(inputs * width), bias: self.take(biases), width }
    }
}

static WEIGHTS: LazyLock<Weights> = LazyLock::new(|| {
    let mut r = Reader(include_bytes!("model.f32"));
    let w = Weights {
        mean: r.take(2 * BANDS),
        std: r.take(2 * BANDS),
        stem: r.linear(42, CHANNELS, CHANNELS),
        layers: (0..7)
            .map(|_| {
                (r.take(21 * CHANNELS), r.take(CHANNELS), r.linear(CHANNELS, CHANNELS, CHANNELS))
            })
            .collect(),
        gather: r.linear(18 * CHANNELS, WIDTH, NOTES * WIDTH),
        interval: r.linear(15 * WIDTH, 16, 16),
        residual: r.linear(16, WIDTH, WIDTH),
        scale: r.take(WIDTH),
        shift: r.take(WIDTH),
        input_gates: r.linear(WIDTH, 3 * WIDTH, 3 * WIDTH),
        hidden_gates: r.linear(WIDTH, 3 * WIDTH, 3 * WIDTH),
        presence: r.linear(WIDTH, 1, NOTES),
    };
    assert!(r.0.is_empty());
    w
});

/// Stack frequency-shifted feature rows for the harmonic and note-interval projections.
fn gather(input: &[f32], channels: usize, stride: usize, offsets: &[isize]) -> Vec<f32> {
    let mut out = vec![0.; NOTES * offsets.len() * channels];
    for (note, row) in out.chunks_exact_mut(offsets.len() * channels).enumerate() {
        for (&offset, tap) in offsets.iter().zip(row.chunks_exact_mut(channels)) {
            if let Some(index) = (note * stride).checked_add_signed(offset)
                && index < input.len() / channels
            {
                tap.copy_from_slice(&input[index * channels..][..channels]);
            }
        }
    }
    out
}

/// The initial 3-by-7 time/frequency convolution on both spectral windows.
fn stem_convolution(frames: &[&[f32]; 3], projection: &Linear) -> Vec<f32> {
    let mut taps = vec![0.; BANDS * 42];
    for (t, frame) in frames.iter().enumerate() {
        for (bin, row) in taps.as_chunks_mut::<42>().0.iter_mut().enumerate() {
            for k in 0..7 {
                if (3..BANDS + 3).contains(&(bin + k)) {
                    let source = (bin + k - 3) * 2;
                    row[t * 14 + k * 2..][..2].copy_from_slice(&frame[source..][..2]);
                }
            }
        }
    }
    projection.apply(&taps, BANDS)
}

/// Dilated depthwise convolution followed by a learned channel projection.
fn separable_convolution(
    frames: &[&[f32]; 3],
    dilation: isize,
    kernel: &[f32],
    bias: &[f32],
    pointwise: &Linear,
) -> Vec<f32> {
    let mut depth = bias.repeat(BANDS);
    for (t, frame) in frames.iter().enumerate() {
        for k in 0..7 {
            let offset = (k as isize - 3) * dilation;
            let dst = (-offset).max(0) as usize;
            let src = offset.max(0) as usize;
            let weights = &kernel[(t * 7 + k) * CHANNELS..][..CHANNELS];
            for (row, input) in depth[dst * CHANNELS..]
                .as_chunks_mut::<CHANNELS>()
                .0
                .iter_mut()
                .zip(frame[src * CHANNELS..].as_chunks::<CHANNELS>().0)
            {
                for c in 0..CHANNELS {
                    row[c] += input[c] * weights[c];
                }
            }
        }
    }
    pointwise.apply(&depth, BANDS)
}

pub(super) struct Network {
    // Each layer reads two previous frames plus the current frame.
    history: Vec<[Vec<f32>; 2]>,
    hidden: Vec<f32>,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            history: (0..8)
                .map(|i| {
                    std::array::from_fn(|_| vec![0.; BANDS * if i == 0 { 2 } else { CHANNELS }])
                })
                .collect(),
            hidden: vec![0.; NOTES * WIDTH],
        }
    }
}

impl Network {
    pub(super) fn reset(&mut self) {
        for frames in &mut self.history {
            for frame in frames {
                frame.fill(0.);
            }
        }
        self.hidden.fill(0.);
    }

    pub(super) fn step(&mut self, features: &[f32]) -> Vec<f32> {
        let w = &*WEIGHTS;
        let mut x = vec![0.; 2 * BANDS];
        for channel in 0..2 {
            for bin in 0..BANDS {
                let index = channel * BANDS + bin;
                x[bin * 2 + channel] = (features[index] - w.mean[index]) / w.std[index];
            }
        }
        for (i, cache) in self.history.iter_mut().enumerate() {
            let frames = [&cache[0][..], &cache[1], &x];
            let mut next = if i == 0 {
                stem_convolution(&frames, &w.stem)
            } else {
                let (kernel, bias, pointwise) = &w.layers[i - 1];
                separable_convolution(&frames, 1 << (i - 1), kernel, bias, pointwise)
            };
            for v in &mut next {
                *v = v.max(0.);
            }
            if i > 0 {
                for (v, prev) in next.iter_mut().zip(&x) {
                    *v += prev;
                }
            }
            cache.swap(0, 1);
            cache[1] = x;
            x = next;
        }

        // Third-semitone bands relative to each note, from half the fundamental
        // through its sixteenth harmonic (including the 3/2 interval).
        let offsets =
            [-35, 1, 22, 37, 58, 73, 85, 94, 102, 109, 115, 121, 126, 130, 134, 138, 142, 145];
        let mut notes = w.gather.apply(&gather(&x, CHANNELS, 3, &offsets), NOTES);
        for v in &mut notes {
            *v = v.max(0.);
        }
        let neighbors = [0, 1, -1, 2, -2, 5, -5, 7, -7, 12, -12, 19, -19, 24, -24];
        let mut intervals = w.interval.apply(&gather(&notes, WIDTH, 1, &neighbors), NOTES);
        for v in &mut intervals {
            *v = v.max(0.);
        }
        let residual = w.residual.apply(&intervals, NOTES);
        for (v, r) in notes.iter_mut().zip(residual) {
            *v += r;
        }
        for row in notes.as_chunks_mut::<WIDTH>().0 {
            let mean = row.iter().sum::<f32>() / WIDTH as f32;
            let variance = row.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / WIDTH as f32;
            let inv_std = (variance + 1e-5).sqrt().recip();
            for c in 0..WIDTH {
                row[c] = (row[c] - mean) * inv_std * w.scale[c] + w.shift[c];
            }
        }

        let input = w.input_gates.apply(&notes, NOTES);
        let recurrent = w.hidden_gates.apply(&self.hidden, NOTES);
        for ((row, a), b) in self
            .hidden
            .as_chunks_mut::<WIDTH>()
            .0
            .iter_mut()
            .zip(input.as_chunks::<{ 3 * WIDTH }>().0)
            .zip(recurrent.as_chunks::<{ 3 * WIDTH }>().0)
        {
            for c in 0..WIDTH {
                let reset = sigmoid(a[c] + b[c]);
                let update = sigmoid(a[WIDTH + c] + b[WIDTH + c]);
                let candidate = (a[2 * WIDTH + c] + reset * b[2 * WIDTH + c]).tanh();
                row[c] = candidate + update * (row[c] - candidate);
            }
        }
        w.presence.apply(&self.hidden, NOTES)
    }
}

fn sigmoid(x: f32) -> f32 {
    1. / (1. + (-x).exp())
}
