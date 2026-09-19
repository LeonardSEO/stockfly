#!/usr/bin/env python3
"""Build a portable ZIP in isolated staging; never mutate active browser/model caches."""
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(*command):
    print('+', ' '.join(map(str, command)), flush=True)
    subprocess.run(list(map(str, command)), cwd=ROOT, check=True)


def copy_tree(source, target):
    # Only explicitly selected runtime directories reach this function. Nested links
    # are rejected; the selected source root itself may be a shared local cache.
    source = source.resolve(strict=True)
    for path in source.rglob('*'):
        if path.is_symlink():
            raise ValueError(f'Unexpected symlink in runtime source: {path}')
    shutil.copytree(source, target, dirs_exist_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--without-models', action='store_true', help='Build a download-first runtime; fetching needs an existing published model release')
    parser.add_argument('--out', type=Path, default=ROOT / 'dist/releases', help='Output directory within this repository\'s dist/ tree (required for workspace dependency resolution)')
    parser.add_argument('--stockfish-source-archive', type=Path, help='Reuse an already downloaded upstream source archive (hash still checked)')
    args = parser.parse_args()
    system, machine = platform.system(), platform.machine().lower()
    if system == 'Darwin' and machine in ('arm64', 'aarch64'):
        name, exe = 'stockfly-macos-arm64', 'stockfly-server'
    elif system == 'Windows' and machine in ('amd64', 'x86_64'):
        name, exe = 'stockfly-windows-x64', 'stockfly-server.exe'
    else:
        raise SystemExit('Supported local packaging hosts: macOS arm64 or Windows x64')
    out = args.out.resolve()
    if not out.is_relative_to(ROOT.resolve() / 'dist'):
        raise SystemExit('--out must be inside this repository\'s dist/ directory so the staged app can resolve workspace node_modules')
    out.mkdir(parents=True, exist_ok=True)
    # Each run owns a new directory, preserving previous builds and active Vite files.
    stage = Path(tempfile.mkdtemp(prefix=f'{name}-', dir=out))
    bundle = stage / 'stockfly'
    app = stage / 'build-app'
    bundle.mkdir()
    shutil.copytree(ROOT / 'apps/web', app, ignore=shutil.ignore_patterns('node_modules', 'dist', 'public', 'wasm-gen'))
    (app / 'public').mkdir()
    shutil.copy2(ROOT / 'apps/web/public/favicon.svg', app / 'public/favicon.svg')
    copy_tree(ROOT / 'apps/web/public/pieces', app / 'public/pieces')
    stockfish = app / 'public/vendor/stockfish'
    stockfish.mkdir(parents=True)
    if args.stockfish_source_archive:
        shutil.copyfile(args.stockfish_source_archive, stockfish / 'stockfish-js-v19.0.0-source.tar.gz')
    run('python' if system == 'Windows' else 'python3', 'tools/browser/fetch_stockfish.py', '--out', stockfish, '--source-dir', ROOT / 'data/vendor/stockfish-19-lite')
    copy_tree(ROOT / 'docs/results', bundle / 'docs/results')
    # Source maps are tracked; no raw data or map regeneration is needed for a build.
    copy_tree(ROOT / 'crates/stockfly-chess/resources', bundle / 'data/chess-maps')
    if not args.without_models:
        catalog = json.loads((ROOT / 'data/browser-models/catalog.json').read_text())
        selected = {m['id']: ROOT / 'data/browser-models/checkpoints' / Path(m['checkpointUrl']).name
                    for m in catalog['models'] if m['availability'] == 'available'}
        run('python' if system == 'Windows' else 'python3', 'tools/browser/prepare_models.py',
            '--bio-source', selected['bio-full'], '--max-source', selected['max-full'],
            '--mode', 'copy', '--output-dir', bundle / 'data/browser-models')
        copy_tree(ROOT / 'data/compiled/malecns-v1', bundle / 'data/compiled/malecns-v1')
        copy_tree(ROOT / 'data/browser', bundle / 'data/browser')
        evidence = bundle / 'data/release-evidence'
        evidence.mkdir()
        for source in ['data/reports/bio-causal-controls-2026-09-18-final.json',
                       'data/reports/max-quick-causal-controls-2026-09-18.json',
                       'data/training-runs/max-quick-2026-09-18/training-summary.json',
                       'data/reports/ladder-16step-2026-09-18/summary.json',
                       'data/reports/ladder-16step-2026-09-18/RESULTS.md']:
            shutil.copyfile(ROOT / source, evidence / Path(source).name)
    run('wasm-pack', 'build', 'crates/stockfly-wasm', '--target', 'web', '--release', '--out-dir', app / 'src/wasm-gen')
    run('npx.cmd' if system == 'Windows' else 'npx', '--no-install', 'vite', 'build', app, '--outDir', bundle / 'apps/web/dist')
    run('cargo', 'build', '--locked', '--release', '-p', 'stockfly-server')
    shutil.copy2(ROOT / 'target/release' / exe, bundle / exe)
    for source in ['LICENSE', 'THIRD_PARTY_NOTICES.md', 'README.md']:
        shutil.copy2(ROOT / source, bundle / source)
    (bundle / 'EXPERIMENTAL.txt').write_text('Bio Full and Max Full failed causal acceptance. Full describes complete graph coverage, not validated strength. SHA-256 integrity is not a publisher signature. See README and data/release-evidence.\n')
    if system == 'Windows':
        (bundle / 'Start StockFly.cmd').write_text('@echo off\r\ncd /d "%~dp0"\r\nstockfly-server.exe --open\r\npause\r\n')
        (bundle / 'Download models.cmd').write_text('@echo off\r\ncd /d "%~dp0"\r\nstockfly-server.exe fetch-models %*\r\npause\r\n')
    else:
        for launcher_name, arguments in [('Start StockFly.command', '--open'), ('Download models.command', 'fetch-models "$@"')]:
            launcher = bundle / launcher_name
            launcher.write_text('#!/bin/sh\ncd "$(dirname "$0")" || exit 1\nexec ./stockfly-server ' + arguments + '\n')
            launcher.chmod(0o755)
    # Include the current source tree (including local release changes), excluding
    # ignored caches/raw assets. Corresponding Stockfish source lives by its binaries.
    sources = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=ROOT).decode().split('\0')
    with tarfile.open(bundle / 'stockfly-source.tar.gz', 'w:gz') as archive:
        for source in sorted(set(sources)):
            path = ROOT / source
            if source and path.is_file() and not path.is_symlink() and not source.startswith(('data/', '.superpowers/')):
                archive.add(path, arcname=f'stockfly-source/{source}', recursive=False)
    links = [path for path in bundle.rglob('*') if path.is_symlink()]
    if links:
        raise ValueError(f'Portable bundle contains symlinks: {links}')
    archive_path = shutil.make_archive(str(stage / name), 'zip', stage, 'stockfly')
    print(f'PORTABLE_ROOT={bundle}\nARCHIVE={archive_path}', flush=True)


if __name__ == '__main__':
    main()
