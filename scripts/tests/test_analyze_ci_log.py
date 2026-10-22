"""Run with: python3 -m unittest discover -s scripts/tests"""
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import importlib.util

spec = importlib.util.spec_from_file_location(
    "analyze", os.path.join(os.path.dirname(__file__), "..", "analyze-ci-log.py"))
analyze = importlib.util.module_from_spec(spec)
spec.loader.exec_module(analyze)


def line(job, step, sec, msg):
    return f"{job}\t{step}\t2026-10-20T08:00:{sec:02d}.0000000Z {msg}\r\n"


def parse(lines):
    with tempfile.NamedTemporaryFile("w", suffix=".log", delete=False, newline="") as f:
        f.writelines(lines)
    try:
        return analyze.parse(f.name)
    finally:
        os.unlink(f.name)


class AnalyzerTests(unittest.TestCase):
    def test_passing_job_is_success(self):
        jobs = parse([line("Tier 1: A", "Test", 1, "test result: ok. 5 passed; 0 failed; 1 ignored; 0 measured")])
        run = analyze.build(jobs, 10)
        self.assertEqual(run[0]["status"], "success")
        self.assertEqual(run[0]["tests"]["passed"], 5)
        self.assertEqual(run[0]["tests"]["ignored"], 1)

    def test_timeout_is_cancelled_not_failure(self):
        jobs = parse([
            line("Tier 2: W", "Build", 1, "   Compiling foo v1.0.0"),
            line("Tier 2: W", "Build", 59, "##[error]The operation was canceled."),
        ])
        e = analyze.build(jobs, 10)[0]
        self.assertEqual(e["status"], "cancelled")
        self.assertEqual(e["category"], "Cancelled / timeout")
        self.assertEqual(e["failing_step"], "Build")

    def test_cc_rs_missing_tool_names_the_tool(self):
        jobs = parse([
            line("Tier 4: X", "UNKNOWN STEP", 1, "##[group]Run ./bin/crorust build"),
            line("Tier 4: X", "UNKNOWN STEP", 2, '  error occurred in cc-rs: failed to find tool "llvm-ar": No such file'),
            line("Tier 4: X", "UNKNOWN STEP", 3, "##[error]Process completed with exit code 1."),
        ])
        e = analyze.build(jobs, 10)[0]
        self.assertEqual(e["category"], "Tool not found (cc-rs)")
        self.assertEqual(e["detail"], "llvm-ar")
        # step name is unknown in the log, so we fall back to the last ##[group]
        self.assertEqual(e["failing_step"], "Run ./bin/crorust build")

    def test_ansi_codes_and_bom_are_stripped(self):
        jobs = parse(["J\tS\t\ufeff2026-10-20T08:00:01.0Z \x1b[31m##[error]\x1b[0mboom\r\n"])
        # ANSI before the marker must not hide the error
        self.assertEqual(analyze.build(jobs, 5)[0]["status"], "failure")

    def test_soft_failure_in_green_job_is_reported(self):
        jobs = parse([line("Tier 2: L", "Provision", 1,
                           "  [i --fix] Provisioning NDK... \x1b[31m✗ failed (404)\x1b[0m")])
        e = analyze.build(jobs, 5)[0]
        self.assertEqual(e["status"], "success")
        self.assertEqual(len(e["soft_failures"]), 1)

    def test_node20_warning_is_split_per_action(self):
        w = ("Node.js 20 is deprecated. The following actions target Node.js 20 but are being "
             "forced to run on Node.js 24: actions/checkout@v4, actions/cache@v4. For more information see: x")
        self.assertTrue(analyze.normalize_warning(w).endswith("actions/checkout@v4, actions/cache@v4"))

    def test_failed_test_names_are_collected(self):
        jobs = parse([line("Tier 1: A", "Test", 1, "test foo::bar ... FAILED")])
        self.assertEqual(analyze.build(jobs, 5)[0]["failed_tests"], ["foo::bar"])


if __name__ == "__main__":
    unittest.main()
