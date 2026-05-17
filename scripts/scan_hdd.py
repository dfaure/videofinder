#!/usr/bin/env python3
"""Scan an external HDD and emit a JSONL listing of video files.

The output is consumed by videofinder's merger: one JSON object per line,
each one ready to be inserted as a Tape row (CODE_TAPE is assigned at
merge time). Paths are stored relative to basedir to match the existing
kvideomanager schema.

Replaces helper_import_filenames_into_db.pl. Same filtering and
title-shaping rules; no SQLite writes here.

Usage:
    cd /run/media/dfaure/<HDD-root>
    scan_hdd.py

CWD must contain:
    - an "id" file whose first line is the HDD label (e.g. "ELORA 3");
      the label becomes the LOCATION value (with spaces -> underscores)
      and the output filename (<label>.jsonl).
    - exactly one of: films/, films2/, films3/ (the scan root).
"""

import argparse
import json
import os
import re
import sys

VIDEO_EXTENSIONS = {"mpg", "avi", "ogg", "mp4", "mkv", "m2ts"}

SKIP_PATTERNS = [
    re.compile(r"^Oiseaux/"),
    re.compile(r"(^|/)bin/"),
]
SERIES_PATTERN = re.compile(r"^Series/([^/]+)/")
# Freebox-style trailing block: " - DD-MM-YYYY HHhMM HHhMM (ID)".
# The second HHhMM is the recording duration in hours/minutes; we
# extract it and store it in Tape.DURATION (in minutes). The whole
# block is also stripped from the title for cleaner display.
FREEBOX_TRAILER = re.compile(
    r"\s*-?\s*(\d{2})-(\d{2})-(\d{4})\s+\d{1,2}h\d{2}\s+(\d{1,2})h(\d{2})\s*(?:\(\d+\))?\s*$"
)
# Freebox date alone (fallback if the trailer doesn't match).
DATE_PATTERN_NEW = re.compile(r"(\d{2})-(\d{2})-(\d{4})")
# Legacy manual-recording style "-DDMMYYYY".
DATE_PATTERN_OLD = re.compile(r"-(\d{2})(\d{2})(\d{4})")
UNNAMED_PATTERN = re.compile(r"unn?amed", re.IGNORECASE)

ID_FILE = "id"
CANDIDATE_BASEDIRS = ["films", "films2", "films3"]


def parse_args():
    p = argparse.ArgumentParser(
        description="Scan an HDD for video files and emit a JSONL listing.",
    )
    p.add_argument("--verbose", action="store_true",
                   help="Print each skipped/included file to stderr.")
    return p.parse_args()


def read_location():
    if not os.path.isfile(ID_FILE):
        print(f"error: {ID_FILE!r} not found in CWD ({os.getcwd()!r})",
              file=sys.stderr)
        sys.exit(1)
    with open(ID_FILE, encoding="utf-8") as f:
        first = f.readline().strip()
    if not first:
        print(f"error: {ID_FILE!r} is empty", file=sys.stderr)
        sys.exit(1)
    # Mirror `sed -e 's/ /_/'` from update_filenames_in_kvideomanager.sh:
    # only the first space becomes an underscore. The result is the LOCATION.
    return first.replace(" ", "_", 1)


def find_basedir():
    for candidate in CANDIDATE_BASEDIRS:
        if os.path.isdir(candidate):
            return candidate
    print(f"error: none of {CANDIDATE_BASEDIRS} found in CWD ({os.getcwd()!r})",
          file=sys.stderr)
    sys.exit(1)


def collect_paths(basedir):
    """Walk basedir and return the set of paths relative to basedir.
    The films/films2/films3 prefix is dropped: LOCATION already says which
    disk a file is on, so the prefix carried no extra information."""
    entries = set()
    for dirpath, _dirnames, filenames in os.walk(basedir):
        for name in filenames:
            rel = os.path.relpath(os.path.join(dirpath, name), basedir)
            entries.add(rel)
    return entries


def derive_record(path, all_paths, location):
    """Return a Tape-row dict, or None to skip this file."""
    for pat in SKIP_PATTERNS:
        if pat.search(path):
            return None

    dot = path.rfind(".")
    if dot < 0:
        return None
    ext = path[dot + 1:]
    if not re.fullmatch(r"[a-zA-Z0-9]+", ext):
        return None
    if ext not in VIDEO_EXTENSIONS:
        return None

    stem = path[:dot]

    # Prefer an .mp4 over an .mpg with the same stem.
    if ext == "mpg" and f"{stem}.mp4" in all_paths:
        return None

    title = stem
    date_purchase = ""
    duration = 0

    m = FREEBOX_TRAILER.search(title)
    if m:
        day, month, year = m.group(1), m.group(2), m.group(3)
        duration = int(m.group(4)) * 60 + int(m.group(5))
        if not UNNAMED_PATTERN.search(title):
            title = title[:m.start()] + title[m.end():]
        date_purchase = f"{year}-{month}-{day}"
    else:
        md = DATE_PATTERN_NEW.search(title)
        if md:
            day, month, year = md.group(1), md.group(2), md.group(3)
            if not UNNAMED_PATTERN.search(title):
                title = title.replace(f"{day}-{month}-{year}", "")
            date_purchase = f"{year}-{month}-{day}"
        else:
            mo = DATE_PATTERN_OLD.search(title)
            if mo:
                day, month, year = mo.group(1), mo.group(2), mo.group(3)
                if not UNNAMED_PATTERN.search(title):
                    title = title.replace(f"-{day}{month}{year}", "")
                date_purchase = f"{year}-{month}-{day}"

    series_match = SERIES_PATTERN.match(path)
    title = title.rsplit("/", 1)[-1]
    if series_match:
        title = f"{series_match.group(1)}: {title}"

    return {
        "path": path,
        "title": title,
        "location": location,
        "shelf": 1,
        "row": 1,
        "position": 1,
        "type": 4,
        "date_purchase": date_purchase,
        "duration": duration,
    }


def main():
    args = parse_args()

    location = read_location()
    basedir = find_basedir()
    output = f"{location}.jsonl"

    print(f"Scanning {basedir}/ as location={location} -> {output}",
          file=sys.stderr)

    all_paths = collect_paths(basedir)

    written = 0
    skipped = 0
    with open(output, "w", encoding="utf-8") as out:
        for path in sorted(all_paths):
            rec = derive_record(path, all_paths, location)
            if rec is None:
                skipped += 1
                if args.verbose:
                    print(f"skip: {path}", file=sys.stderr)
                continue
            out.write(json.dumps(rec, ensure_ascii=False) + "\n")
            written += 1

    print(f"Wrote {written} entries, skipped {skipped} files -> {output}",
          file=sys.stderr)


if __name__ == "__main__":
    main()
