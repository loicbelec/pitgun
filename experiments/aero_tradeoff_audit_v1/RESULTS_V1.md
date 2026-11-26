# Aero tradeoff audit V1

Numerical PASS: **True**. 42 main GP +6 fresh repeats; 2796 accepted laps; 6 exact repeats and nine reference input/result bridges.

Developed Craft era-II V8_1970/basic aero; 16 discretionary points plus eight installed bonuses; gear .5, balanced reference driver, seed73013, SM midpoint stop, fuel100kg. Same thermal policy and solver throughout. Counterfactual coefficients are offline diagnostics only.

## Full coupled slider response

| Circuit | Slider | Drag/downforce area m² | Total s | Delta .5 s | First-five delta .5 s | Max km/h | Top-gear redline time s |
|---|---:|---:|---:|---:|---:|---:|---:|
| MONZA | 0.0 | 0.7708/3.0720 | 6436.866 | -74.674 | +0.886 | 252.80 | 0.0 |
| MONZA | 0.2 | 0.8103/3.4816 | 6452.138 | -59.403 | -2.104 | 249.19 | 0.0 |
| MONZA | 0.5 | 1.0178/4.0960 | 6511.540 | +0.000 | +0.000 | 233.55 | 0.0 |
| MONZA | 0.8 | 1.4031/4.7104 | 6607.981 | +96.441 | +8.756 | 226.55 | 0.0 |
| MONZA | 1.0 | 1.7588/5.1200 | 6721.046 | +209.506 | +22.374 | 214.55 | 0.0 |
| SILVERSTONE | 0.0 | 0.7708/3.0720 | 6754.217 | -47.151 | +2.726 | 248.42 | 0.0 |
| SILVERSTONE | 0.2 | 0.8103/3.4816 | 6760.295 | -41.073 | -0.155 | 245.54 | 0.0 |
| SILVERSTONE | 0.5 | 1.0178/4.0960 | 6801.368 | +0.000 | +0.000 | 232.44 | 0.0 |
| SILVERSTONE | 0.8 | 1.4031/4.7104 | 6880.942 | +79.574 | +3.193 | 226.55 | 0.0 |
| SILVERSTONE | 1.0 | 1.7588/5.1200 | 6937.501 | +136.133 | +13.225 | 211.54 | 0.0 |
| SAOPAULO | 0.0 | 0.7708/3.0720 | 6951.475 | -50.070 | +2.136 | 258.36 | 0.0 |
| SAOPAULO | 0.2 | 0.8103/3.4816 | 6959.283 | -42.261 | -1.592 | 255.15 | 0.0 |
| SAOPAULO | 0.5 | 1.0178/4.0960 | 7001.545 | +0.000 | +0.000 | 235.94 | 0.0 |
| SAOPAULO | 0.8 | 1.4031/4.7104 | 7072.972 | +71.427 | +5.999 | 226.55 | 0.0 |
| SAOPAULO | 1.0 | 1.7588/5.1200 | 7146.892 | +145.347 | +14.529 | 217.53 | 0.0 |

## Controlled coefficient responses and geometry partitions

Deltas compare against coupled .5. Geometry columns use identical supplied curvature bins, so their driving-time deltas sum to the total-time delta (service is unchanged). Drag/downforce controls retain live thermal/wear/fuel response: their seconds are not additive independent effects.

| Circuit | Mode | Slider | Total delta s | Low/medium/high curvature delta s | Drag work delta MJ | Braking time delta s | Brake-cap time s |
|---|---|---:|---:|---:|---:|---:|---:|
| MONZA | coupled | 0.0 | -74.674 | -60.605/-28.205/+14.136 | -106.995 | +133.879 | 53.8 |
| MONZA | coupled | 0.2 | -59.403 | -50.014/-23.471/+14.082 | -89.284 | +107.959 | 59.6 |
| MONZA | coupled | 0.5 | +0.000 | +0.000/+0.000/+0.000 | +0.000 | +0.000 | 48.3 |
| MONZA | coupled | 0.8 | +96.441 | +74.616/+37.351/-15.526 | +157.879 | -147.564 | 39.4 |
| MONZA | coupled | 1.0 | +209.506 | +181.459/+76.539/-48.492 | +273.941 | -256.385 | 35.9 |
| MONZA | drag_only | 0.0 | -35.729 | -57.050/-23.659/+44.980 | -108.329 | +149.072 | 72.3 |
| MONZA | drag_only | 0.2 | -31.604 | -47.339/-20.084/+35.819 | -90.441 | +121.158 | 67.6 |
| MONZA | drag_only | 0.8 | +83.395 | +74.725/+35.832/-27.162 | +157.452 | -156.611 | 34.4 |
| MONZA | drag_only | 1.0 | +209.546 | +182.526/+74.471/-47.451 | +271.220 | -245.993 | 27.6 |
| MONZA | downforce_only | 0.0 | -6.102 | +0.228/-1.289/-5.041 | -1.092 | +8.065 | 29.0 |
| MONZA | downforce_only | 0.2 | -11.053 | -0.186/-1.503/-9.364 | -0.184 | -4.395 | 38.8 |
| MONZA | downforce_only | 0.8 | +35.415 | +2.666/+4.408/+28.341 | -1.747 | +22.727 | 52.5 |
| MONZA | downforce_only | 1.0 | +75.612 | +10.654/+8.965/+55.993 | -5.236 | +43.280 | 52.4 |
| MONZA | no_downforce | 0.5 | +294.850 | +18.611/+39.697/+236.542 | -28.109 | +463.921 | 0.0 |
| SILVERSTONE | coupled | 0.0 | -47.151 | -48.306/-19.713/+20.868 | -100.662 | +169.939 | 30.0 |
| SILVERSTONE | coupled | 0.2 | -41.073 | -38.619/-17.195/+14.741 | -83.960 | +127.403 | 39.6 |
| SILVERSTONE | coupled | 0.5 | +0.000 | +0.000/+0.000/+0.000 | +0.000 | +0.000 | 37.2 |
| SILVERSTONE | coupled | 0.8 | +79.574 | +62.704/+35.240/-18.370 | +145.143 | -171.055 | 26.8 |
| SILVERSTONE | coupled | 1.0 | +136.133 | +125.756/+60.188/-49.812 | +267.024 | -310.229 | 11.4 |
| SILVERSTONE | drag_only | 0.0 | -30.721 | -43.953/-20.133/+33.365 | -100.588 | +140.161 | 56.1 |
| SILVERSTONE | drag_only | 0.2 | -27.113 | -37.132/-17.173/+27.192 | -83.899 | +117.165 | 52.3 |
| SILVERSTONE | drag_only | 0.8 | +74.034 | +62.274/+32.690/-20.930 | +144.525 | -150.281 | 20.0 |
| SILVERSTONE | drag_only | 1.0 | +147.912 | +127.649/+63.232/-42.969 | +261.618 | -278.181 | 3.5 |
| SILVERSTONE | downforce_only | 0.0 | +5.233 | -4.040/+1.135/+8.139 | -1.171 | +50.012 | 15.6 |
| SILVERSTONE | downforce_only | 0.2 | -0.411 | -0.371/+0.946/-0.987 | -1.030 | +18.224 | 25.2 |
| SILVERSTONE | downforce_only | 0.8 | +23.889 | +1.536/+4.139/+18.215 | -0.962 | -7.207 | 44.4 |
| SILVERSTONE | downforce_only | 1.0 | +50.775 | +3.566/+5.559/+41.650 | -2.343 | +1.203 | 46.2 |
| SILVERSTONE | no_downforce | 0.5 | +423.103 | +29.149/+69.514/+324.440 | -44.339 | +586.468 | 0.0 |
| SAOPAULO | coupled | 0.0 | -50.070 | -45.473/-11.249/+6.653 | -101.617 | +160.473 | 49.6 |
| SAOPAULO | coupled | 0.2 | -42.261 | -40.177/-10.756/+8.672 | -83.594 | +124.313 | 58.5 |
| SAOPAULO | coupled | 0.5 | +0.000 | +0.000/+0.000/+0.000 | +0.000 | +0.000 | 55.4 |
| SAOPAULO | coupled | 0.8 | +71.427 | +69.308/+22.506/-20.386 | +144.023 | -179.090 | 46.6 |
| SAOPAULO | coupled | 1.0 | +145.347 | +145.754/+40.174/-40.581 | +258.859 | -316.107 | 44.7 |
| SAOPAULO | drag_only | 0.0 | -12.740 | -44.928/-10.280/+42.468 | -101.078 | +152.005 | 75.9 |
| SAOPAULO | drag_only | 0.2 | -12.511 | -38.018/-8.981/+34.488 | -84.250 | +126.115 | 72.1 |
| SAOPAULO | drag_only | 0.8 | +53.634 | +66.610/+21.396/-34.372 | +145.080 | -177.601 | 36.3 |
| SAOPAULO | drag_only | 1.0 | +134.044 | +139.478/+39.555/-44.989 | +260.248 | -298.418 | 29.1 |
| SAOPAULO | downforce_only | 0.0 | -12.031 | -0.112/+0.818/-12.737 | -1.355 | +25.999 | 28.7 |
| SAOPAULO | downforce_only | 0.2 | -15.851 | -1.605/-0.625/-13.621 | +0.193 | +7.331 | 41.5 |
| SAOPAULO | downforce_only | 0.8 | +38.756 | +4.402/+3.416/+30.938 | -2.341 | +10.380 | 64.8 |
| SAOPAULO | downforce_only | 1.0 | +76.691 | +8.489/+6.784/+61.418 | -4.790 | +28.658 | 68.1 |
| SAOPAULO | no_downforce | 0.5 | +448.120 | +87.299/+63.308/+297.513 | -59.911 | +598.220 | 0.0 |

## Limits and decision

All six repeats and nine old results must match at the full-result/input level, not merely total time. Bin work is checked against accepted native energy ledgers. Braking-phase duration changes are not fixed spatial sectors; do not attribute their difference as isolated braking seconds. Fixed gear, driver, car/means and one tire strategy condition the response. No real aero identification or global optimum is claimed.

See `INTERPRETATION.md` for the source polar convention, causal findings and proposed next step. No coefficient, game, runtime, catalog or deployment change.
