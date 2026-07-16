#!/usr/bin/env python3

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
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
DIAGNOSTIC_BUILD_TIMEOUT_S = 600
ANSI_ESCAPE_RE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
RUST_ERROR_RE = re.compile(r"^error(?:\[(E\d{4})\])?:\s*(.+)$")
IGNORED_RUST_ERROR_PREFIXES = (
    "aborting due to",
    "could not compile ",
)

@dataclass
class OutputCapture:
    chunks: list[str] = field(default_factory=list)
    captured_chars: int = 0
    truncated: bool = False

    def add(self, chunk: str, max_chars: int) -> None:
        remaining = max_chars - self.captured_chars
        if remaining > 0:
            kept = chunk[:remaining]
            self.chunks.append(kept)
            self.captured_chars += len(kept)
            if len(chunk) > remaining:
                self.truncated = True
        else:
            self.truncated = True

    def render(self) -> str:
        output = "".join(self.chunks).strip()
        if self.truncated:
            suffix = f"[output truncated after {self.captured_chars} chars]"
            return f"{output}\n{suffix}" if output else suffix
        return output

@dataclass
class StepOutcome:
    ok: bool
    duration_s: float
    error: str | None = None

@dataclass
class ModelOutcome:
    path: Path
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
        help="Keep generated project directories for failing metamodels",
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
            "Write per-model results to this CSV file with columns "
            "name, parse, generate, compile, error "
            "(default: modelset_coverage.csv)"
        ),
    )
    return parser.parse_args()


def find_ecore_files(root: Path) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*")
        if path.is_file() and path.suffix.lower() == ".ecore"
    )


def drain_limited(stream: TextIO, capture: OutputCapture, max_chars: int) -> None:
    while True:
        chunk = stream.read(4096)
        if not chunk:
            break
        capture.add(chunk, max_chars)


def kill_process_tree(process: subprocess.Popen[str]) -> None:
    if os.name == "nt":
        process.kill()
        return

    try:
        os.killpg(process.pid, signal.SIGKILL)
    except OSError:
        pass


def run_command(
    cmd: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    timeout_s: int,
    max_output_chars: int = DEFAULT_MAX_ERROR_CHARS,
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
            ok=False,
            duration_s=time.perf_counter() - start,
            error=f"failed to start {' '.join(cmd)}: {exc}",
        )

    capture = OutputCapture()
    assert process.stdout is not None
    reader = threading.Thread(
        target=drain_limited,
        args=(process.stdout, capture, max_output_chars),
        daemon=True,
    )
    reader.start()

    timed_out = False
    try:
        returncode = process.wait(timeout=timeout_s)
    except subprocess.TimeoutExpired:
        timed_out = True
        kill_process_tree(process)
        returncode = process.wait()

    reader.join()
    duration_s = time.perf_counter() - start
    output = capture.render()

    if timed_out:
        error = f"timeout after {timeout_s}s: {' '.join(cmd)}"
        if output:
            error = f"{error}\n{output}"
        return StepOutcome(
            ok=False,
            duration_s=duration_s,
            error=error,
        )

    if returncode == 0:
        return StepOutcome(ok=True, duration_s=duration_s)

    error = output
    if not error:
        error = f"command exited with code {returncode}"
    return StepOutcome(ok=False, duration_s=duration_s, error=error)


def build_cli(workspace_root: Path, max_output_chars: int) -> Path:
    outcome = run_command(
        ["cargo", "build", "--release", "-q", "-p", "arachne-cli"],
        cwd=workspace_root,
        timeout_s=600,
        max_output_chars=max_output_chars,
    )
    if not outcome.ok:
        raise RuntimeError(f"failed to build arachne-cli:\n{outcome.error}")

    binary = workspace_root / "target" / "release" / ("arachne.exe" if os.name == "nt" else "arachne")
    if not binary.exists():
        raise RuntimeError(f"arachne-cli binary not found at {binary}")
    return binary


def write_parse_diagnostic_project(project_dir: Path, parser_crate: Path) -> Path:
    src_dir = project_dir / "src"
    src_dir.mkdir(parents=True, exist_ok=True)

    manifest_path = project_dir / "Cargo.toml"
    manifest_path.write_text(
        "\n".join(
            [
                "[package]",
                'name = "arachne-parse-diagnostic"',
                'version = "0.1.0"',
                'edition = "2021"',
                "",
                "[dependencies]",
                f"ecore_rs = {{ path = {json.dumps(str(parser_crate))} }}",
                "",
            ]
        ),
        encoding="utf-8",
    )

    (src_dir / "main.rs").write_text(
        """
use std::{env, fs, process};

use ecore_rs::ctx::Ctx;

fn main() {
    let Some(path) = env::args_os().nth(1) else {
        eprintln!("missing .ecore path argument");
        process::exit(2);
    };

    let path_display = path.to_string_lossy();
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("failed to read {path_display}: {err}");
            process::exit(1);
        }
    };

    if let Err(err) = Ctx::parse(&content) {
        eprintln!("failed to parse {path_display}: {err}");
        process::exit(1);
    }
}
""".lstrip(),
        encoding="utf-8",
    )

    return manifest_path


def diagnostic_binary_path(shared_target_dir: Path) -> Path:
    binary_name = (
        "arachne-parse-diagnostic.exe"
        if os.name == "nt"
        else "arachne-parse-diagnostic"
    )
    return shared_target_dir / "release" / binary_name


def ensure_parse_diagnostic_binary(
    *,
    workspace_root: Path,
    temp_root: Path,
    shared_target_dir: Path,
    max_output_chars: int,
) -> Path:
    binary = diagnostic_binary_path(shared_target_dir)
    if binary.exists():
        return binary

    manifest_path = write_parse_diagnostic_project(
        temp_root / "parse-diagnostic",
        workspace_root / "arachne-parser",
    )
    workspace_lockfile = workspace_root / "Cargo.lock"
    if workspace_lockfile.exists():
        shutil.copy2(workspace_lockfile, manifest_path.parent / "Cargo.lock")

    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(shared_target_dir)
    outcome = run_command(
        [
            "cargo",
            "build",
            "--release",
            "--quiet",
            "--offline",
            "--manifest-path",
            str(manifest_path),
        ],
        cwd=workspace_root,
        env=env,
        timeout_s=DIAGNOSTIC_BUILD_TIMEOUT_S,
        max_output_chars=max_output_chars,
    )
    if not outcome.ok:
        raise RuntimeError(f"failed to build parse diagnostic runner:\n{outcome.error}")

    if not binary.exists():
        raise RuntimeError(f"parse diagnostic binary not found at {binary}")

    return binary


def should_diagnose_parse_failure(error: str | None) -> bool:
    return not error or error.startswith("command exited with code ")


def diagnose_parse_failure(
    *,
    ecore_path: Path,
    workspace_root: Path,
    temp_root: Path,
    shared_target_dir: Path,
    parse_diagnostic_binary: Path | None,
    timeout_s: int,
    max_output_chars: int,
) -> tuple[Path | None, StepOutcome]:
    if parse_diagnostic_binary is None:
        parse_diagnostic_binary = ensure_parse_diagnostic_binary(
            workspace_root=workspace_root,
            temp_root=temp_root,
            shared_target_dir=shared_target_dir,
            max_output_chars=max_output_chars,
        )

    outcome = run_command(
        [str(parse_diagnostic_binary), str(ecore_path)],
        cwd=workspace_root,
        timeout_s=timeout_s,
        max_output_chars=max_output_chars,
    )
    return parse_diagnostic_binary, outcome


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


def rust_compile_error_names(error: str | None, limit: int = 3) -> list[str]:
    if not error:
        return []

    names: list[str] = []
    seen: set[str] = set()
    for raw_line in error.splitlines():
        line = ANSI_ESCAPE_RE.sub("", raw_line).strip()
        match = RUST_ERROR_RE.match(line)
        if match is None:
            continue

        code, message = match.groups()
        message = " ".join(message.strip().split())
        if any(message.startswith(prefix) for prefix in IGNORED_RUST_ERROR_PREFIXES):
            continue

        name = f"{code}: {message}" if code else message
        if name in seen:
            continue

        seen.add(name)
        names.append(name)
        if len(names) >= limit:
            break

    return names


def rust_compile_failure_reason(error: str | None) -> str:
    names = rust_compile_error_names(error)
    if names:
        return "; ".join(names)
    return short_error_excerpt(error, max_chars=160) or "compile failed"


def annotate_compile_failure(error: str | None, reason: str) -> str:
    if not error:
        return f"Rust error: {reason}"
    return f"Rust error: {reason}\n{error}"


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
        ("Generated", str(summary.generated), format_ratio(summary.generated, summary.total)),
        ("Compiled", str(summary.compiled), format_ratio(summary.compiled, summary.total)),
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
        ("Parse failures", str(summary.parse_failure_count), ""),
        ("Generation failures", str(summary.generate_failure_count), ""),
        ("Compile failures", str(summary.compile_failure_count), ""),
        ("Parse time", f"{summary.parse_time_s:.1f}s", ""),
        ("Generation time", f"{summary.generate_time_s:.1f}s", ""),
        ("Compile time", f"{summary.compile_time_s:.1f}s", ""),
        ("Wall clock", f"{elapsed_s:.1f}s", ""),
    ]

    col1 = max(len(row[0]) for row in rows)
    col2 = max(len(row[1]) for row in rows)
    col3 = max(len(row[2]) for row in rows)
    sep = f"+-{'-' * col1}-+-{'-' * col2}-+-{'-' * col3}-+"
    out = [sep, f"| {'Metric'.ljust(col1)} | {'Value'.ljust(col2)} | {'Coverage'.ljust(col3)} |", sep]
    for metric, value, coverage in rows:
        out.append(
            f"| {metric.ljust(col1)} | {value.ljust(col2)} | {coverage.ljust(col3)} |"
        )
    out.append(sep)
    return "\n".join(out)


def print_failures(title: str, failures: Sequence[tuple[Path, str]], limit: int) -> None:
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


def write_csv_report(path: Path, outcomes: Sequence[ModelOutcome]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as file:
        writer = csv.DictWriter(
            file,
            fieldnames=["name", "parse", "generate", "compile", "error"],
        )
        writer.writeheader()
        for outcome in outcomes:
            writer.writerow(
                {
                    "name": str(outcome.path),
                    "parse": str(outcome_ok(outcome.parse)).lower(),
                    "generate": str(outcome_ok(outcome.generate)).lower(),
                    "compile": str(outcome_ok(outcome.compile)).lower(),
                    "error": outcome_error(outcome),
                }
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

    if not scan_root.exists():
        print(f"Scan root does not exist: {scan_root}", file=sys.stderr)
        return 1

    if args.show_failures < 0:
        print("--show-failures must be >= 0", file=sys.stderr)
        return 1
    if args.max_error_chars < 0:
        print("--max-error-chars must be >= 0", file=sys.stderr)
        return 1

    ecore_files = find_ecore_files(scan_root)
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
    outcomes: list[ModelOutcome] = []
    started = time.perf_counter()
    cleanup_warning_printed = False
    parse_diagnostic_binary: Path | None = None

    try:
        for index, ecore_path in enumerate(ecore_files, start=1):
            outcome = ModelOutcome(path=ecore_path)
            rel = ecore_path.relative_to(scan_root)
            print(f"[{index}/{len(ecore_files)}] {rel}")

            project_dir = temp_root / f"generated-{index:05d}"
            project_name = project_name_for(index, ecore_path)

            outcome.parse = run_command(
                [str(cli_binary), "parse", str(ecore_path), "--quiet"],
                cwd=workspace_root,
                timeout_s=args.parse_timeout,
                max_output_chars=args.max_error_chars,
            )
            if not outcome.parse.ok:
                if should_diagnose_parse_failure(outcome.parse.error):
                    original_error = outcome.parse.error or "parse command failed"
                    try:
                        parse_diagnostic_binary, diagnostic = diagnose_parse_failure(
                            ecore_path=ecore_path,
                            workspace_root=workspace_root,
                            temp_root=temp_root,
                            shared_target_dir=shared_target_dir,
                            parse_diagnostic_binary=parse_diagnostic_binary,
                            timeout_s=args.parse_timeout,
                            max_output_chars=args.max_error_chars,
                        )
                        outcome.parse.duration_s += diagnostic.duration_s
                        if diagnostic.error:
                            outcome.parse.error = diagnostic.error
                        elif diagnostic.ok:
                            outcome.parse.error = (
                                "CLI parse failed, but direct parser diagnostic succeeded.\n"
                                f"Original CLI error: {original_error}"
                            )
                    except RuntimeError as exc:
                        outcome.parse.error = f"{original_error}\n{exc}"
                print(f"  parse    : FAIL - {short_error_excerpt(outcome.parse.error)}")
                summary.add(outcome)
                outcomes.append(outcome)
                continue
            print("  parse    : OK")

            outcome.generate = run_command(
                [
                    str(cli_binary),
                    "generate",
                    str(ecore_path),
                    "-o",
                    str(project_dir),
                    "-p",
                    project_name,
                ],
                cwd=workspace_root,
                timeout_s=args.generate_timeout,
                max_output_chars=args.max_error_chars,
            )
            if not outcome.generate.ok:
                print(f"  generate : FAIL - {short_error_excerpt(outcome.generate.error)}")
                summary.add(outcome)
                outcomes.append(outcome)
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
                    "--quiet",
                    "--manifest-path",
                    str(project_dir / "Cargo.toml"),
                ],
                cwd=workspace_root,
                env=compile_env,
                timeout_s=args.compile_timeout,
                max_output_chars=args.max_error_chars,
            )
            if outcome.compile.ok:
                print("  compile  : OK")
            else:
                compile_reason = rust_compile_failure_reason(outcome.compile.error)
                outcome.compile.error = annotate_compile_failure(
                    outcome.compile.error,
                    compile_reason,
                )
                print(f"  compile  : FAIL - {short_error_excerpt(compile_reason)}")

            if not args.skip_cargo_clean:
                clean_outcome = clean_generated_artifacts(
                    project_dir=project_dir,
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
                    print(f"  cleanup  : WARN - {short_error_excerpt(clean_outcome.error)}")
                    cleanup_warning_printed = True

            summary.add(outcome)
            outcomes.append(outcome)

            if project_dir.exists() and (outcome.compile.ok or not args.keep_failures):
                shutil.rmtree(project_dir, ignore_errors=True)

        elapsed_s = time.perf_counter() - started
        print("")
        print(render_table(summary, elapsed_s))
        print_failures("Parse failures", summary.parse_failures, args.show_failures)
        print_failures("Generation failures", summary.generate_failures, args.show_failures)
        print_failures("Compile failures", summary.compile_failures, args.show_failures)
        return 0
    finally:
        if args.csv is not None:
            csv_path = args.csv.expanduser().resolve()
            write_csv_report(csv_path, outcomes)
            print(f"\nCSV report   : {csv_path}")
        shutil.rmtree(temp_root, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
