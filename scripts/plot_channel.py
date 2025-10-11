import numpy as np, pandas as pd, matplotlib.pyplot as plt

# df: Timestamp (nanoseconds), Value (rpm)
df = pd.read_csv("datasets/telemetry/DY8-nEngine.csv")

# Convert to float, subtract min (start at 0)
ts = pd.to_numeric(df["Timestamp"], errors="coerce").astype("float64")
t = (ts - ts.min()) / 1e9  # <-- convert ns → seconds ✅
y = df["nEngine"].to_numpy()

data = pd.DataFrame({"t": t, "y": y}).sort_values("t")

# Bin and aggregate
nbins = 5000
bins = pd.cut(data["t"], bins=nbins)
agg = data.groupby(bins)["y"].agg(["min", "mean", "max"])
centers = data.groupby(bins)["t"].mean().to_numpy()

# Plot
fig = plt.figure(figsize=(10.5, 4))
ax = fig.add_subplot(111)
ax.plot(centers, agg["min"], linewidth=0.6)
ax.plot(centers, agg["mean"], linewidth=2.4)
ax.plot(centers, agg["max"], linewidth=0.6)
ax.set_title("nEngine envelope vs Time")
ax.set_xlabel("Timestamp (s)")  # seconds ✅
ax.set_ylabel("nEngine (rpm)")
ax.grid(True, linewidth=0.3, alpha=0.6)
fig.tight_layout()
plt.show()