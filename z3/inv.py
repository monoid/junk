#!/usr/bin/env python3

"""
Transforming integer division by multiplication and shift.
"""

from z3 import *
from multiprocessing import Pool, cpu_count

def find_constants(div, bits, range=None):
    s = Solver()

    x = BitVec('x', bits)
    a = BitVec('a', bits)
    shift = BitVec('s', bits)

    div = BitVecVal(div, bits)

    equality = UDiv(x, div) == Extract(bits - 1, 0, LShR(ZeroExt(bits, a) * ZeroExt(bits, x), (ZeroExt(bits, shift))))

    s.add(shift >= 0)
    s.add(shift < 2*bits)

    if range is not None:
        (min_a, max_a) = range
        s.add(ULE(min_a, a))
        s.add(ULT(a , max_a))

    s.add(ForAll([x], equality))

    result = s.check()
    if result == sat:
        m = s.model()
        print(m)
        return {
            'multiplier': m[a].as_long(),
            'shift': m[shift].as_long()
        }
    else:
        return None


def find_constants_2(args):
    (div, bits, range) = args
    return find_constants(div,  bits, range=range)


def check_solution(sol, divisor, bits):
    s = Solver()

    x = BitVec('x', bits)
    div = BitVecVal(divisor, bits)
    a = BitVecVal(solution['multiplier'], bits)
    shift = BitVecVal(solution['shift'], bits)

    s.add(
        UDiv(x, div) !=
        Extract(bits - 1, 0, LShR(ZeroExt(bits, a) * ZeroExt(bits, x), (ZeroExt(bits, shift))))
    )

    result = s.check()

    if result == unsat:
        print("correct:", sol)
        return True  # Решение верно для всех значений
    elif result == sat:
        m = s.model()
        d = (m[x].as_long() * sol['multiplier'] & ((1<<(bits)) - 1)) >> sol['shift']
        e = m[x].as_long() // divisor
        print(f"Counter-example found: x = {m[x]}: {e} != {d}")
    else:
        print("Verification timeout")
    return False


def find_parallel(div, bits, cpus=None):

    if cpus is None:
        cpus = cpu_count()
    part_num = 1024 * cpus
    max_value = (2 ** bits)
    part_size = max_value // part_num
    ranges = []
    for i in range(part_num):
        start = i * part_size
        end = (i + 1) * part_size if i < part_num - 1 else max_value
        ranges.append((div, bits, (start, end)))

    with Pool(cpus) as pool:
        results = pool.map(find_constants_2, ranges)
    return list(filter(lambda x: x, results))


# A slightly modified example from the "SMT by example" by Dennis Yurichev
def simple(div, bits):
    m=BitVec('m', bits)
    s=Solver ()
    # wouldn 't work for 10, etc
    divisor = div
    # random constant , must be divisible by divisor :
    constt =(0x1234567 * divisor )
    s.add(constt * m == constt / divisor)
    c = s.check()
    if c == sat:
        model = s.model()
        mult = model[m].as_long()
        print("{:x}".format(mult))
        return {
            'multiplier': mult,
            'shift': 0,
        }
    else:
        print(c)


if __name__ == '__main__':
    value = 9
    bits = 32
    # solutions = find_parallel(value, bits)
    # print(solutions)

    solution = find_constants(value, bits)
    # solution = simple(value, bits)
    if solution:
        check_solution(solution, value, bits)
