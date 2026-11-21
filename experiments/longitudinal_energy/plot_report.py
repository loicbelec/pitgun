#!/usr/bin/env python3
"""Render the archived native sensitivity; never launch or fit a simulation."""
import hashlib
import json
import pathlib
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

HERE = pathlib.Path(__file__).resolve().parent
path = HERE/'results/local-energy-v1.json'
report = json.loads(path.read_bytes())
identity = 'sha256:'+hashlib.sha256(path.read_bytes()).hexdigest()
if identity != path.with_suffix('.sha256').read_text().strip():
    raise ValueError('report checksum mismatch')
rows = [s for s in report['sensitivities'] if s['group'] == 'cooling']
plt.rcParams.update({'font.size':10, 'axes.spines.right':False, 'axes.spines.top':False,
                     'svg.hashsalt':identity})
fig, axes = plt.subplots(1, 2, figsize=(10, 4), layout='constrained')
for ambient, color in [('35', '#225c69'), ('55', '#b75539')]:
    selected = sorted((s for s in rows if s['options'][2] == ambient), key=lambda s:float(s['options'][1]))
    scales = [float(s['options'][1]) for s in selected]
    outputs = [s['outcome']['competitors']['player']['output'] for s in selected]
    axes[0].plot(scales, [o['final_state']['engine_temp'] for o in outputs], 'o-', color=color, label=f'Engine air {ambient} °C')
    axes[1].plot(scales, [o['total_time_s'] for o in outputs], 'o-', color=color)
for ax in axes:
    ax.set_xlabel('Conductance multiplier H/Href (fixed vehicle)')
    ax.set_xticks([.25,.5,1,2])
    ax.grid(alpha=.2)
axes[0].set_ylabel('Final aggregate engine temperature (°C)')
axes[0].legend(frameon=False)
axes[1].set_ylabel('Five-lap simulated duration (s)')
fig.suptitle('Pitgun native energy V1 · Monza modern · smooth driver')
fig.supxlabel('Synthetic sensitivity only · unchanged heat/derating coefficients · not an HQ balance or full weather model', fontsize=8)
out = HERE/'figures'
out.mkdir(exist_ok=True)
fig.savefig(out/'cooling-v1.png', dpi=170, metadata={'Description':identity})
fig.savefig(out/'cooling-v1.svg', metadata={'Description':identity, 'Date':None})
plt.close(fig)
print(identity)
