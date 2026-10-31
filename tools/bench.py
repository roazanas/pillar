#!/usr/bin/env python3
"""Record frontend/build/runtime wall time and artifact size; emit JSON for comparison."""
import argparse
import json
from pathlib import Path
import statistics
import subprocess
import tempfile
import time
import sys
try:
    import resource
except ImportError:
    resource = None

def run(command, count, stdin=None, expected=None):
    samples = []
    for _ in range(count):
        before = time.perf_counter()
        result = subprocess.run(command, input=stdin, capture_output=True, text=True, timeout=120)
        assert result.returncode == 0, result.stderr
        if expected is not None:
            assert result.stdout == expected, result.stdout
        samples.append(time.perf_counter() - before)
    return statistics.median(samples)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', default='target/release/plrc')
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--functions', type=int, nargs='+', default=[10, 100, 1000])
    args = parser.parse_args()
    compiler = str(Path(args.compiler).resolve())
    results = []
    with tempfile.TemporaryDirectory(prefix='pillar-benchmark-') as directory:
        directory = Path(directory)
        for count in args.functions:
            assert 0 < count <= 10000 and args.repeats > 0
            horizontal = '\n'.join(f'FN f{i}(x: int): int {{ RET x + {i}~ }}' for i in range(count)) + f'\nFN main(): int\n{{\n  OUTI f{count-1}(1)~\n  LT i: int = 0~\n  LT sum: int = 0~\n  WH i < 1000000\n  {{\n    sum = sum + i~\n    i = i + 1~\n  }}\n  OUTI sum~\n  RET 0~\n}}\n'
            rows = horizontal.splitlines()
            source = '\n'.join(' '.join(row[i] if i < len(row) else ' ' for row in rows) for i in range(max(map(len, rows)))) + '\n'
            path, exe = directory / 'benchmark.plr', directory / 'benchmark.exe'
            path.write_text(source)
            check = run([compiler, 'check', str(path)], args.repeats)
            build = run([compiler, 'build', str(path), '--release', '-o', str(exe)], args.repeats)
            runtime = run([str(exe)], args.repeats, expected=f"{count}\n499999500000\n")
            results.append({'functions': count, 'source_bytes': len(source.encode()), 'check_seconds': check, 'build_seconds': build, 'run_seconds': runtime, 'binary_bytes': exe.stat().st_size, 'cumulative_peak_child_rss_bytes': (int(resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss) * (1 if sys.platform == 'darwin' else 1024)) if resource else None})
    print(json.dumps(results, indent=2))
