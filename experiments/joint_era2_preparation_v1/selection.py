"""Offline preparation selection; never accepts a player's input or result."""
from copy import deepcopy
from fractions import Fraction


def choose(options, style, base):
    """Keep a fixed style within 0.3% of this actor's sampled best time.

    All nine use the same rule, including the two strongest trait profiles.
    The midpoint preference breaks style ties before lap-time optimization.
    """
    if not options or any(row['checks'] for _, row in options):
        raise ValueError('selection requires a complete checked actor pool')
    best = min(row['data']['outcome']['time_s'] for _, row in options)
    eligible = [(pid, row) for pid, row in options
                if row['data']['outcome']['time_s'] <= best * 1.003]
    return min(eligible, key=lambda item: (
        base.style_key(item[1]['case'], style),
        abs(item[1]['case']['downforce'] - .5),
        abs(item[1]['case']['gear'] - .5),
        item[1]['data']['outcome']['time_s'], item[0]))


def preparation(policy, circuit, budget, actor, scale):
    """Nearest complete anchor (tie: lower), exact integer points, no extrapolation.

    Gear, aero and stint plan travel together. This is an experimental policy;
    non-anchor budgets still require reserved native validation before admission.
    """
    ctx = next(c for c in policy['contexts'] if c['circuit'] == circuit)
    anchors = ctx['actors'][actor]['anchors']
    keys = sorted(int(k) for k in anchors)
    if isinstance(budget, bool) or not isinstance(budget, int) or not keys[0] <= budget <= keys[-1]:
        raise ValueError('outside experimental integer budget range')
    anchor = min(keys, key=lambda k: (abs(k - budget), k))
    result = deepcopy(anchors[str(anchor)]['preparation'])
    result['effective_allocation'] = scale(result['effective_allocation'], budget, ctx['floors'])
    result['effective_budget'] = budget
    result['source_anchor_budget'] = anchor
    return result


def heavy_80(case):
    floors = [2, 1, 4, 1]
    points = case['effective_allocation']
    return Fraction(points[1] - floors[1] + points[2] - floors[2],
                    case['effective_budget'] - sum(floors)) >= Fraction(4, 5)
