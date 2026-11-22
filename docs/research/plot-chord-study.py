"""Draw the study figures from three note_bench logs and the paired MAESTRO charts."""
import argparse
import json
import re
from pathlib import Path

import matplotlib.pyplot as plt
from matplotlib.lines import Line2D
from matplotlib.patches import Patch, Rectangle


def save_svg(fig, output):
    fig.savefig(output, metadata={"Date": None})
    plt.close(fig)
    # Matplotlib leaves spaces before newlines inside SVG path coordinates.
    lines = output.read_text(encoding="utf-8").splitlines()
    output.write_text("\n".join(line.rstrip() for line in lines) + "\n", encoding="utf-8")


def scores(path):
    summary = path.read_text().split("== summary: per recording, per detector ==")[1]
    rows = re.findall(r"^\s*(\S+)\s+(FFT|JIT-MPE)\s+notes[^\n]*?chord P/R\s+\d+%/\s*(\d+)%", summary, re.M)
    by_take = {}
    for take, detector, recall in rows:
        by_take.setdefault(take, {})[detector] = int(recall)
    return by_take


def plot_recalls(guitar, keys, reeds, output):
    groups = [
        ("Guitar", [(name.removeprefix("05_").removesuffix("_comp"), pair)
                   for name, pair in sorted(scores(guitar).items())]),
        ("Piano", [(name.split("-", 1)[1], pair) for name, pair in sorted(scores(keys).items())]),
        ("Reeds", [("36 mixtures", scores(reeds)["special20_soft"])]),
    ]
    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 9, "svg.hashsalt": "harmonicon-chords",
                         "svg.fonttype": "none", "axes.unicode_minus": False})
    fig, ax = plt.subplots(figsize=(6.6, 3.08))
    fig.subplots_adjust(left=.26, right=.955, bottom=.17, top=.88)
    blue, orange, ink = "#006a70", "#9c461f", "#213748"
    y = 0
    for group, takes in groups:
        for index, (name, pair) in enumerate(takes):
            if index == 0:
                ax.text(-.33, y, group, transform=ax.get_yaxis_transform(), va="center",
                        weight="bold", color=ink, clip_on=False)
            ax.text(-.025, y, name, transform=ax.get_yaxis_transform(), ha="right",
                    va="center", color=ink, clip_on=False)
            fft, neural = pair["FFT"], pair["JIT-MPE"]
            ax.plot([fft, neural], [y, y], color="#a5b6bd", lw=1.6, zorder=2)
            for value, other, marker, color in [(fft, neural, "o", orange), (neural, fft, "D", blue)]:
                ax.plot(value, y, marker, markersize=5.6, markeredgecolor=color,
                        markerfacecolor="white" if marker == "o" else color, zorder=3)
                # Place labels inward along the connector, keeping 1% clear of row names.
                right = value < other
                ax.annotate(str(value), (value, y), xytext=(5 if right else -5, 0),
                            textcoords="offset points", ha="left" if right else "right", va="center",
                            color=color, fontsize=8, bbox=dict(fc="white", ec="none", pad=.3))
            y += 1
        y += .63
    ax.set(xlim=(0, 100), ylim=(y-1.1, -.6), xticks=range(0, 101, 20), yticks=[],
           xlabel="Exact chord-frame recall (%)")
    ax.xaxis.label.set_color(ink)
    ax.grid(axis="x", color="#dbe1e2", linewidth=.65)
    ax.spines[:].set_visible(False)
    ax.tick_params(axis="x", colors=ink, length=0, pad=4, labelsize=8)
    ax.legend(handles=[Line2D([], [], color=orange, marker="o", mfc="white", ls="", label="FFT"),
                       Line2D([], [], color=blue, marker="D", ls="", label="JIT-MPE")],
              loc="lower right", bbox_to_anchor=(1, 1.01), ncol=2, frameon=False,
              handletextpad=.2, columnspacing=1, borderaxespad=0)
    save_svg(fig, output)


def plot_pedal(maestro, output):
    start, end = 16, 19
    names = "C C# D D# E F F# G G# A A# B".split()
    colors = {"keys": "#006a70", "pedal": "#e2ab74"}
    fig, ax = plt.subplots(figsize=(6.6, 2.65))
    fig.subplots_adjust(left=.105, right=.96, bottom=.19, top=.86)
    ax.set_axisbelow(True)
    for policy in ("pedal", "keys"):
        chart = maestro / policy / "2013-Chopin" / "expected.harpchart"
        events = json.loads(chart.read_text())["track"]
        for event in events:
            onset = max(start, event["time"])
            offset = min(end, event["time"] + event["duration"])
            if onset >= offset:
                continue
            note = event["events"][0]["note"]
            pitch = 12 * (int(note[-1]) + 1) + names.index(note[:-1])
            ax.add_patch(Rectangle((onset, pitch - .43), offset - onset, .86,
                                   edgecolor="none", facecolor=colors[policy]))
    ax.autoscale_view()
    bottom, top = ax.get_ylim()
    ticks = [pitch for pitch in range(24, 109, 12) if bottom <= pitch <= top]
    ax.set(yticks=ticks, yticklabels=[f"C{pitch//12-1}" for pitch in ticks],
           xlim=(start, end), xticks=[16, 16.5, 17, 17.5, 18, 18.5, 19],
           xlabel="Time in Chopin excerpt (s)")
    ax.grid(which="major", color="#dbe1e2", linewidth=.65)
    ax.spines[:].set_visible(False)
    ax.tick_params(length=0, pad=4, colors="#213748")
    ax.legend(handles=[Patch(color=colors["keys"], label="Key held"),
                       Patch(color=colors["pedal"], label="Extended by sustain label")],
              loc="lower left", bbox_to_anchor=(0, 1.015), frameon=False, ncol=2,
              handlelength=1.2, borderaxespad=0)
    save_svg(fig, output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("guitar_log", "keys_log", "reeds_log", "maestro_fixtures", "output_directory"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    args.output_directory.mkdir(exist_ok=True, parents=True)
    plot_recalls(args.guitar_log, args.keys_log, args.reeds_log, args.output_directory / "chord-recall.svg")
    plot_pedal(args.maestro_fixtures, args.output_directory / "piano-sustain.svg")
