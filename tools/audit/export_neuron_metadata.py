#!/usr/bin/env python3
"""Export graph-aligned transmitter and region labels for causal controls.

This reads the already-required MaleCNS PyArrow tables and writes a derived
audit artifact. It never modifies the raw or compiled graph directories.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

import pyarrow.compute as pc
import pyarrow.feather as feather


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--raw', type=Path, default=Path('data/raw/malecns-v1'))
    parser.add_argument('--graph', type=Path, default=Path('data/compiled/malecns-v1'))
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()

    manifest = json.loads((args.graph / 'manifest.json').read_text())
    neuron_bytes = (args.graph / 'neurons.bin').read_bytes()
    if hashlib.sha256(neuron_bytes).hexdigest() != manifest['neurons_sha256']:
        raise ValueError('compiled neurons.bin hash does not match manifest')
    body_ids = struct.unpack(f'<{len(neuron_bytes) // 8}Q', neuron_bytes)

    nt_table = feather.read_table(
        args.raw / 'body-neurotransmitters-male-cns-v1.0.feather',
        columns=['body', 'consensus_nt'],
    )
    transmitter = dict(zip(
        nt_table.column('body').to_pylist(),
        nt_table.column('consensus_nt').to_pylist(),
    ))
    annotations = feather.read_table(
        args.raw / 'body-annotations-male-cns-v1.0-minconf-0.5.feather',
        columns=['bodyId', 'status', 'superclass'],
    )
    annotations = annotations.filter(pc.equal(annotations.column('status'), 'Traced'))
    region = dict(zip(
        annotations.column('bodyId').to_pylist(),
        annotations.column('superclass').to_pylist(),
    ))
    if set(body_ids) != set(region):
        raise ValueError('traced annotation IDs do not exactly match compiled neurons')

    document = {
        'format_version': 1,
        'graph_neurons_sha256': manifest['neurons_sha256'],
        'source': (
            'MaleCNS v1.0 consensus_nt and traced body annotation superclass; '
            'aligned to compiled neurons.bin'
        ),
        'neurons': [
            {
                'body_id': body_id,
                'transmitter': transmitter.get(body_id) or 'unresolved',
                'region': region.get(body_id) or 'unannotated',
            }
            for body_id in body_ids
        ],
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open('x') as target:
        json.dump(document, target, separators=(',', ':'))
        target.write('\n')
    print(json.dumps({
        'out': str(args.out),
        'neurons': len(body_ids),
        'transmitters': len({row['transmitter'] for row in document['neurons']}),
        'regions': len({row['region'] for row in document['neurons']}),
    }, indent=2))


if __name__ == '__main__':
    main()
