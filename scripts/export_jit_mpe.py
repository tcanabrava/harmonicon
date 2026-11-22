# SPDX-License-Identifier: MIT
"""Pack the JIT-MPE S/seed 1 checkpoint for Harmonicon's causal CPU kernels.

Run with numpy and safetensors installed, passing a checkout of
https://github.com/Frieve-A/jit-mpe at 08bbd5acbb223962895f847a86c2d47a36179896.
Only needed when regenerating the bundled model.
"""

import json
from pathlib import Path
import sys

import numpy as np
from safetensors.numpy import load_file

weights = Path(sys.argv[1]) / "weights"
model = load_file(weights / "S-s1.safetensors")
cal = json.loads((weights / "S-s1.calibration.json").read_text())
target = Path(__file__).resolve().parents[1] / "crates/harmonicon-dsp/src/jit_mpe/model.f32"
with target.open("wb") as out:
    def write(*arrays):
        for array in arrays:
            out.write(np.asarray(array, dtype="<f4").tobytes())

    write(model["encoder.affine.mean"], model["encoder.affine.std"])
    for i in range(8):
        prefix = f"encoder.layers.{i}."
        if i == 0:
            write(model[prefix + "weight"].transpose(2, 3, 1, 0), model[prefix + "bias"])
        else:
            write(model[prefix + "dw.weight"][:, 0].transpose(1, 2, 0), model[prefix + "dw.bias"])
            write(model[prefix + "pw.weight"][:, :, 0, 0].T, model[prefix + "pw.bias"])
    write(model["encoder.gather_weight"], model["encoder.gather_bias"] + model["encoder.note_bias"])
    write(model["encoder.interval_weight"], model["encoder.interval_bias"])
    write(model["encoder.interval_out.weight"].T, model["encoder.interval_out.bias"])
    write(model["encoder.norm.weight"], model["encoder.norm.bias"])
    for source in ["ih", "hh"]:
        write(model[f"gru.weight_{source}_l0"].T, model[f"gru.bias_{source}_l0"])
    bias = np.full(88, cal["global"][0], dtype=np.float32)
    for register, values in cal["registers"].items():
        low, high = map(int, register.split("-"))
        bias[low - 21:high - 20] = values[0]
    write(model["presence.weight"][0], model["presence_bias"][:, 0] + model["presence.bias"][0] + bias)
