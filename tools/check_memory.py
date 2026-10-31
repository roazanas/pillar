#!/usr/bin/env python3
"""Run generated native programs against an ASan/UBSan-instrumented C runtime."""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CASES = [
    ('FN copy(s: str): str { RET s + "!"~ } FN main(): int { LT a: [str] = ["old"]~ LT b: [str] = a~ LT i: int = 0~ WH i < 50 { b[0] = copy(a[0])~ i = i + 1~ IF i < 49 { continue~ } break~ } println(b[0])~ RET 0~ }', '', 0),
    ('ST P { a: [[str]] } FN f(p: P): P { RET p~ } FN main(): int { LT a: P = P { a: [["x"]] }~ LT b: P = f(a)~ b.a[0][0] = "y"~ println(a.a[0][0])~ println(b.a[0][0])~ RET 0~ }', '', 0),
    ('FN main(): int { LT s: str = "retained"~ OUTI 9223372036854775807 + 1~ RET 0~ }', '', 1),
    ('FN main(): int { println(read_line())~ RET 0~ }', '\udcff', 1),
]

def vertical(text):
    rows = text.splitlines()
    return '\n'.join(' '.join(row[i] if i < len(row) else ' ' for row in rows) for i in range(max(map(len, rows), default=0))) + '\n'

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', default='target/debug/plrc')
    parser.add_argument('--cc', default='clang')
    args = parser.parse_args()
    compiler = Path(args.compiler).resolve()
    env = os.environ.copy()
    env['ASAN_OPTIONS'] = 'detect_leaks=1' if sys.platform.startswith('linux') else 'detect_leaks=0'
    env['UBSAN_OPTIONS'] = 'halt_on_error=1'
    with tempfile.TemporaryDirectory(prefix='pillar-sanitizers-') as directory:
        directory = Path(directory)
        for i, (source, stdin, expected) in enumerate(CASES):
            path = directory / f'case{i}.plr'
            path.write_text(vertical(source))
            obj, exe = directory / 'program.o', directory / 'program'
            subprocess.run([str(compiler), 'build', str(path), '--emit', 'obj', '-o', str(obj)], check=True)
            subprocess.run([args.cc, '-std=c11', '-g', '-O1', '-fsanitize=address,undefined', '-fno-omit-frame-pointer', str(obj), str(ROOT / 'runtime.c'), '-lm', '-o', str(exe)], check=True)
            result = subprocess.run([str(exe)], input=stdin.encode('utf-8', 'surrogateescape'), capture_output=True, timeout=10, env=env)
            stderr = result.stderr.decode('utf-8', 'replace')
            assert result.returncode == expected, (i, result.returncode, stderr)
            assert not any(marker in stderr for marker in ['ERROR: AddressSanitizer', 'LeakSanitizer', 'runtime error:']), stderr
            print(f'sanitizer case {i}: OK')
