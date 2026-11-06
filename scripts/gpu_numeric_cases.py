"""Bounded case records shared by emitted-code numeric witnesses."""
import json

from gpu_certification_io import read_bounded
from gpu_launch_contract import require


def read_numeric_cases(path, schema):
    records = [json.loads(line) for line in read_bounded(path, 33554432).decode().splitlines()
               if line.startswith('{"schema":"' + schema + '"')]
    require(len(records) == 140, 'shared packed numeric corpus coverage differs')
    for record in records:
        require(set(record) == {'schema', 'allocations', 'initializers', 'body', 'value_id', 'lane_id', 'scale_id', 'inputs', 'expected'},
                'unknown numeric case field')
        for key, count in (('inputs', 4), ('expected', 3)):
            require(isinstance(record[key], list) and len(record[key]) == count and
                    all(type(value) is int and 0 <= value <= 0xffffffff for value in record[key]), 'invalid numeric words')
        require(record['expected'][1] in (0, 1) and record['expected'][2] in (0, 1), 'invalid expected validity words')
        require(all(type(record[key]) is int and 1 <= record[key] < 1048576 for key in ('value_id', 'lane_id', 'scale_id')),
                'invalid numeric value identity')
        require(all(isinstance(record[key], str) for key in ('allocations', 'initializers', 'body')), 'invalid numeric fragments')
        require(sum(len(record[key]) for key in ('allocations', 'initializers', 'body')) <= 1048576, 'numeric fragment ceiling')
    return records
