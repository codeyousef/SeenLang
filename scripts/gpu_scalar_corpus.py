"""Backend-independent scalar and splat witnesses over a small harness API."""
from gpu_launch_contract import require

SENTINEL = 0x6d6d6d6d


def portable_scalar_cases(harness):
    require(set(harness.entries) == {'bounded', 'budgeted', 'short_circuit', 'casts', 'vectors'}, 'unexpected scalar fixture')
    inputs = [-7, -2147483648, -1, 1, 31, 32, 0, 7, 2147483647]
    harness.array('integers', inputs); harness.array('bounded_output', [SENTINEL] * 16)
    harness.geometry(); harness.call('bounded', [('integers', 9), ('bounded_output', 13)])
    harness.check_array('bounded_output', [7, 0, 0, 0, 0, -1, -2147483648, 19, 7, 18, 0, 0, 9] + [SENTINEL] * 3); harness.fault(3)
    harness.check_array('integers', inputs)
    harness.array('budget_output', [SENTINEL]); harness.geometry(); harness.call('budgeted', [('budget_output', 1)])
    harness.check_array('budget_output', [1000000]); harness.fault(4)
    harness.array('short_output', [SENTINEL] * 2); harness.geometry(); harness.call('short_circuit', [('integers', 1), ('short_output', 2)])
    harness.check_array('short_output', [0, 1]); harness.fault(0)
    floating = [0x80000000, 0x3fc00000, 0xbfc00000, 0x4f000000, 0xcf000000, 0x4f800000, 0x7fc12345, 0x80000001, 1, 0x7f800000]
    harness.array('floating', floating); harness.array('signed', [SENTINEL] * 10); harness.array('unsigned', [SENTINEL] * 10)
    harness.array('rounded', [SENTINEL] * 14); harness.geometry()
    harness.call('casts', [('floating', 10), ('signed', 10), ('unsigned', 10), ('rounded', 14)])
    harness.check_array('signed', [0, 1, -1, 0, -2147483648, 0, 0, 0, 0, 0])
    harness.check_array('unsigned', [0, 1, 0, 2147483648, 0, 0, 0, 0, 0, 0])
    harness.check_array('rounded', [0x4f000000, 0x4f800000, 0xcf000000, 0x80000000, 0x80000000, 0, 0x7fc12345, 0x3f800000,
                                    0x40000000, 0x3f800000, 0, 0, 0, 2]); harness.fault(10)
    harness.array('vector_input', [0x3f800000, 0xc0000000, 0, 0xdeadbeef]); harness.array('vector_output', [SENTINEL] * 12)
    harness.geometry(); harness.call('vectors', [('vector_input', 1), ('vector_output', 3), [0x40000000, 0x40800000, 0x80000000, 0]])
    harness.check_array('vector_output', [0x40400000, 0x40000000, 0, SENTINEL, 0x3f800000, 0xc0000000, 0, SENTINEL,
                                         0x40000000, 0x40800000, 0, SENTINEL]); harness.fault(0)


def splat_cases(harness):
    require(set(harness.entries) == {'constantSplat', 'variableSplat'}, 'unexpected splat fixture')
    harness.array('constant_output', [SENTINEL] * 4); harness.geometry()
    harness.call('constantSplat', [('constant_output', 1)])
    harness.check_array('constant_output', [0x40000000] * 3 + [SENTINEL]); harness.fault(0)
    # Compare raw lane bits, including signed zero, subnormal and NaN payloads.
    for index, value in enumerate((0xc0200000, 0x80000000, 1, 0x7fc12345)):
        name = 'splat_input_' + str(index); output = 'splat_output_' + str(index)
        harness.array(name, [value]); harness.array(output, [SENTINEL] * 4); harness.geometry()
        harness.call('variableSplat', [(name, 1), (output, 1)])
        harness.check_array(output, [value] * 3 + [SENTINEL]); harness.fault(0)
        harness.check_array(name, [value])
    harness.array('empty_input', [SENTINEL]); harness.array('empty_output', [SENTINEL] * 4); harness.geometry()
    harness.call('variableSplat', [('empty_input', 0), ('empty_output', 1)])
    harness.check_array('empty_output', [0] * 3 + [SENTINEL]); harness.fault(1)



def geometry_cases(harness):
    require(set(harness.entries) == {'coordinates', 'seen_shared_0_4', 'empty_entry'}, 'unexpected geometry fixture')
    harness.array('coordinates', [SENTINEL] * 16)
    harness.geometry((3, 2, 2), (2, 3, 4), (1, 1, 1), (1, 2, 3)); harness.call('coordinates', [('coordinates', 16)])
    harness.check_array('coordinates', [4, 5, 7, 1, 1, 1, 1, 2, 3, 2, 3, 4, 3, 2, 2, 10]); harness.fault(0)
    harness.reset_array('coordinates'); harness.geometry((3, 2, 2), (1431655766, 1, 1)); harness.call('coordinates', [('coordinates', 16)])
    harness.check_array('coordinates', [SENTINEL] * 16); harness.fault(16)
    harness.array('shared_input', [19, 23]); harness.array('shared_output', [SENTINEL]); harness.geometry()
    harness.call('seen_shared_0_4', [('shared_input', 2), ('shared_output', 1)]); harness.check_array('shared_output', [42]); harness.fault(0)
    harness.geometry(); harness.call('empty_entry', []); harness.fault(0)
