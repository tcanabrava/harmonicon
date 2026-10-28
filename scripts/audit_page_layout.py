#!/usr/bin/env python3
"""Check every menu page's layout against the window, over BRP.

Catches the class of fault that made the skill tree's drag-to-pan dead: a
node with `min-width/height: auto` holding content bigger than the viewport
grows *past* the viewport instead of letting the scroll area inside it
overflow. The page then has no scrollable range at all — no scrollbar
appears and every drag clamps to zero — and the part of the page beyond the
window edge is simply unreachable.

Two things are reported per page:

  oversized  a node bigger than the window. The scroll-area *content* is
             legitimately oversized (that is the point of scrolling), so
             only nodes that are NOT inside a scrolling ancestor count.
  dead-scroll a node with `Overflow::Scroll` whose content_size does not
             exceed its own size, i.e. it can never scroll. Suspicious
             when the page visibly has more content than fits.

Needs a running `--features dev` build (scripts/run-dev.sh).

    python3 scripts/audit_page_layout.py [Page ...]

Menu pages are reached by writing `NextState<MenuPage>`. The gameplay
screens can't be — Play 2D, Jam Session and Results all want a
`SelectedSong` first, and that holds an asset handle no JSON value can
express — so they are reached the way a player reaches them, by clicking
through, using `brpctl.py`'s routes. Results additionally needs a song to
*finish*, so it plays the shortest bundled chart to its end. The Song Editor
is state-reachable but empty until a chart is loaded through its file
dialog, so it goes the same way.
"""

import json
import os
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import brpctl  # noqa: E402  (sibling script, same directory)

URL = "http://127.0.0.1:15702"

PAGES = [
    "Main", "Welcome", "Play", "ArtistList", "HarpCheck", "Options",
    "Theme", "LessonTree", "JamSessionMenu", "JamGenerate", "HelpAbout",
]

# Screens that need a `SelectedSong`: name → how to get there from the main
# menu. Checked after `PAGES`, and each starts from `home` because a song
# can end and move to Results on its own schedule.
GAMEPLAY = {
    "Play2D": lambda: brpctl.enter_song("Traditional", "Amazing Grace"),
    "JamSession": lambda: brpctl.enter_jam("Traditional", "Amazing Grace"),
    "SongEditor": lambda: brpctl.enter_editor("Traditional", "Amazing Grace"),
    "Results": lambda: (
        brpctl.enter_song(*brpctl.SHORTEST_SONG),
        brpctl.wait_for_results(),
    ),
}

COMPUTED = "bevy_ui::ui_node::ComputedNode"
NODE = "bevy_ui::ui_node::Node"
CHILD_OF = "bevy_ecs::hierarchy::ChildOf"


def rpc(method, params):
    req = urllib.request.Request(
        URL,
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=10) as r:
        body = json.load(r)
    if "error" in body:
        raise RuntimeError(body["error"])
    return body.get("result")


def goto(page):
    # Same shape `brpctl.set_state` uses: `mutate_resources` with an empty
    # path, and `NextState` under `resources` (not `states`, which resolves
    # to nothing). See contributing/src/remote-control.md.
    rpc("world.mutate_resources", {
        "resource":
            "bevy_state::state::resources::NextState"
            "<harmonicon_menu::menu::routing::MenuPage>",
        "path": "",
        "value": {"Pending": page},
    })


def ui_nodes():
    # `ChildOf` goes under `option`, not `has`: `has` reports mere presence
    # as a bool, so the ancestor walk below would never see a parent and
    # every scroll-area child would look like unreachable content.
    rows = rpc("world.query", {
        "data": {"components": [COMPUTED, NODE], "option": [CHILD_OF]},
        "filter": {"with": [COMPUTED]},
    })
    out = {}
    for r in rows:
        c = r["components"]
        cn, n = c.get(COMPUTED) or {}, c.get(NODE) or {}
        out[r["entity"]] = {
            "size": cn.get("size") or [0.0, 0.0],
            "content": cn.get("content_size") or [0.0, 0.0],
            "parent": c.get(CHILD_OF),
            "scrolls": scrolls(n),
            "clips": clips(n),
        }
    return out


def scrolls(node):
    ov = node.get("overflow") or {}
    return ov.get("x") == "Scroll" or ov.get("y") == "Scroll"


def clips(node):
    """Whether this node bounds its children at all — scrolled *or* clipped.

    `Overflow::Clip` counts: the credits screen deliberately animates a
    1344x1799 column's `top` inside a `clip_y()` overlay, so its oversized
    content is reachable by design and must not read as a fault.
    """
    ov = node.get("overflow") or {}
    return any(ov.get(ax) in ("Scroll", "Clip", "Hidden") for ax in ("x", "y"))


def inside_scroller(nodes, e):
    """Whether any ancestor of `e` bounds it (scroll or clip)."""
    seen = set()
    p = nodes.get(e, {}).get("parent")
    while isinstance(p, int) and p not in seen:
        seen.add(p)
        if nodes.get(p, {}).get("clips"):
            return True
        p = nodes.get(p, {}).get("parent")
    return False


def audit(page, window, slack=1.0):
    if page in GAMEPLAY:
        brpctl.to_main_menu()
        GAMEPLAY[page]()
    else:
        goto(page)
    time.sleep(1.2)
    nodes = ui_nodes()

    oversized, starved = [], []
    for e, info in nodes.items():
        w, h = info["size"]
        # Unreachable content: bigger than the window, with no scrolling
        # ancestor to reach the rest of it through.
        if (w > window[0] + slack or h > window[1] + slack) and not inside_scroller(nodes, e):
            oversized.append((e, info["size"]))
        # A scroller whose own subtree is wider/taller than it is, but
        # whose content_size does not say so, can never scroll to it —
        # the skill tree's exact failure.
        if info["scrolls"] and any(d > s + slack for d, s in zip(subtree_extent(nodes, e), info["size"])):
            room = [round(c - s, 1) for c, s in zip(info["content"], info["size"])]
            if room[0] <= 0 and room[1] <= 0:
                starved.append((e, info["size"], info["content"]))

    status = "ok" if not (oversized or starved) else "FAIL"
    print(f"{page:<15} {status:<6} nodes={len(nodes)}")
    for e, size in oversized[:4]:
        print(f"    oversized   {e}: size={size} > window {[round(v) for v in window]}")
    for e, size, content in starved[:4]:
        print(f"    unscrollable {e}: size={size} content={content} (subtree is larger)")
    return not (oversized or starved)


def subtree_extent(nodes, root):
    """Largest width/height among `root`'s descendants."""
    children = {}
    for e, info in nodes.items():
        p = info.get("parent")
        if isinstance(p, int):
            children.setdefault(p, []).append(e)
    best, stack, seen = [0.0, 0.0], list(children.get(root, [])), set()
    while stack:
        e = stack.pop()
        if e in seen:
            continue
        seen.add(e)
        s = nodes.get(e, {}).get("size") or [0.0, 0.0]
        best = [max(best[0], s[0]), max(best[1], s[1])]
        stack.extend(children.get(e, []))
    return best


def main():
    pages = sys.argv[1:] or (PAGES + list(GAMEPLAY))
    win = rpc("world.query", {
        "data": {"components": ["bevy_window::window::Window"]},
        "filter": {"with": ["bevy_window::window::Window"]},
    })
    res = win[0]["components"]["bevy_window::window::Window"]["resolution"]
    # `ComputedNode::size` is in *physical* pixels, so compare against the
    # window's physical resolution — dividing by the scale factor first
    # makes every correctly-sized root node look oversized.
    window = [float(res["physical_width"]), float(res["physical_height"])]
    print(f"window (physical px): {[round(v) for v in window]}\n")

    bad, skipped = [], []
    for page in pages:
        try:
            if not audit(page, window):
                bad.append(page)
        except Exception as exc:  # a page needing a prior selection, etc.
            print(f"{page:<15} SKIPPED    ({exc})")
            skipped.append(page)

    # Leave the game on the main menu, not mid-song, for whoever runs next.
    if any(page in GAMEPLAY for page in pages):
        brpctl.to_main_menu()

    print()
    if bad:
        print(f"pages exceeding the window: {', '.join(bad)}")
    elif len(skipped) == len(pages):
        print("every page skipped — nothing was actually checked")
    else:
        print(f"{len(pages) - len(skipped)} pages fit the window")
    if skipped:
        print(f"not checked: {', '.join(skipped)}")
    # A run that checked nothing is a failure, not a pass.
    return 1 if (bad or len(skipped) == len(pages)) else 0


if __name__ == "__main__":
    sys.exit(main())
