#!/usr/bin/env python3
"""Export real, graph-aligned browser annotations without touching source data.

Usage: .venv/bin/python tools/browser/export_metadata.py
Body IDs come from neurons.bin in the browser and are decoded as BigInt strings.
Missing somaNeuromere means unannotated, never an inferred anatomical region.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

import pyarrow.feather as feather


def export(raw: Path, graph: Path, out: Path):
    manifest = json.loads((graph / 'manifest.json').read_text())
    neuron_bytes = (graph / 'neurons.bin').read_bytes()
    if hashlib.sha256(neuron_bytes).hexdigest() != manifest['neurons_sha256']:
        raise ValueError('neurons.bin does not match the graph manifest')
    ids = struct.unpack(f'<{len(neuron_bytes) // 8}Q', neuron_bytes)
    fields = ['type', 'class', 'superclass', 'somaNeuromere']
    table = feather.read_table(raw / 'body-annotations-male-cns-v1.0-minconf-0.5.feather', columns=['bodyId', *fields])
    rows = {row['bodyId']: [row[field] or '' for field in fields] for row in table.to_pylist()}
    if not all(body in rows for body in ids):
        raise ValueError('compiled neuron missing from source annotations')
    doc = {'formatVersion': 1, 'graphNeuronsSha256': manifest['neurons_sha256'],
           'fields': fields, 'neurons': [rows[body] for body in ids]}
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(doc, separators=(',', ':')) + '\n')
    print(f'Exported {len(ids)} neuron annotations to {out}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--raw', type=Path, default=Path('data/raw/malecns-v1'))
    parser.add_argument('--graph', type=Path, default=Path('data/compiled/malecns-v1'))
    parser.add_argument('--out', type=Path, default=Path('data/browser/metadata.json'))
    args = parser.parse_args()
    export(args.raw, args.graph, args.out)
