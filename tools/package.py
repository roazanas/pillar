#!/usr/bin/env python3
"""Build a deterministic, self-contained native archive and smoke-test its extraction."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
SHELLS = {"bash": "plrc.bash", "zsh": "_plrc", "fish": "plrc.fish", "powershell": "_plrc.ps1"}

def notices(stage, target):
    metadata = json.loads(subprocess.run(["cargo", "metadata", "--locked", "--offline", "--format-version", "1", "--filter-platform", target], cwd=ROOT, check=True, capture_output=True, text=True).stdout)
    records = []
    for package in metadata["packages"]:
        if package["name"] == "pillar":
            continue
        directory = Path(package["manifest_path"]).parent
        candidates = set()
        for pattern in ["LICENSE*", "LICENCE*", "COPYING*", "NOTICE*"]:
            candidates.update(directory.glob(pattern))
        if package.get("license_file"):
            candidate = (directory / package["license_file"]).resolve()
            if candidate.is_relative_to(directory.resolve()):
                candidates.add(candidate)
        copied = []
        for source in sorted(candidates):
            if source.is_file():
                destination = stage / "licenses" / f'{package["name"]}-{package["version"]}' / source.name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, destination)
                copied.append(str(destination.relative_to(stage)))
        records.append({"name": package["name"], "version": package["version"], "license": package.get("license"), "repository": package.get("repository"), "notices": copied})
    sysroot = Path(subprocess.run(["rustc", "--print", "sysroot"], check=True, capture_output=True, text=True).stdout.strip())
    rust_notices = sysroot / "share/doc/rust"
    destination = stage / "licenses/rust"
    if (rust_notices / "licenses").is_dir():
        shutil.copytree(rust_notices / "licenses", destination / "licenses")
    if (rust_notices / "COPYRIGHT-library.html").is_file():
        destination.mkdir(parents=True, exist_ok=True)
        shutil.copy2(rust_notices / "COPYRIGHT-library.html", destination / "COPYRIGHT-library.html")
    (stage / "THIRD_PARTY.json").write_text(json.dumps(records, indent=2) + "\n")
    shutil.copy2(ROOT / "Cargo.lock", stage / "Cargo.lock")

def package(binary, target, destination):
    binary, destination = Path(binary).resolve(), Path(destination).resolve()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    identity = subprocess.run([str(binary), "--version"], check=True, capture_output=True, text=True, timeout=10).stdout.splitlines()
    assert identity[0] == "plrc " + version and "target: " + target in identity, ("binary identity does not match package", identity, version, target)
    name = f"pillar-{version}-{target}"
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="pillar-package-") as directory:
        stage = Path(directory) / name
        stage.mkdir()
        executable = stage / ("plrc.exe" if "windows" in target else "plrc")
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        for filename in ["README.md", "LICENSE", "CHANGELOG.md"]:
            shutil.copy2(ROOT / filename, stage / filename)
        shutil.copytree(ROOT / "docs", stage / "docs")
        shutil.copytree(ROOT / "media", stage / "media")
        shutil.copytree(ROOT / "examples", stage / "examples")
        (stage / "completions").mkdir()
        for shell, filename in SHELLS.items():
            result = subprocess.run([str(binary), "completions", shell], check=True, capture_output=True, timeout=20)
            (stage / "completions" / filename).write_bytes(result.stdout)
        notices(stage, target)
        files = {str(p.relative_to(stage)).replace(os.sep, "/"): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(stage.rglob("*")) if p.is_file()}
        (stage / "MANIFEST.json").write_text(json.dumps({"version": version, "target": target, "files": files}, indent=2) + "\n")
        suffix = ".zip" if "windows" in target else ".tar.gz"
        artifact = destination / (name + suffix)
        if suffix == ".zip":
            with zipfile.ZipFile(artifact, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                for p in sorted(stage.rglob("*")):
                    if p.is_file():
                        entry = zipfile.ZipInfo(str(p.relative_to(stage.parent)).replace(os.sep, "/"), (1980, 1, 1, 0, 0, 0))
                        entry.external_attr = (0o755 if p == executable else 0o644) << 16
                        archive.writestr(entry, p.read_bytes(), compress_type=zipfile.ZIP_DEFLATED)
        else:
            with artifact.open("wb") as raw, gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as compressed, tarfile.open(fileobj=compressed, mode="w") as archive:
                for p in sorted(stage.rglob("*")):
                    if p.is_file():
                        data = p.read_bytes()
                        entry = tarfile.TarInfo(str(p.relative_to(stage.parent)))
                        entry.size, entry.mtime = len(data), 0
                        entry.mode = 0o755 if p == executable else 0o644
                        archive.addfile(entry, io.BytesIO(data))
        digest = hashlib.sha256(artifact.read_bytes()).hexdigest()
        artifact.with_name(artifact.name + ".sha256").write_text(f"{digest}  {artifact.name}\n")
        smoke(artifact, name, "windows" in target)
        return artifact

def smoke(artifact, name, windows):
    with tempfile.TemporaryDirectory(prefix="pillar installed ") as directory:
        directory = Path(directory)
        if windows:
            with zipfile.ZipFile(artifact) as archive:
                archive.extractall(directory)
        else:
            with tarfile.open(artifact) as archive:
                archive.extractall(directory, filter="data")
        installed = directory / name
        manifest = json.loads((installed / "MANIFEST.json").read_text())
        for filename, expected in manifest["files"].items():
            assert hashlib.sha256((installed / filename).read_bytes()).hexdigest() == expected, filename
        compiler = installed / ("plrc.exe" if windows else "plrc")
        compiler.chmod(0o755)
        clean = directory / "empty working directory"
        clean.mkdir()
        for example, stdin, expected in [
            ("fact", "5\n", "120\n"), ("fib", "10\n", "55\n"),
            ("test", "", "5\n"),
            ("mult", "", "".join(f"{i*j}\n" for i in range(1, 11) for j in range(1, 11))),
            ("complex", "6\n2\n2.0\n0.0\n", "6\n4\n3\n1\n1\n1\n12.5664\n32\n"),
        ]:
            result = subprocess.run([str(compiler), "run", str(installed / "examples" / (example + ".plr"))], input=stdin, text=True, capture_output=True, cwd=clean, timeout=30)
            assert result.returncode == 0 and result.stdout == expected, (example, result.stderr, result.stdout)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", default="target/packages")
    args = parser.parse_args()
    print(package(args.binary, args.target, args.output))
