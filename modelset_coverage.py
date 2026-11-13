#!/usr/bin/env python3

from __future__ import annotations

import argparse
import csv
from collections import Counter
import hashlib
import json
import os
import platform
import re
import signal
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Sequence, TextIO

DEFAULT_MODELSET = Path("./modelset").expanduser()
DEFAULT_MAX_ERROR_CHARS = 4_000
CARGO_CLEAN_TIMEOUT_S = 60
ANSI_ESCAPE_RE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")


@dataclass
class OutputCapture:
    """Bounded head and tail: startup noise cannot displace the terminal error."""

    head: str = ""
    tail: str = ""
    total_chars: int = 0
    limit: int = DEFAULT_MAX_ERROR_CHARS

    def add(self, chunk: str, max_chars: int) -> None:
        self.limit = max_chars
        self.total_chars += len(chunk)
        head_limit = (max_chars + 1) // 2
        remaining = head_limit - len(self.head)
        if remaining > 0:
            self.head += chunk[:remaining]
            chunk = chunk[remaining:]
        tail_limit = max_chars - head_limit
        if tail_limit:
            self.tail = (self.tail + chunk)[-tail_limit:]

    def render(self) -> str:
        if self.total_chars <= self.limit:
            return (self.head + self.tail).strip()
        return f"{self.head}\n[output truncated; {self.total_chars - self.limit} chars omitted]\n{self.tail}".strip()


@dataclass
class DiagnosticCapture:
    """Extract structured events before truncating their human-readable output."""

    samples: list[dict] = field(default_factory=list)
    counts: Counter = field(default_factory=Counter)
    warning_counts: Counter = field(default_factory=Counter)
    result: dict | None = None
    sample_chars: int = 0
    dropped: int = 0
    errors: int = 0
    warnings: int = 0
    last_phase: str = ""
    pending: str = ""
    oversized: bool = False
    executable: str = ""
    expected_manifest: str = ""
    dependency_error: bool = False
    error_names: list[str] = field(default_factory=list)

    def add(self, chunk: str, budget: int) -> None:
        for piece in chunk.splitlines(keepends=True):
            if not self.oversized:
                self.pending += piece
                if len(self.pending) > 1_048_576:
                    self.pending = ""
                    self.oversized = True
                    self.dropped += 1
            if piece.endswith("\n"):
                if not self.oversized:
                    self.line(self.pending, budget)
                self.pending = ""
                self.oversized = False

    def finish(self, budget: int) -> None:
        if self.pending:
            self.line(self.pending, budget)
            self.pending = ""

    def line(self, line: str, budget: int) -> None:
        try:
            event = json.loads(line)
        except (ValueError, TypeError):
            return
        if not isinstance(event, dict):
            return
        reason = event.get("reason")
        if reason == "arachne-result":
            self.result = event
            if isinstance(event.get("error"), dict):
                self.record({"level": "error", **event["error"]}, budget)
        elif reason == "arachne-diagnostic":
            if event.get("level") == "info":
                self.last_phase = str(event.get("message", ""))[:200]
            else:
                self.record(event, budget)
        elif reason == "compiler-message":
            message = event.get("message", {})
            code = message.get("code") or {}
            spans = message.get("spans", [])
            primary = next((span for span in spans if span.get("is_primary")), {})
            if message.get("level") in ("error", "ice"):
                manifest = event.get("manifest_path")
                if (
                    self.expected_manifest
                    and manifest
                    and str(Path(manifest).resolve()) != self.expected_manifest
                ):
                    self.dependency_error = True
            self.record(
                {
                    "level": message.get("level"),
                    "code": code.get("code") or "rustc",
                    "message": message.get("message", ""),
                    "package_id": event.get("package_id"),
                    "location": {
                        k: primary[k]
                        for k in ("file_name", "line_start", "column_start")
                        if k in primary
                    },
                },
                budget,
            )
        elif (
            reason == "compiler-artifact"
            and event.get("target", {}).get("name") == "arachne"
        ):
            self.executable = str(event.get("executable") or "")

    def record(self, diagnostic: dict, budget: int) -> None:
        level = diagnostic.get("level")
        if level not in ("warning", "warn", "error", "failure-note", "ice"):
            return
        warning = level in ("warning", "warn")
        self.warnings += int(warning)
        self.errors += int(not warning)
        code = str(diagnostic.get("code") or "unknown")[:120]
        if not warning and len(self.error_names) < 3:
            name = f"{code}: {str(diagnostic.get('message', ''))[:300]}"
            if name not in self.error_names:
                self.error_names.append(name)
        counter = self.warning_counts if warning else self.counts
        key = code if code in counter or len(counter) < 64 else "other"
        counter[key] += 1
        sample = {
            k: v
            for k, v in diagnostic.items()
            if k not in ("reason", "rendered", "spans")
        }
        sample["message"] = str(sample.get("message", ""))[:1000]
        if sample in self.samples:
            return
        size = len(json.dumps(sample, ensure_ascii=False))
        # Error locations deserve space even when earlier warnings used the budget.
        if not warning:
            while self.samples and self.sample_chars + size > budget:
                index = next(
                    (
                        i
                        for i, item in enumerate(self.samples)
                        if item.get("level") in ("warning", "warn")
                    ),
                    None,
                )
                if index is None:
                    break
                removed = self.samples.pop(index)
                self.sample_chars -= len(json.dumps(removed, ensure_ascii=False))
                self.dropped += 1
        if self.sample_chars + size <= budget and len(self.samples) < 64:
            self.samples.append(sample)
            self.sample_chars += size
        else:
            self.dropped += 1


@dataclass
class StepOutcome:
    ok: bool
    duration_s: float
    error: str | None = None
    status: str = ""
    failure_kind: str = ""
    capability: str = ""
    returncode: int | None = None
    diagnostics: list[dict] = field(default_factory=list)
    diagnostic_counts: dict[str, int] = field(default_factory=dict)
    warning_counts: dict[str, int] = field(default_factory=dict)
    warning_count: int = 0
    diagnostics_dropped: int = 0
    last_phase: str = ""
    artifact_path: str = ""
    classification_source: str = ""

    def __post_init__(self) -> None:
        if not self.status:
            self.status = "passed" if self.ok else "failed"


@dataclass
class ModelOutcome:
    path: Path
    input_hash: str = ""
    dependency_hash: str = ""
    artifact_dir: str = ""
    parse: StepOutcome | None = None
    generate: StepOutcome | None = None
    compile: StepOutcome | None = None


@dataclass
class Summary:
    failure_sample_limit: int = 10
    total: int = 0
    parsed: int = 0
    generated: int = 0
    compiled: int = 0
    compiled_with_warnings: int = 0
    failure_groups: Counter = field(default_factory=Counter)
    parse_failure_count: int = 0
    generate_failure_count: int = 0
    compile_failure_count: int = 0
    parse_failures: list[tuple[Path, str]] = field(default_factory=list)
    generate_failures: list[tuple[Path, str]] = field(default_factory=list)
    compile_failures: list[tuple[Path, str]] = field(default_factory=list)
    parse_time_s: float = 0.0
    generate_time_s: float = 0.0
    compile_time_s: float = 0.0

    def add(self, outcome: ModelOutcome) -> None:
        self.total += 1
        if (
            outcome.compile
            and outcome.compile.ok
            and any(
                step and step.warning_count
                for step in (outcome.parse, outcome.generate, outcome.compile)
            )
        ):
            self.compiled_with_warnings += 1
        for stage, step in (
            ("parse", outcome.parse),
            ("generate", outcome.generate),
            ("compile", outcome.compile),
        ):
            if step and not step.ok:
                self.failure_groups[
                    (
                        stage,
                        step.failure_kind or "unknown",
                        step.capability or "unknown",
                    )
                ] += 1
        if outcome.parse is not None:
            self.parse_time_s += outcome.parse.duration_s
            if outcome.parse.ok:
                self.parsed += 1
            else:
                self.parse_failure_count += 1
                if len(self.parse_failures) < self.failure_sample_limit:
                    self.parse_failures.append(
                        (outcome.path, outcome.parse.error or "parse failed")
                    )
        if outcome.generate is not None:
            self.generate_time_s += outcome.generate.duration_s
            if outcome.generate.ok:
                self.generated += 1
            else:
                self.generate_failure_count += 1
                if len(self.generate_failures) < self.failure_sample_limit:
                    self.generate_failures.append(
                        (outcome.path, outcome.generate.error or "generation failed")
                    )
        if outcome.compile is not None:
            self.compile_time_s += outcome.compile.duration_s
            if outcome.compile.ok:
                self.compiled += 1
            else:
                self.compile_failure_count += 1
                if len(self.compile_failures) < self.failure_sample_limit:
                    self.compile_failures.append(
                        (outcome.path, outcome.compile.error or "compile failed")
                    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Walk a directory tree of .ecore files and measure Arachne coverage "
            "(parse, code generation, generated-code compilation)."
        )
    )
    parser.add_argument(
        "root",
        nargs="?",
        default=str(DEFAULT_MODELSET),
        help=f"Directory to scan recursively (default: {DEFAULT_MODELSET})",
    )
    parser.add_argument(
        "--parse-timeout",
        type=int,
        default=30,
        help="Timeout in seconds for the parse step per metamodel",
    )
    parser.add_argument(
        "--generate-timeout",
        type=int,
        default=120,
        help="Timeout in seconds for the generation step per metamodel",
    )
    parser.add_argument(
        "--compile-timeout",
        type=int,
        default=180,
        help="Timeout in seconds for the compile step per metamodel",
    )
    parser.add_argument(
        "--keep-failures",
        action="store_true",
        help="Persist failing inputs, diagnostics, and generated sources beside the CSV",
    )
    parser.add_argument(
        "--failures-dir",
        type=Path,
        help="Persistent reproduction directory (default: CSV stem + '-failures')",
    )
    parser.add_argument(
        "--show-failures",
        type=int,
        default=10,
        help="How many failing models to print per stage in the final report",
    )
    parser.add_argument(
        "--max-error-chars",
        type=int,
        default=DEFAULT_MAX_ERROR_CHARS,
        help=(
            "Maximum subprocess output characters to keep per failed step "
            f"(default: {DEFAULT_MAX_ERROR_CHARS})"
        ),
    )
    parser.add_argument(
        "--skip-cargo-clean",
        action="store_true",
        help=(
            "Do not run 'cargo clean -p' after each generated-project compile. "
            "This is faster but lets the shared Cargo target grow for the whole run."
        ),
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="Maximum number of .ecore files to process; default is all discovered files",
    )
    parser.add_argument(
        "--offset",
        type=int,
        default=0,
        help="Number of discovered .ecore files to skip before processing",
    )
    parser.add_argument(
        "--csv",
        type=Path,
        default=Path("modelset_coverage.csv"),
        help=(
            "Stream per-model status, diagnostics, warnings and timings to this CSV "
            "(default: modelset_coverage.csv)"
        ),
    )
    return parser.parse_args()


def find_ecore_files(root: Path, exclude: Sequence[Path] = ()) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*")
        if path.is_file()
        and path.suffix.lower() == ".ecore"
        and not any(path.is_relative_to(directory) for directory in exclude)
    )


def drain_limited(
    stream: TextIO,
    capture: OutputCapture,
    diagnostics: DiagnosticCapture,
    max_chars: int,
) -> None:
    while True:
        chunk = stream.read(4096)
        if not chunk:
            break
        capture.add(chunk, max_chars)
        diagnostics.add(chunk, max_chars)
    diagnostics.finish(max_chars)


def kill_process_tree(process: subprocess.Popen[str]) -> None:
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if process.poll() is None:
            process.kill()
        return
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def classify_failure(
    error: str, stage: str, structured: dict | None = None
) -> tuple[str, str]:
    """Unknown stays unknown; never turn an infrastructure error into model rejection."""
    text = ANSI_ESCAPE_RE.sub("", error).lower()
    if any(
        token in text
        for token in ("panicked at", "internal compiler error", "stack overflow")
    ):
        return "implementation_bug", "panic"
    if any(
        token in text
        for token in (
            "no space left",
            "failed to download",
            "failed to get",
            "failed to fetch",
            "could not resolve",
            "failed to load source",
            "permission denied",
            "could not execute process",
        )
    ):
        return "infrastructure", "environment"
    if structured and structured.get("kind") not in (None, "unknown"):
        return str(structured["kind"]), str(structured.get("code", "unknown"))
    if stage == "compile":
        return "unknown", "cargo_failure"
    if (
        "failed to parse xml" in text
        or "expected boolean" in text
        or "unexpected upper bound" in text
    ):
        return "invalid_input", "xml_or_attribute_value"
    rules = (
        (("builtin", "builtit", "failed to parse type"), "datatype_mapping"),
        (("etype", "type path", "forward-referenced"), "type_resolution"),
        (("generic", "typeparameter"), "generics"),
        (("annotation",), "annotations"),
        (("root class",), "root_selection"),
        (("root element", "package element"), "document_structure"),
        (
            ("unsupported attribute", "unsupported namespaced attribute"),
            "metadata_attribute",
        ),
        (("unsupported",), "unsupported_construct"),
    )
    for tokens, capability in rules:
        if any(token in text for token in tokens):
            return "unsupported_feature", capability
    return "unknown", str((structured or {}).get("code", "unknown"))


def run_command(
    cmd: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    timeout_s: int,
    max_output_chars: int = DEFAULT_MAX_ERROR_CHARS,
    stage: str = "",
) -> StepOutcome:
    start = time.perf_counter()
    popen_kwargs: dict[str, object] = {}
    if os.name == "nt":
        popen_kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
    else:
        popen_kwargs["start_new_session"] = True
    try:
        process = subprocess.Popen(
            cmd,
            cwd=str(cwd),
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            encoding="utf-8",
            errors="replace",
            **popen_kwargs,
        )
    except OSError as exc:
        return StepOutcome(
            False,
            time.perf_counter() - start,
            str(exc),
            status="failed",
            failure_kind="infrastructure",
            capability="process_start",
            classification_source="process",
        )

    capture = OutputCapture()
    diagnostics = DiagnosticCapture()
    if "--manifest-path" in cmd:
        diagnostics.expected_manifest = str(
            Path(cmd[cmd.index("--manifest-path") + 1]).resolve()
        )
    assert process.stdout is not None
    reader = threading.Thread(
        target=drain_limited,
        args=(process.stdout, capture, diagnostics, max_output_chars),
        daemon=True,
    )
    reader.start()
    timed_out = False
    try:
        try:
            returncode = process.wait(timeout=timeout_s)
        except subprocess.TimeoutExpired:
            timed_out = True
            kill_process_tree(process)
            returncode = process.wait()
        # A descendant may still own stdout after its parent exits.
        reader.join(timeout=2)
        if reader.is_alive():
            kill_process_tree(process)
            reader.join(timeout=2)
        if reader.is_alive():
            raise RuntimeError(
                "subprocess output reader did not stop after process-tree cleanup"
            )
    except BaseException:
        kill_process_tree(process)
        process.wait()
        reader.join(timeout=2)
        raise
    finally:
        if not reader.is_alive():
            process.stdout.close()

    output = capture.render()
    result = diagnostics.result or {}
    error_diagnostic = result.get("error") or {}
    error = str(
        error_diagnostic.get("message")
        or output
        or f"command exited with code {returncode}"
    )
    ok = returncode == 0 and result.get("ok", True) is not False
    status = "passed" if ok else "failed"
    kind, capability = (
        ("", "") if ok else classify_failure(error, stage, error_diagnostic)
    )
    classification_source = (
        ""
        if ok
        else (
            "structured"
            if error_diagnostic.get("kind") not in (None, "unknown")
            else "inferred"
        )
    )
    if timed_out:
        classification_source = "process"
        status, kind, capability = "timed_out", "resource_limit", "timeout"
        error = f"timeout after {timeout_s}s: {' '.join(cmd)}\n{output}"
    elif (
        returncode < 0
        or (returncode == 101 and stage in ("parse", "generate"))
        or "panicked at" in output
        or "stack overflow" in output
    ):
        classification_source = "process"
        status = "crashed"
        kind, capability = (
            ("implementation_bug", "panic")
            if returncode == 101
            or "panicked at" in output
            or "stack overflow" in output
            else ("unknown", "signal")
        )
    if (
        ok
        and stage in ("parse", "generate")
        and (result.get("stage") != stage or not isinstance(result.get("ok"), bool))
    ):
        ok, status, kind, capability = (
            False,
            "failed",
            "infrastructure",
            "diagnostic_protocol",
        )
        error = "CLI did not emit a valid structured result"
        classification_source = "process"
    if not ok and stage == "compile" and diagnostics.errors and status == "failed":
        kind = (
            "infrastructure" if diagnostics.dependency_error else "implementation_bug"
        )
        capability = (
            "dependency_build" if diagnostics.dependency_error else "generated_code"
        )
        classification_source = "structured"
    if not ok and diagnostics.error_names and not error_diagnostic:
        error = f"{'; '.join(diagnostics.error_names)}\n{error}"
    bounded_error = OutputCapture()
    bounded_error.add(error, max_output_chars)
    return StepOutcome(
        ok,
        time.perf_counter() - start,
        None if ok else bounded_error.render(),
        status=status,
        failure_kind=kind,
        capability=capability,
        returncode=returncode,
        diagnostics=diagnostics.samples,
        diagnostic_counts=dict(diagnostics.counts),
        warning_counts=dict(diagnostics.warning_counts),
        warning_count=diagnostics.warnings,
        diagnostics_dropped=diagnostics.dropped,
        last_phase=diagnostics.last_phase,
        artifact_path=diagnostics.executable,
        classification_source=classification_source,
    )


def build_cli(workspace_root: Path, max_output_chars: int) -> Path:
    outcome = run_command(
        ["cargo", "build", "--release", "--message-format=json", "-p", "arachne-cli"],
        cwd=workspace_root,
        timeout_s=600,
        max_output_chars=max_output_chars,
    )
    if not outcome.ok:
        raise RuntimeError(f"failed to build arachne-cli:\n{outcome.error}")
    # Read the actual artifact path, including custom target layouts/configuration.
    binary = Path(outcome.artifact_path) if outcome.artifact_path else None
    if binary is None or not binary.is_file():
        raise RuntimeError(f"arachne-cli binary not found at {binary}")
    return binary


def shorten_error(error: str, max_lines: int = 8, max_chars: int = 800) -> str:
    lines = error.strip().splitlines()
    trimmed = "\n".join(lines[:max_lines])
    if len(trimmed) > max_chars:
        return trimmed[: max_chars - 3] + "..."
    return trimmed


def short_error_excerpt(error: str | None, max_chars: int = 200) -> str:
    if not error:
        return ""
    # Remove everything before the first ":"
    compact = " ".join(error.strip().split()).split(":", 1)[-1].strip()
    if len(compact) > max_chars:
        return compact[: max_chars - 3] + "..."
    return compact


def project_name_for(index: int, ecore_path: Path) -> str:
    digest = hashlib.sha1(str(ecore_path).encode("utf-8")).hexdigest()[:8]
    return f"modelset-{index:05d}-{digest}"


def format_ratio(value: int, total: int) -> str:
    if total == 0:
        return "0.0%"
    return f"{(100.0 * value / total):5.1f}%"


def render_table(summary: Summary, elapsed_s: float) -> str:
    rows = [
        ("Total .ecore", str(summary.total), "100.0%"),
        ("Parsed", str(summary.parsed), format_ratio(summary.parsed, summary.total)),
        (
            "Generated",
            str(summary.generated),
            format_ratio(summary.generated, summary.total),
        ),
        (
            "Compiled",
            str(summary.compiled),
            format_ratio(summary.compiled, summary.total),
        ),
        (
            "Gen / Parsed",
            f"{summary.generated}/{summary.parsed}",
            format_ratio(summary.generated, summary.parsed),
        ),
        (
            "Compiled / Generated",
            f"{summary.compiled}/{summary.generated}",
            format_ratio(summary.compiled, summary.generated),
        ),
        ("Checked with warnings", str(summary.compiled_with_warnings), ""),
        ("Parse failures", str(summary.parse_failure_count), ""),
        ("Generation failures", str(summary.generate_failure_count), ""),
        ("Compile failures", str(summary.compile_failure_count), ""),
        ("Parse time", f"{summary.parse_time_s:.1f}s", ""),
        ("Generation time", f"{summary.generate_time_s:.1f}s", ""),
        ("Compile time", f"{summary.compile_time_s:.1f}s", ""),
        ("Wall clock", f"{elapsed_s:.1f}s", ""),
    ]

    col1 = max(len("Metric"), max(len(row[0]) for row in rows))
    col2 = max(len("Value"), max(len(row[1]) for row in rows))
    col3 = max(len("Coverage"), max(len(row[2]) for row in rows))
    sep = f"+-{'-' * col1}-+-{'-' * col2}-+-{'-' * col3}-+"
    out = [
        sep,
        f"| {'Metric'.ljust(col1)} | {'Value'.ljust(col2)} | {'Coverage'.ljust(col3)} |",
        sep,
    ]
    for metric, value, coverage in rows:
        out.append(
            f"| {metric.ljust(col1)} | {value.ljust(col2)} | {coverage.ljust(col3)} |"
        )
    out.append(sep)
    return "\n".join(out)


def print_failures(
    title: str, failures: Sequence[tuple[Path, str]], limit: int
) -> None:
    if not failures:
        return

    print(f"\n{title}")
    for path, error in failures[:limit]:
        print(f"- {path}")
        print(f"  {shorten_error(error).replace(chr(10), chr(10) + '  ')}")


def outcome_ok(step: StepOutcome | None) -> bool:
    return step.ok if step is not None else False


def outcome_error(outcome: ModelOutcome) -> str:
    errors = []
    for stage, step in (
        ("parse", outcome.parse),
        ("generate", outcome.generate),
        ("compile", outcome.compile),
    ):
        if step is not None and not step.ok:
            errors.append(f"{stage}: {step.error or f'{stage} failed'}")
    return "\n\n".join(errors)


CSV_FIELDS = [
    "name",
    "parse",
    "generate",
    "compile",
    "error",
    "input_hash",
    "dependency_hash",
    "artifact_dir",
    "fidelity",
] + [
    f"{stage}_{field}"
    for stage in ("parse", "generate", "compile")
    for field in (
        "status",
        "failure_kind",
        "capability",
        "classification_source",
        "duration_s",
        "returncode",
        "warning_count",
        "warning_counts",
        "diagnostic_counts",
        "diagnostics",
        "diagnostics_dropped",
        "last_phase",
    )
]


def outcome_row(outcome: ModelOutcome) -> dict:
    stages = (outcome.parse, outcome.generate, outcome.compile)
    row = {
        "name": str(outcome.path),
        "error": outcome_error(outcome),
        "input_hash": outcome.input_hash,
        "dependency_hash": outcome.dependency_hash,
        "artifact_dir": outcome.artifact_dir,
        "fidelity": "unknown",
    }
    model_stages = stages[:2]
    warning_codes = {
        code.rsplit("::", 1)[-1]
        for step in model_stages
        if step
        for code in step.warning_counts
    }
    if warning_codes & {
        "feature_omitted",
        "dependency_omitted",
        "class_omitted",
        "operation_omitted",
    }:
        row["fidelity"] = "omitted"
    elif warning_codes & {"bounds_normalized", "properties_transformed"}:
        row["fidelity"] = "transformed"
    elif (
        outcome.compile
        and outcome.compile.ok
        and not any(
            step and (step.warning_count or step.diagnostics_dropped)
            for step in model_stages
        )
    ):
        row["fidelity"] = "no_known_omissions"
    for stage, step in zip(("parse", "generate", "compile"), stages):
        row[stage] = str(outcome_ok(step)).lower()
        row[f"{stage}_status"] = step.status if step else "skipped"
        for key in (
            "failure_kind",
            "capability",
            "classification_source",
            "returncode",
            "warning_count",
            "diagnostics_dropped",
            "last_phase",
        ):
            row[f"{stage}_{key}"] = getattr(step, key) if step else ""
        row[f"{stage}_duration_s"] = f"{step.duration_s:.6f}" if step else ""
        for key in ("warning_counts", "diagnostic_counts", "diagnostics"):
            row[f"{stage}_{key}"] = (
                json.dumps(getattr(step, key), ensure_ascii=False) if step else ""
            )
    return row


class CsvReport:
    """One durable row per completed model; no in-memory outcome history."""

    def __init__(self, path: Path):
        path.parent.mkdir(parents=True, exist_ok=True)
        self.file = path.open("w", newline="", encoding="utf-8")
        self.writer = csv.DictWriter(self.file, fieldnames=CSV_FIELDS)
        try:
            self.writer.writeheader()
            self.flush()
        except BaseException:
            self.file.close()
            raise

    def flush(self) -> None:
        self.file.flush()
        os.fsync(self.file.fileno())

    def add(self, outcome: ModelOutcome) -> None:
        self.writer.writerow(outcome_row(outcome))
        self.flush()

    def __enter__(self) -> CsvReport:
        return self

    def __exit__(self, *args) -> None:
        self.file.close()


def write_csv_report(path: Path, outcomes: Sequence[ModelOutcome]) -> None:
    with CsvReport(path) as report:
        for outcome in outcomes:
            report.add(outcome)


def retain_failure(
    outcome: ModelOutcome, project_dir: Path, failures_dir: Path, project_name: str
) -> None:
    destination = failures_dir / project_name
    destination.mkdir(parents=True, exist_ok=False)
    if project_dir.exists():
        shutil.move(str(project_dir), str(destination / "project"))
    copy_error = None
    try:
        shutil.copy2(outcome.path, destination / outcome.path.name)
    except OSError as exc:
        copy_error = str(exc)
    outcome.artifact_dir = str(destination)
    result = outcome_row(outcome)
    if copy_error:
        result["input_copy_error"] = copy_error
    (destination / "result.json").write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_run_metadata(
    csv_path: Path,
    workspace_root: Path,
    scan_root: Path,
    cli_binary: Path,
    args: argparse.Namespace,
) -> None:
    def command(cmd: list[str]) -> str:
        result = subprocess.run(
            cmd, cwd=workspace_root, capture_output=True, text=True, check=False
        )
        return result.stdout.strip()

    source_files = [
        workspace_root / "Cargo.toml",
        workspace_root / "Cargo.lock",
        workspace_root / "modelset_coverage.py",
    ]
    for crate in ("arachne-cli", "arachne-parser", "arachne-codegen", "arachne-xmi"):
        source_files.extend(sorted((workspace_root / crate).rglob("*.rs")))
        source_files.append(workspace_root / crate / "Cargo.toml")
    digest = hashlib.sha256()
    for path in sorted(source_files):
        digest.update(str(path.relative_to(workspace_root)).encode() + b"\0")
        digest.update(hash_file(path).encode() + b"\0")
    metadata = {
        "schema_version": 2,
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "scan_root": str(scan_root),
        "cli_binary": str(cli_binary),
        "cli_hash": hash_file(cli_binary),
        "python": platform.python_version(),
        "platform": platform.platform(),
        "source_hash": digest.hexdigest(),
        "revision": command(["git", "rev-parse", "HEAD"]),
        "worktree": command(["git", "status", "--porcelain"]),
        "rustc": command(["rustc", "--version"]),
        "cargo": command(["cargo", "--version"]),
        "environment": {
            k: v
            for k, v in os.environ.items()
            if k.startswith(("CARGO_PROFILE_", "CARGO_BUILD_"))
            or k in ("CARGO_NET_OFFLINE", "CARGO_TARGET_DIR", "RUSTFLAGS", "RUST_LOG")
        },
        "options": {
            k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()
        },
    }
    csv_path.with_suffix(csv_path.suffix + ".run.json").write_text(
        json.dumps(metadata, indent=2) + "\n", encoding="utf-8"
    )


def clean_generated_artifacts(
    *,
    project_dir: Path,
    project_name: str,
    shared_target_dir: Path,
    workspace_root: Path,
    max_output_chars: int,
) -> StepOutcome | None:
    manifest_path = project_dir / "Cargo.toml"
    if not manifest_path.exists():
        return None

    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(shared_target_dir)
    return run_command(
        [
            "cargo",
            "clean",
            "--quiet",
            "--manifest-path",
            str(manifest_path),
            "-p",
            project_name,
        ],
        cwd=workspace_root,
        env=env,
        timeout_s=CARGO_CLEAN_TIMEOUT_S,
        max_output_chars=max_output_chars,
    )


def main() -> int:
    args = parse_args()
    workspace_root = Path(__file__).resolve().parent
    scan_root = Path(args.root).expanduser().resolve()

    if not scan_root.is_dir():
        print(f"Scan root does not exist: {scan_root}", file=sys.stderr)
        return 1

    if args.show_failures < 0:
        print("--show-failures must be >= 0", file=sys.stderr)
        return 1
    if args.max_error_chars < 0:
        print("--max-error-chars must be >= 0", file=sys.stderr)
        return 1

    for name in ("parse_timeout", "generate_timeout", "compile_timeout"):
        if getattr(args, name) <= 0:
            print(f"--{name.replace('_', '-')} must be > 0", file=sys.stderr)
            return 1
    csv_path = args.csv.expanduser().resolve()
    failures_dir = (
        (args.failures_dir or csv_path.with_name(csv_path.stem + "-failures"))
        .expanduser()
        .resolve()
    )
    ecore_files = find_ecore_files(scan_root, exclude=(failures_dir,))
    if not ecore_files:
        print(f"No .ecore files found under {scan_root}", file=sys.stderr)
        return 1
    if args.offset < 0:
        print("--offset must be >= 0", file=sys.stderr)
        return 1
    if args.offset:
        ecore_files = ecore_files[args.offset :]
    if args.limit is not None:
        if args.limit < 0:
            print("--limit must be >= 0", file=sys.stderr)
            return 1
        ecore_files = ecore_files[: args.limit]

    if not ecore_files:
        print("No files selected after --offset/--limit", file=sys.stderr)
        return 1
    started = time.perf_counter()
    cli_binary = build_cli(workspace_root, args.max_error_chars)
    temp_root = Path(tempfile.mkdtemp(prefix="arachne-modelset-"))
    shared_target_dir = temp_root / "cargo-target"
    shared_target_dir.mkdir(parents=True, exist_ok=True)

    print(f"CLI binary   : {cli_binary}")
    print(f"Scan root    : {scan_root}")
    print(f"Offset       : {args.offset}")
    print(f"Ecore files  : {len(ecore_files)}")
    print(f"Temp root    : {temp_root}")
    print(f"Cargo target : {shared_target_dir}")
    print(f"Error cap    : {args.max_error_chars} chars per failed step")
    cargo_clean_status = (
        "disabled" if args.skip_cargo_clean else "generated package after each compile"
    )
    print(f"Cargo clean  : {cargo_clean_status}")
    print("")

    summary = Summary(failure_sample_limit=args.show_failures)

    def record(outcome: ModelOutcome, project_dir: Path, project_name: str) -> None:
        if args.keep_failures and any(
            step and not step.ok
            for step in (outcome.parse, outcome.generate, outcome.compile)
        ):
            retain_failure(
                outcome, project_dir, failures_dir / temp_root.name, project_name
            )
        report.add(outcome)
        summary.add(outcome)

    cleanup_warning_printed = False

    try:
        csv_path.parent.mkdir(parents=True, exist_ok=True)
        write_run_metadata(csv_path, workspace_root, scan_root, cli_binary, args)
        with CsvReport(csv_path) as report:
            for index, ecore_path in enumerate(ecore_files, start=1):
                outcome = ModelOutcome(path=ecore_path)
                rel = ecore_path.relative_to(scan_root)
                print(f"[{index}/{len(ecore_files)}] {rel}")

                project_dir = temp_root / f"generated-{index:05d}"
                project_name = project_name_for(index, ecore_path)

                try:
                    outcome.input_hash = hash_file(ecore_path)
                except OSError as exc:
                    outcome.parse = StepOutcome(
                        False,
                        0,
                        str(exc),
                        failure_kind="infrastructure",
                        capability="file_read",
                        classification_source="process",
                    )
                    record(outcome, project_dir, project_name)
                    continue
                outcome.parse = run_command(
                    [
                        str(cli_binary),
                        "parse",
                        str(ecore_path),
                        "--quiet",
                        "--diagnostics-json",
                    ],
                    cwd=workspace_root,
                    stage="parse",
                    timeout_s=args.parse_timeout,
                    max_output_chars=args.max_error_chars,
                )
                if not outcome.parse.ok:
                    print(
                        f"  parse    : FAIL - {short_error_excerpt(outcome.parse.error)}"
                    )
                    record(outcome, project_dir, project_name)
                    continue
                print("  parse    : OK")

                outcome.generate = run_command(
                    [
                        str(cli_binary),
                        "generate",
                        "--diagnostics-json",
                        str(ecore_path),
                        "-o",
                        str(project_dir),
                        "-p",
                        project_name,
                    ],
                    cwd=workspace_root,
                    stage="generate",
                    timeout_s=args.generate_timeout,
                    max_output_chars=args.max_error_chars,
                )
                if not outcome.generate.ok:
                    print(
                        f"  generate : FAIL - {short_error_excerpt(outcome.generate.error)}"
                    )
                    record(outcome, project_dir, project_name)
                    if project_dir.exists() and not args.keep_failures:
                        shutil.rmtree(project_dir, ignore_errors=True)
                    continue
                print("  generate : OK")

                compile_env = os.environ.copy()
                compile_env["CARGO_TARGET_DIR"] = str(shared_target_dir)
                outcome.compile = run_command(
                    [
                        "cargo",
                        "check",
                        "--message-format=json",
                        "--manifest-path",
                        str(project_dir / "Cargo.toml"),
                    ],
                    cwd=workspace_root,
                    env=compile_env,
                    stage="compile",
                    timeout_s=args.compile_timeout,
                    max_output_chars=args.max_error_chars,
                )
                if outcome.compile.ok:
                    print("  compile  : OK")
                else:
                    compile_reason = short_error_excerpt(outcome.compile.error)
                    print(f"  compile  : FAIL - {short_error_excerpt(compile_reason)}")

                lockfile = project_dir / "Cargo.lock"
                if lockfile.is_file():
                    outcome.dependency_hash = hash_file(lockfile)
                record(outcome, project_dir, project_name)

                if not args.skip_cargo_clean:
                    clean_outcome = clean_generated_artifacts(
                        project_dir=Path(outcome.artifact_dir) / "project"
                        if outcome.artifact_dir
                        else project_dir,
                        project_name=project_name,
                        shared_target_dir=shared_target_dir,
                        workspace_root=workspace_root,
                        max_output_chars=args.max_error_chars,
                    )
                    if (
                        clean_outcome is not None
                        and not clean_outcome.ok
                        and not cleanup_warning_printed
                    ):
                        print(
                            f"  cleanup  : WARN - {short_error_excerpt(clean_outcome.error)}"
                        )
                        cleanup_warning_printed = True

                if project_dir.exists() and (
                    outcome.compile.ok or not args.keep_failures
                ):
                    shutil.rmtree(project_dir, ignore_errors=True)

            elapsed_s = time.perf_counter() - started
            print("")
            print(render_table(summary, elapsed_s))
            if summary.failure_groups:
                print("\nFailure categories (first failing stage per model):")
                for (
                    stage,
                    kind,
                    capability,
                ), count in summary.failure_groups.most_common():
                    print(f"  {count:5d}  {stage}: {kind} / {capability}")
            print_failures("Parse failures", summary.parse_failures, args.show_failures)
            print_failures(
                "Generation failures", summary.generate_failures, args.show_failures
            )
            print_failures(
                "Compile failures", summary.compile_failures, args.show_failures
            )
            return 0
    finally:
        shutil.rmtree(temp_root, ignore_errors=True)
        print(f"\nCSV report   : {csv_path}")
        if args.keep_failures:
            print(f"Failures     : {failures_dir / temp_root.name}")


if __name__ == "__main__":
    raise SystemExit(main())
