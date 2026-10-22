#!/usr/bin/env python3
"""Turn a GitHub Actions combined log (`gh run view --log`) into a readable report.

Log lines look like:  <job>\t<step>\t<timestamp> <message>

For every job this extracts: result, duration, the step that broke, the lines
leading up to the failure, and a one-line diagnosis from a small rule table.
Warnings are grouped and counted instead of dumped. Soft failures (a step that
exits 0 but prints that something failed) are listed separately.

Usage:  analyze-ci-log.py LOG [--md OUT.md] [--json OUT.json] [--context N]
Only the standard library is used.
"""
import argparse
import json
import re
import sys
from collections import OrderedDict, defaultdict
from datetime import datetime

ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]|\^\[\[[0-9;]*[A-Za-z]")
TS = re.compile(r"^\ufeff?(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d)(\.\d+)?Z ?(.*)$")

# (regex on the cleaned failure window, category, advice). First match wins.
DIAGNOSES = [
    (r"The operation was canceled|The run was canceled|exceeded the maximum execution time",
     "Cancelled / timeout",
     "Usually `timeout-minutes` was reached or the run was superseded. Check job duration against the limit; a cold cache is the common reason for slow builds."),
    (r"Unable to download artifact.*not found for name: (\S+)",
     "Missing upstream artifact",
     "A previous job never uploaded this artifact. Fix the earlier job first; this failure is a cascade."),
    (r"failed to find tool \"([^\"]+)\"",
     "Tool not found (cc-rs)",
     "A build script needs a C toolchain binary that is not on PATH or not resolved by crorust. Install it on the runner or fix the resolver."),
    (r"Library (\S+) could not be loaded|cannot open shared object file",
     "Missing shared library at run time",
     "Install the runtime package that provides this .so on the runner (not just the -dev package), or bundle it."),
    (r"current package believes it's in a workspace when it's not",
     "Cargo workspace membership",
     "The package is below a directory with a [workspace] manifest. Move it outside the checkout or add it to `workspace.exclude`."),
    (r"error\[E\d+\]",
     "Rust compile error", "Fix the rustc error shown above."),
    (r"test .* \.\.\. FAILED|failures:\n",
     "Test failure", "See the failing test names listed for the job."),
    (r"panicked at",
     "Panic", "A process panicked; the message and location are in the context lines."),
    (r"status code 404|404 Not Found",
     "HTTP 404", "A download URL does not exist (missing release asset or wrong repo/tag)."),
    (r"fatal: |could not resolve host|Connection (reset|timed out)",
     "Network / git", "Transient network or git failure; retry, then check the URL."),
    (r"Process completed with exit code (\d+)",
     "Command failed", "A script step exited non-zero; see the lines before it."),
]

# Messages that look scary but are not problems.
BENIGN_WARNINGS = [
    re.compile(r"hint: ", re.I),
]

SOFT_FAILURE = re.compile(r"(✗ failed|⚠️\s+Failed / deferred)")


def clean(msg):
    return ANSI.sub("", msg).replace("\r", "").rstrip()


def parse_ts(s):
    try:
        return datetime.strptime(s, "%Y-%m-%dT%H:%M:%S")
    except Exception:
        return None


class Job:
    def __init__(self, name):
        self.name = name
        self.lines = []          # (step, ts, msg)
        self.first = None
        self.last = None
        self.group = None        # last ##[group] title, for jobs without step names
        self.err_idx = None      # index of first ##[error]
        self.err_msgs = []
        self.warnings = []
        self.soft = []
        self.group_at = {}       # line index -> group title
        self.rust_warnings = 0
        self.failed_tests = []
        self.test_summary = []   # (passed, failed, ignored)

    @property
    def duration(self):
        if self.first and self.last:
            return int((self.last - self.first).total_seconds())
        return 0

    @property
    def status(self):
        if self.err_idx is None:
            return "success"
        text = " ".join(self.err_msgs)
        if re.search(r"canceled|cancelled", text, re.I):
            return "cancelled"
        return "failure"

    def failing_step(self):
        if self.err_idx is None:
            return None
        step = self.lines[self.err_idx][0]
        if step and step != "UNKNOWN STEP":
            return step
        return self.group_at.get(self.err_idx) or step


def parse(path):
    jobs = OrderedDict()
    with open(path, encoding="utf-8", errors="replace") as fh:
        for raw in fh:
            parts = raw.rstrip("\n").split("\t", 2)
            if len(parts) < 3:
                continue
            jname, step, rest = parts
            m = TS.match(rest)
            ts, msg = (parse_ts(m.group(1)), m.group(3)) if m else (None, rest)
            msg = clean(msg)
            job = jobs.setdefault(jname, Job(jname))
            if ts:
                job.first = job.first or ts
                job.last = ts
            idx = len(job.lines)
            job.lines.append((step, ts, msg))

            g = re.match(r"##\[group\](.*)", msg)
            if g:
                job.group = g.group(1)
            if job.group:
                job.group_at[idx] = job.group

            if msg.startswith("##[error]"):
                if job.err_idx is None:
                    job.err_idx = idx
                job.err_msgs.append(msg[len("##[error]"):])
            elif msg.startswith("##[warning]"):
                text = msg[len("##[warning]"):]
                if not any(b.search(text) for b in BENIGN_WARNINGS):
                    job.warnings.append(text)
            elif re.match(r"warning(\[.*?\])?: ", msg) or "cargo:warning=" in msg:
                job.rust_warnings += 1
                job.warnings.append(msg.strip())
            tm = re.match(r"test (\S+) \.\.\. FAILED", msg)
            if tm:
                job.failed_tests.append(tm.group(1))
            rm = re.match(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", msg)
            if rm:
                job.test_summary.append(tuple(int(x) for x in rm.groups()))
            if SOFT_FAILURE.search(msg):
                job.soft.append(msg.strip())
    return jobs


NOISE = re.compile(r"^(##\[(group|endgroup)\]|\s*$|\s+(at |\d+: )|\s*\d+: )")


def failure_window(job, n):
    """The n meaningful lines before the first error plus the error itself."""
    if job.err_idx is None:
        return []
    out = []
    i = job.err_idx
    while i >= 0 and len(out) < n:
        msg = job.lines[i][2]
        if not NOISE.match(msg) or i == job.err_idx:
            out.append(msg)
        i -= 1
    out.reverse()
    return out


def root_cause_lines(job):
    """Specific lines worth quoting: panics, cc-rs, cargo errors, 404s."""
    pats = re.compile(
        r"(panicked at|error occurred in cc-rs|^error(\[E\d+\])?:|^Error: |could not be loaded|"
        r"Unable to download artifact|status code 404|believes it's in a workspace|exit(ed)? with \d+)")
    seen, picked = set(), []
    for _, _, msg in job.lines:
        if pats.search(msg) and msg not in seen:
            seen.add(msg)
            picked.append(msg[:300])
        if len(picked) >= 6:
            break
    return picked


def diagnose(job, window):
    text = "\n".join(window + root_cause_lines(job) + job.err_msgs)
    for rx, cat, advice in DIAGNOSES:
        m = re.search(rx, text)
        if m:
            detail = m.group(1) if m.groups() and m.group(1) else ""
            return cat, detail, advice
    return "Unclassified", "", "Read the context lines below."


def fmt_dur(s):
    return f"{s // 60}m {s % 60:02d}s"


def tier_of(name):
    m = re.match(r"(Tier \d+)", name)
    return m.group(1) if m else "Other"


NODE20 = re.compile(r"Node\.js 20 is deprecated.*?actions target Node\.js 20 but are being forced to run on Node\.js 24: ([^.]*?)\. For more")


def normalize_warning(w):
    m = NODE20.search(w)
    if m:
        return "Node.js 20 action forced onto Node 24: " + m.group(1)
    w = re.sub(r"\d+\.\d+\.\d+", "X.Y.Z", w)
    w = re.sub(r"\(node:\d+\)", "(node:N)", w)
    return w[:200]


def build(jobs, ctx):
    res = []
    for name, j in jobs.items():
        entry = {
            "job": name, "tier": tier_of(name), "status": j.status,
            "duration_s": j.duration, "failing_step": j.failing_step(),
            "error_messages": j.err_msgs[:5],
            "failed_tests": j.failed_tests,
            "tests": {"passed": sum(t[0] for t in j.test_summary),
                      "failed": sum(t[1] for t in j.test_summary),
                      "ignored": sum(t[2] for t in j.test_summary)},
            "rust_warnings": j.rust_warnings,
            "soft_failures": j.soft[:10],
        }
        if j.err_idx is not None:
            w = failure_window(j, ctx)
            cat, detail, advice = diagnose(j, w)
            entry.update(category=cat, detail=detail, advice=advice,
                         key_lines=root_cause_lines(j), context=w)
        res.append(entry)
    return res


def markdown(run, jobs, ctx):
    L = []
    failed = [e for e in run if e["status"] in ("failure", "cancelled")]
    L.append("# CI Run Diagnostic Summary\n")
    total = len(run)
    L.append(f"**{total - len(failed)} of {total} jobs passed, "
             f"{len(failed)} failed or were cancelled.**\n")

    L.append("## Result by job\n")
    L.append("| Tier | Job | Result | Time | Where it broke |")
    L.append("| :-- | :-- | :-- | --: | :-- |")
    icon = {"success": "✅ pass", "failure": "❌ fail", "cancelled": "⏹️ cancelled"}
    for e in run:
        step = e["failing_step"] or "-"
        step = step.replace("|", "\\|")[:70]
        cat = f" ({e['category']})" if e.get("category") else ""
        L.append(f"| {e['tier']} | {e['job'].split(': ',1)[-1]} | {icon[e['status']]} | "
                 f"{fmt_dur(e['duration_s'])} | {step}{cat} |")
    L.append("")

    if failed:
        L.append("## Failures, grouped by diagnosis\n")
        groups = defaultdict(list)
        for e in failed:
            groups[(e["category"], e["detail"])].append(e)
        for (cat, detail), items in groups.items():
            title = f"{cat}: `{detail}`" if detail else cat
            L.append(f"### {title}  ({len(items)} job{'s' if len(items) != 1 else ''})\n")
            L.append(f"**What it usually means:** {items[0]['advice']}\n")
            L.append("Jobs: " + ", ".join(f"`{i['job']}`" for i in items) + "\n")
            e = items[0]
            if e["key_lines"]:
                L.append("Key lines:\n")
                L.append("```text")
                L.extend(e["key_lines"])
                L.append("```\n")
            L.append("<details><summary>Context before the first error</summary>\n")
            L.append("```text")
            L.extend(x[:240] for x in e["context"])
            L.append("```\n</details>\n")

    soft = [e for e in run if e["soft_failures"]]
    if soft:
        L.append("## Soft failures (job did not fail, but something did)\n")
        L.append("| Job | Message |")
        L.append("| :-- | :-- |")
        for e in soft:
            seen = set()
            for m in e["soft_failures"]:
                m = re.sub(r"\s+", " ", m)[:200].replace("|", "\\|")
                if m not in seen:
                    seen.add(m)
                    L.append(f"| {e['job']} | {m} |")
        L.append("")

    # warnings
    wgroups = defaultdict(lambda: [0, set()])
    for name, j in jobs.items():
        for w in j.warnings:
            k = normalize_warning(w)
            keys = [k]
            if k.startswith("Node.js 20 action forced onto Node 24: "):
                acts = k.split(": ", 1)[1].split(", ")
                keys = ["Node.js 20 action forced onto Node 24: " + a for a in acts]
            for kk in keys:
                wgroups[kk][0] += 1
                wgroups[kk][1].add(name)
    if wgroups:
        L.append("## Warnings\n")
        L.append("| Count | Jobs | Message |")
        L.append("| --: | --: | :-- |")
        for k, (c, js) in sorted(wgroups.items(), key=lambda kv: -kv[1][0])[:25]:
            L.append(f"| {c} | {len(js)} | {k.replace('|', chr(92) + '|')} |")
        L.append("")

    tests = [e for e in run if e["tests"]["passed"] or e["tests"]["failed"]]
    if tests:
        L.append("## Tests\n")
        L.append("| Job | Passed | Failed | Ignored |")
        L.append("| :-- | --: | --: | --: |")
        for e in tests:
            t = e["tests"]
            L.append(f"| {e['job']} | {t['passed']} | {t['failed']} | {t['ignored']} |")
        for e in tests:
            for t in e["failed_tests"]:
                L.append(f"\n- ❌ `{t}` ({e['job']})")
        L.append("")

    slow = sorted(run, key=lambda e: -e["duration_s"])[:5]
    L.append("## Slowest jobs\n")
    for e in slow:
        L.append(f"- {fmt_dur(e['duration_s'])}: {e['job']} ({e['status']})")
    L.append("")
    return "\n".join(L)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("log")
    ap.add_argument("--md")
    ap.add_argument("--json")
    ap.add_argument("--context", type=int, default=25)
    a = ap.parse_args()

    jobs = parse(a.log)
    if not jobs:
        print("no job lines found in log (is it a `gh run view --log` file?)", file=sys.stderr)
        return 2
    run = build(jobs, a.context)
    md = markdown(run, jobs, a.context)
    if a.md:
        with open(a.md, "w", encoding="utf-8") as f:
            f.write(md)
    else:
        print(md)
    if a.json:
        with open(a.json, "w", encoding="utf-8") as f:
            json.dump({"jobs": run,
                       "failed": sum(e["status"] != "success" for e in run)}, f, indent=2)
    return 0


if __name__ == "__main__":
    sys.exit(main())
