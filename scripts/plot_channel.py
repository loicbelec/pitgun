import numpy as np, pandas as pd, matplotlib.pyplot as plt

# df: Timestamp (ns/us/ms/s), Value (rpm)
df = pd.read_csv("datasets/telemetry/DY8-nEngine.csv")
ts = pd.to_numeric(df["Timestamp"], errors="coerce").astype("float64")

t = ts - ts.min()  # démarre à zéro
y = df["nEngine"].to_numpy()

data = pd.DataFrame({"t": t, "y": y}).sort_values("t")

# Taille cible à l’écran (ex. 4000 points)
nbins = 10000
bins = pd.cut(data["t"], bins=nbins)
agg = data.groupby(bins)["y"].agg(["min", "max"])
centers = data.groupby(bins)["t"].mean().to_numpy()

fig = plt.figure(figsize=(10.5, 4))
ax = fig.add_subplot(111)
ax.plot(centers, agg["min"], linewidth=0.6)
ax.plot(centers, agg["max"], linewidth=0.6)
ax.set_title("nEngine envelope (min–max) vs Time")
ax.set_xlabel("Time (s)")
ax.set_ylabel("nEngine (rpm)")
ax.grid(True, linewidth=0.3, alpha=0.6)
fig.tight_layout(); plt.show()