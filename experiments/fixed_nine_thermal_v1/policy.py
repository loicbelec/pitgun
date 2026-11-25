"""Pure pre-weekend field construction: no player setup or result input."""
from fractions import Fraction


def project(target, budget, floors):
    if len(target) != 4 or len(floors) != 4 or not sum(floors) <= budget <= 80 or any(not 0 <= x <= 20 for x in floors):
        raise ValueError('invalid available effective budget/floors')
    values=list(floors)
    for _ in range(budget-sum(values)):
        axis=max((i for i in range(4) if values[i]<20),key=lambda i:(target[i]-values[i],-i))
        values[axis]+=1
    return values


def scale(values,budget,floors):
    return project([Fraction(v*budget,sum(values)) for v in values],budget,floors)


def allocate(anchors,budget,floors):
    keys=sorted(int(k) for k in anchors)
    if not keys[0]<=budget<=keys[-1]:
        raise ValueError('outside qualified budget range: no silent extrapolation')
    if str(budget) in anchors:return list(anchors[str(budget)])
    lo=max(k for k in keys if k<budget);hi=min(k for k in keys if k>budget)
    target=[Fraction(anchors[str(lo)][i]*(hi-budget)+anchors[str(hi)][i]*(budget-lo),hi-lo) for i in range(4)]
    return project(target,budget,floors)


def preparation(policy,era,circuit,budget,actor):
    ctx=next(c for c in policy['contexts'] if c['era']==era and c['circuit']==circuit)
    p=ctx['actors'][actor]
    return {'actor':actor,'effective_budget':budget,'effective_allocation':allocate(p['anchors'],budget,ctx['floors']),
            'downforce':p['downforce'],'gear':p['gear'],'strategy':p['strategy'],'mode':'balanced'}
