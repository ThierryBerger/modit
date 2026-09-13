#!/usr/bin/env python3
"""Assemble an mdBook source tree out of the markdown already in this repository.

The site has **no content of its own**. Every page it publishes is a file that is
also read on GitHub and in an editor, and this script only arranges copies of
them:

- the markdown is staged at the *same relative paths* it has in the repository
  (`docs/`, `plans/`), so every cross-link between those files keeps working
  untouched -- that is the whole reason the layout is preserved rather than
  flattened;
- links that leave those directories point at source code, which a documentation
  site has no copy of, so they are rewritten to GitHub blob URLs;
- `SUMMARY.md` -- mdBook's table of contents, and the one file that does not
  exist in the repository -- is generated here, with every chapter title read out
  of the page's own `# ` heading rather than retyped.

Nothing is written outside the output directory, and the output directory is
disposable: `just site` recreates it.

Usage:  python3 docs/site/stage.py [--out target/site/src]
"""

import argparse
import os
import posixpath
import re
import shutil
import sys

# Where a link into the source tree should point instead. The site publishes
# prose; code is read on GitHub.
BLOB = "https://github.com/Vrixyz/modit/blob/main/"

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Directories staged wholesale, in their repository layout.
TREES = ("docs", "plans")
# Single files staged at the top level. None today: everything the site publishes
# lives in one of the trees above.
FILES = ()
# Not staged: this script and the schematic generator are tools, not pages.
#
# `README.md` is deliberately absent too. It is the *repository's* front door --
# crate table, build commands, BLE trade-offs -- while the site's front door is
# `docs/index.md`, written for someone who has not opened a terminal. Staging both
# would put two front doors in one navigation, and mdBook would turn `README.md`
# into the site's `index.html` and bury the real landing page. Links to it are
# rewritten to GitHub like any other link that leaves the site.
SKIP = ("docs/site", "docs/hardware/schematics.py")

# Pages that move when staged, beyond the `README.md` rule below.
#
# mdBook renders the first chapter twice: at its own path, and as the site's root
# `index.html`. That second copy keeps the *chapter's* relative links, so a
# landing page living one directory down produces a root page whose every link is
# broken by one level. The fix is for the landing page to be at the root.
RELOCATE = {"docs/index.md": "index.md"}


def staged_path(path):
    """Where a repository page lives in the staged tree.

    `README.md` becomes `index.md` because mdBook writes a chapter called README
    out as its directory's `index.html` -- but does *not* rewrite links that point
    at `README.md`, which then resolve to a `README.html` that was never written.
    Renaming at staging time, and rewriting links through this same function,
    keeps the two halves agreeing.
    """
    if path in RELOCATE:
        return RELOCATE[path]
    if path.endswith("README.md"):
        return path[: -len("README.md")] + "index.md"
    return path

# The table of contents. Each part is (title, entries); an entry is a path, a
# (path, children) pair for a nested one, or (path, children, label) when the
# page's own heading is the wrong name for a navigation entry. `None` as the part
# title makes its entries prefix chapters, which mdBook places before the first
# part heading -- the landing page is one.
#
# Chapter titles are otherwise not here on purpose: they are read from each file's
# `# ` heading, so a renamed page renames itself in the navigation.
SECTIONS = [
    (None, ["docs/index.md"]),
    (
        "Playing and designing",
        [
            "docs/GETTING-STARTED.md",
            "docs/WHY.md",
            "docs/COMPOSING.md",
            "docs/TUTORIAL.md",
        ],
    ),
    (
        "Building a module",
        [
            (
                "docs/HARDWARE.md",
                [
                    "docs/hardware/module-button.md",
                    "docs/hardware/module-coin.md",
                    "docs/hardware/README.md",
                ],
            )
        ],
    ),
    (
        "Working on modit",
        [
            "docs/ARCHITECTURE.md",
            # Every plan file is appended here, grouped by folder.
            ("plans/README.md", ["plans/CHECKME.md"]),
        ],
    ),
]

# Where the plan files are attached, and in which order the folders appear.
PLAN_PARENT = "plans/README.md"
PLAN_FOLDERS = ("plans/doing", "plans/todo", "plans/done")


def staged_files():
    """Every repository path the site publishes."""
    found = list(FILES)
    for tree in TREES:
        for root, _, names in os.walk(os.path.join(REPO, tree)):
            rel = os.path.relpath(root, REPO).replace(os.sep, "/")
            if any(rel == s or rel.startswith(s + "/") for s in SKIP):
                continue
            for name in sorted(names):
                path = f"{rel}/{name}"
                if path in SKIP or not name.endswith((".md", ".svg")):
                    continue
                found.append(path)
    return sorted(found)


def title_of(path):
    """A page's own `# ` heading, which is its chapter name."""
    with open(os.path.join(REPO, path), encoding="utf-8") as f:
        for line in f:
            if line.startswith("# "):
                return line[2:].strip()
    sys.exit(f"{path} has no `# ` heading, so it cannot be given a chapter title")


LINK = re.compile(r"\]\(([^)\s]+)\)")


def rewrite_links(path, text, staged):
    """Send links that leave the staged tree to GitHub, and leave the rest alone.

    A link is resolved the way a reader's browser would resolve it, relative to
    the page it is on, so the same target written from two different depths is
    treated identically.

    A link that stays inside the site is re-expressed against the staged layout,
    which for all but the relocated pages is the repository layout and so comes
    back unchanged. A link to a directory becomes a link to that directory's page,
    since that is what mdBook renders.
    """
    here = posixpath.dirname(path)
    from_dir = posixpath.dirname(staged_path(path)) or "."

    def one(match):
        target = match.group(1)
        if "://" in target or target.startswith("#"):
            return match.group(0)
        anchor = ""
        if "#" in target:
            target, anchor = target.split("#", 1)
            anchor = "#" + anchor
        resolved = posixpath.normpath(posixpath.join(here, target)) if target else ""
        if not resolved:
            return match.group(0)
        if resolved not in staged:
            # A directory of pages stands for its own index page.
            if f"{resolved}/README.md" in staged:
                resolved = f"{resolved}/README.md"
            else:
                # Outside the site: source code, a justfile, the repository README.
                return f"]({BLOB}{resolved}{anchor})"
        destination = posixpath.relpath(staged_path(resolved), from_dir)
        return f"]({destination}{anchor})"

    return LINK.sub(one, text)


def summary(staged):
    """mdBook's table of contents, with the plan files folded in."""
    sections = []
    for part, entries in SECTIONS:
        resolved = []
        for entry in entries:
            if not isinstance(entry, tuple):
                entry = (entry, [])
            path, children = entry[0], list(entry[1])
            label = entry[2] if len(entry) > 2 else None
            if path == PLAN_PARENT:
                for folder in PLAN_FOLDERS:
                    children += sorted(
                        p for p in staged if p.startswith(folder + "/") and p.endswith(".md")
                    )
            resolved.append((path, children, label))
        sections.append((part, resolved))

    lines = ["# Summary", ""]
    listed = set()
    for part, entries in sections:
        if part:
            lines += [f"# {part}", ""]
        for path, children, label in entries:
            bullet = "- " if part else ""
            lines.append(f"{bullet}[{label or title_of(path)}]({staged_path(path)})")
            listed.add(path)
            for child in children:
                lines.append(f"  - [{title_of(child)}]({staged_path(child)})")
                listed.add(child)
        lines.append("")

    # A staged page nobody linked from the table of contents is not rendered by
    # mdBook at all, so a link to it would 404 on the site while working fine on
    # GitHub. Refuse rather than publish that.
    missing = [p for p in staged if p.endswith(".md") and p not in listed]
    if missing:
        sys.exit(
            "these pages are staged but absent from the table of contents, so the "
            "site would 404 on links to them:\n  " + "\n  ".join(missing)
        )
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default="target/site/src")
    args = parser.parse_args()

    out = os.path.join(REPO, args.out)
    shutil.rmtree(out, ignore_errors=True)

    staged = staged_files()
    for path in staged:
        destination = os.path.join(out, staged_path(path))
        os.makedirs(os.path.dirname(destination), exist_ok=True)
        if path.endswith(".md"):
            with open(os.path.join(REPO, path), encoding="utf-8") as f:
                text = f.read()
            with open(destination, "w", encoding="utf-8") as f:
                f.write(rewrite_links(path, text, set(staged)))
        else:
            shutil.copyfile(os.path.join(REPO, path), destination)

    with open(os.path.join(out, "SUMMARY.md"), "w", encoding="utf-8") as f:
        f.write(summary(set(staged)) + "\n")

    print(f"staged {len(staged)} page(s) into {args.out}")


if __name__ == "__main__":
    main()
