#!/usr/bin/env python3
"""Fail if the built site links to a page it does not contain.

Staging rewrites links, mdBook rewrites them again (`.md` to `.html`, a chapter
called README to its directory's `index.html`), and both rewrites have already
produced links that worked on GitHub and 404'd on the site. That is the failure
this catches: it reads the *rendered* HTML, so it checks the result rather than
either party's intention.

Only links within the site are checked. An `https://` link is somebody else's
problem, and this deliberately makes no network requests.

Usage:  python3 docs/site/check_links.py [--book target/site/book]
"""

import argparse
import os
import posixpath
import re
import sys

LINK = re.compile(r'(?:href|src)="([^"]+)"')

# mdBook's own 404 page links to the site root, which exists once the site is
# served but is not a file in the build directory.
SITE_ROOT_ONLY = ("404.html",)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--book", default="target/site/book")
    args = parser.parse_args()

    if not os.path.isdir(args.book):
        sys.exit(f"{args.book} does not exist -- run `just site` first")

    broken = set()
    pages = 0
    for directory, _, names in os.walk(args.book):
        for name in names:
            if not name.endswith(".html"):
                continue
            pages += 1
            path = os.path.join(directory, name)
            page = os.path.relpath(path, args.book).replace(os.sep, "/")
            with open(path, encoding="utf-8") as f:
                html = f.read()
            for target in LINK.findall(html):
                if "://" in target or target.startswith(("#", "mailto:", "data:")):
                    continue
                target = target.split("#")[0].split("?")[0]
                if not target:
                    continue
                if target.startswith("/"):
                    # Absolute within the served site; only mdBook's own pages
                    # produce these, and they cannot be resolved against a
                    # directory of files.
                    if page in SITE_ROOT_ONLY:
                        continue
                    broken.add((page, target))
                    continue
                resolved = posixpath.normpath(posixpath.join(posixpath.dirname(page), target))
                full = os.path.join(args.book, resolved)
                if os.path.isdir(full):
                    full = os.path.join(full, "index.html")
                if not os.path.exists(full):
                    broken.add((page, target))

    if broken:
        print(f"{len(broken)} broken link(s) in {pages} page(s):", file=sys.stderr)
        for page, target in sorted(broken):
            print(f"  {page} -> {target}", file=sys.stderr)
        sys.exit(
            "\nA link that works on GitHub can still 404 on the site. Check that the "
            "target page is in the table of contents (docs/site/stage.py), and that "
            "a link to a source file points into crates/ so it is sent to GitHub."
        )
    print(f"{pages} page(s), no broken links")


if __name__ == "__main__":
    main()
