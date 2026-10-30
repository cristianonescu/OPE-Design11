#!/usr/bin/env python3
"""Regenerates the status blocks of README.md from the project's own records.

    python -I tools/readme_status.py            # show what would change, write nothing
    python -I tools/readme_status.py --write    # update README.md
    python -I tools/readme_status.py --check    # exit 1 if README.md is out of date
    python -I tools/readme_status.py --offline  # skip the GitHub issue list (keeps it as is)

Sources:
- docs/FEATURE_MATRIX.md: the row counts per status, the Blocked and Documented rows (not yet
  implemented) and the "Planned (beyond PE-DESIGN)" rows.
- docs/COMPATIBILITY_STATUS.md: the spec totals and how many matrix rows reach each tier.
- docs/COMPATIBILITY_AUDIT.md: the summary items not marked **fixed** (known differences).
- `gh issue list` on cristianonescu/OpenStitch: open issues (bugs under known issues, the rest
  under planned).

Each block sits between `<!-- status:NAME -->` and `<!-- /status:NAME -->` in README.md; the
text around the blocks is written by hand and is never touched. A Documented row whose feature
also has an Implemented row (the matrix keeps the guide's row next to the one that implemented
it) is left out of the not-yet-implemented list.
"""

import argparse
import difflib
import json
import re
import shutil
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
README = ROOT / "README.md"
MATRIX = ROOT / "docs" / "FEATURE_MATRIX.md"
STATUS = ROOT / "docs" / "COMPATIBILITY_STATUS.md"
AUDIT = ROOT / "docs" / "COMPATIBILITY_AUDIT.md"
REPO = "cristianonescu/OpenStitch"
ISSUES = f"https://github.com/{REPO}/issues"

def table_rows(text, header):
    """Rows of the first Markdown table whose header line starts with `header`."""
    rows, inside = [], False
    for line in text.splitlines():
        if line.startswith(header):
            inside = True
            continue
        if inside:
            if not line.startswith("|"):
                break
            if line.startswith("|---"):
                continue
            rows.append([c.strip() for c in line.strip().strip("|").split(" | ")])
    return rows


def kind(status):
    s = status.lower()
    if s.startswith("planned"):
        return "Planned (beyond PE-DESIGN)"
    for k in ("Verified Compatible", "Partially Compatible", "Implemented", "Blocked",
              "Documented", "Not Investigated"):
        if s.startswith(k.lower()):
            return k
    return status


def short(feature, limit=110):
    """The feature's name: the text before its first colon or semicolon, without short
    parenthetical notes."""
    name = re.split(r": |; ", feature, maxsplit=1)[0]
    name = re.sub(r" \([^():]{1,40}\)", "", name)
    name = re.split(r" \(", name, maxsplit=1)[0].strip().rstrip(".").replace("`", "")
    return name if len(name) <= limit else name[: limit - 1].rstrip() + "…"


def matrix():
    rows = table_rows(MATRIX.read_text(encoding="utf-8"), "| Area | Feature | Status")
    rows = [r for r in rows if len(r) >= 3]
    documented = [r for r in rows if kind(r[2]) == "Documented"]
    blocked = [r for r in rows if kind(r[2]) == "Blocked"]
    planned = [r for r in rows if kind(r[2]).startswith("Planned")]
    counts = Counter(kind(r[2]) for r in rows)
    return rows, counts, documented, blocked, planned


def compat():
    text = STATUS.read_text(encoding="utf-8")
    m = re.search(r"Specs: (\d+) · pass (\d+) · fail (\d+)", text)
    specs = tuple(int(x) for x in m.groups()) if m else (0, 0, 0)
    rows = table_rows(text, "| Area | Feature | Tier")
    tiers = Counter()
    for r in rows:
        # A row is only as far as its weakest spec: the first tier listed.
        tiers[r[2].split(";")[0].strip()] += 1
    return specs, tiers


def anchor(heading):
    a = re.sub(r"[^\w\- ]", "", heading.strip().lower())
    return a.replace(" ", "-")


def audit():
    text = AUDIT.read_text(encoding="utf-8")
    heads = {}
    for line in text.splitlines():
        m = re.match(r"## (\d+)\. (.*)", line)
        if m:
            heads[m.group(1)] = anchor(f"{m.group(1)}. {m.group(2)}")
    items = []
    for r in table_rows(text, "| # | Difference"):
        num, diff = r[0], r[1]
        if "**fixed**" in diff:
            continue
        partly = "**partly fixed**" in diff
        title = re.split(r" \(\*\*", diff, maxsplit=1)[0].strip()
        items.append((num, title, partly, heads.get(num, "")))
    return items


def issues(offline):
    if offline:
        return None
    gh = shutil.which("gh") or r"C:\Program Files\GitHub CLI\gh.exe"
    try:
        out = subprocess.run(
            [gh, "issue", "list", "--repo", REPO, "--state", "open", "--limit", "200",
             "--json", "number,title,labels"],
            capture_output=True, text=True, encoding="utf-8", check=True, timeout=60).stdout
    except (OSError, subprocess.SubprocessError) as e:
        print(f"gh issue list failed ({e}); issue blocks left as they are", file=sys.stderr)
        return None
    found = json.loads(out)
    found.sort(key=lambda i: i["number"])
    return found


def link_list(entries):
    return "\n".join(entries) if entries else "_None._"


def blocks(offline):
    rows, counts, documented, blocked, planned = matrix()
    (total, passed, failed), tiers = compat()
    open_audit = audit()
    found = issues(offline)

    out = {}
    done = counts["Implemented"] + counts["Partially Compatible"] + counts["Verified Compatible"]
    tier_text = ", ".join(f"{n} at the {t} tier" for t, n in
                          sorted(tiers.items(), key=lambda x: -x[1]))
    out["glance"] = "\n".join([
        "| | |",
        "|---|---|",
        f"| Feature matrix rows | {len(rows)}: {done} implemented"
        f" ({counts['Partially Compatible']} of them partially compatible),"
        f" {counts['Documented']} documented only, {counts['Blocked']} blocked,"
        f" {counts['Planned (beyond PE-DESIGN)']} planned beyond PE-DESIGN |",
        f"| Compatibility specs | {total}: {passed} pass, {failed} fail"
        f" ([status](docs/COMPATIBILITY_STATUS.md)) |",
        f"| Matrix rows covered by specs | {sum(tiers.values())}: {tier_text} |",
        f"| Open audit differences | {len(open_audit)}"
        f" ([audit](docs/COMPATIBILITY_AUDIT.md#summary)) |",
    ] + ([f"| Open GitHub issues | {len(found)} ([list]({ISSUES})) |"] if found is not None
         else []))

    out["not-implemented"] = link_list(
        [f"- **{short(r[1])}** ({r[0]})" for r in documented])
    out["blocked"] = link_list(
        [f"- **{short(r[1])}** ({r[0]})" for r in blocked])
    out["known-issues"] = link_list(
        [f"- [{title}](docs/COMPATIBILITY_AUDIT.md#{a})" + (" (partly fixed)" if partly else "")
         for num, title, partly, a in open_audit])
    out["beyond-planned"] = link_list([f"- {r[1].replace('`', '')}" for r in planned])
    if found is not None:
        bugs = [i for i in found if any(lb["name"] == "bug" for lb in i["labels"])]
        rest = [i for i in found if i not in bugs]
        out["issues-bugs"] = link_list(
            [f"- [#{i['number']}]({ISSUES}/{i['number']}) {i['title']}" for i in bugs])
        out["issues-planned"] = link_list(
            [f"- [#{i['number']}]({ISSUES}/{i['number']}) {i['title']}" for i in rest])
    return out


def apply(text, generated):
    missing = []
    for name, body in generated.items():
        tag = re.escape(name)
        pattern = re.compile(rf"(<!-- status:{tag} -->\n)(?:.*?\n)?(<!-- /status:{tag} -->)", re.S)
        if not pattern.search(text):
            missing.append(name)
            continue
        text = pattern.sub(lambda m: m.group(1) + body + "\n" + m.group(2), text)
    return text, missing


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    p = argparse.ArgumentParser()
    p.add_argument("--write", action="store_true", help="update README.md")
    p.add_argument("--check", action="store_true", help="exit 1 if README.md is out of date")
    p.add_argument("--offline", action="store_true", help="skip the GitHub issue list")
    a = p.parse_args()

    old = README.read_text(encoding="utf-8")
    new, missing = apply(old, blocks(a.offline))
    for name in missing:
        print(f"README.md has no <!-- status:{name} --> block", file=sys.stderr)
    diff = list(difflib.unified_diff(old.splitlines(), new.splitlines(), "README.md (now)",
                                     "README.md (generated)", lineterm="", n=0))
    if not diff:
        print("README.md status blocks are current.")
        return 1 if a.check and missing else 0
    print("\n".join(diff))
    if a.write:
        README.write_text(new, encoding="utf-8", newline="\n")
        print("README.md updated.")
        return 0
    return 1 if a.check else 0


if __name__ == "__main__":
    sys.exit(main())
