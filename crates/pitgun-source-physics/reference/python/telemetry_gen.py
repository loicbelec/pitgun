#!/usr/bin/env python3
"""
Pitgun Physics Engine (Runtime) - STINT MODE
Gère la simulation multi-tours avec continuité thermique et derating.
"""

from __future__ import annotations
from dataclasses import dataclass
import math
import numpy as np
import pandas as pd
import logging
import sys
import copy

# --- IMPORTS MODULAIRES ---
# On charge la classe F1 spécifique, pas juste le véhicule générique
from .vehicle.f1_2026 import F1_2026
from .vehicle.vehicle import Vehicle

logger = logging.getLogger(__name__)
logger.addHandler(logging.NullHandler())
logger.setLevel(logging.INFO)

# ----------------------------
# Data Models
# ----------------------------

@dataclass
class PlayerTuning:
    aero_points: int
    chassis_points: int
    cooling_points: int
    engine_points: int
    downforce_slider: float
    gear_ratio_slider: float

@dataclass
class TrackProfile:
    s: np.ndarray        
    x: np.ndarray
    y: np.ndarray
    z: np.ndarray
    kappa: np.ndarray
    slope: np.ndarray
    heading: np.ndarray

# ----------------------------
# 1. Physics Solver (Vitesse & Trajectoire)
# ----------------------------

def compute_speed_profile(track: TrackProfile, vehicle: Vehicle):
    """
    Calcule le profil de vitesse idéal sur un tour (Forward-Backward Solver).
    Prend en compte la puissance ACTUELLE du véhicule (qui peut être dégradée).
    """
    s = track.s
    ds = s[1] - s[0] if len(s) > 1 else 1.0
    n = len(s)
    
    # A. Vitesse Max en Virage (Grip Latéral)
    v_corner = np.full(n, 400.0) # Init large
    
    for i in range(n):
        k_val = abs(track.kappa[i])
        if k_val < 1e-5: continue

        # Itération pour trouver l'équilibre Aéro / Grip
        v = 70.0
        for _ in range(5):
            q = 0.5 * vehicle.rho * v * v
            downforce = q * vehicle.clA_Z # On assume mode Z en virage
            a_lat_max = vehicle.mu * (vehicle.g + downforce / vehicle.m)
            v = math.sqrt(max(0.1, a_lat_max / k_val))
        v_corner[i] = min(v, 400.0)

    # B. Forward Pass (Accélération)
    v_fwd = np.zeros(n)
    v_fwd[0] = 30.0 / 3.6 # Départ lancé lent
    
    for i in range(n - 1):
        v = min(v_fwd[i], v_corner[i])
        
        # Mode Aéro (Z si virage ou freinage, X si ligne droite)
        # Seuil de courbure arbitraire pour l'activation du DRS/Low Drag
        mode_z = abs(track.kappa[i]) > 0.001
        
        q = 0.5 * vehicle.rho * v * v
        cdA = vehicle.cdA_Z if mode_z else vehicle.cdA_X
        clA = vehicle.clA_Z if mode_z else vehicle.clA_X
        
        # Forces résistantes
        F_drag = q * cdA
        F_roll = vehicle.c_rr * (vehicle.m * vehicle.g + q * clA)
        F_slope = vehicle.m * vehicle.g * track.slope[i]
        
        # Force Motrice (Dépend de la puissance dispo et du grip)
        # On utilise la méthode de la classe qui intègre la courbe de couple actuelle
        # Note: on passe une température bidon car le derating est géré globalement par le couple
        pwr_max, _rpm_max, _gear_choice = vehicle.max_engine_power(v, vehicle.t_init)
        pwr_max *= 1000.0 # kW -> W
        F_eng_max = pwr_max / max(10.0, v)
        
        normal_load = vehicle.m * vehicle.g + q * clA
        F_traction = vehicle.mu * normal_load
        
        F_drive = min(F_eng_max, F_traction)
        F_net = F_drive - F_drag - F_roll - F_slope
        
        a = F_net / vehicle.m
        v_fwd[i+1] = math.sqrt(max(0, v*v + 2.0 * a * ds))

    v_fwd = np.minimum(v_fwd, v_corner)

    # C. Backward Pass (Freinage)
    v_bwd = np.copy(v_fwd)
    
    for i in range(n - 2, -1, -1):
        v_target = v_bwd[i+1]
        
        q = 0.5 * vehicle.rho * v_target * v_target
        # Freinage toujours en configuration High Downforce (Z)
        F_drag = q * vehicle.cdA_Z
        F_roll = vehicle.c_rr * (vehicle.m * vehicle.g + q * vehicle.clA_Z)
        F_slope = vehicle.m * vehicle.g * track.slope[i]
        
        # Cercle de friction (Grip dispo pour freinage vs virage)
        normal_load = vehicle.m * vehicle.g + q * vehicle.clA_Z
        grip_total = vehicle.mu * normal_load
        
        F_lat_req = vehicle.m * (v_target**2) * abs(track.kappa[i])
        
        if F_lat_req >= grip_total:
            F_brake_max = 0.0
        else:
            F_brake_max = math.sqrt(grip_total**2 - F_lat_req**2)
            
        F_decel_avail = F_brake_max + F_drag + F_roll + F_slope
        a_decel = F_decel_avail / vehicle.m
        a_decel = min(a_decel, 6.0 * vehicle.g) # Limite humaine
        
        v_max = math.sqrt(v_target**2 + 2 * a_decel * ds)
        if v_bwd[i] > v_max:
            v_bwd[i] = v_max

    v_final = np.minimum(v_fwd, v_bwd)
    
    # Intégration temps
    dt = np.zeros(n)
    v_safe = np.maximum(v_final, 1.0)
    dt[1:] = ds / (0.5 * (v_safe[1:] + v_safe[:-1]))
    t = np.cumsum(dt)

    return {"s": s, "t": t, "v": v_final}

# ----------------------------
# 2. Resampler (Télémétrie Temporelle)
# ----------------------------

def resample_telemetry(track, sol, vehicle: Vehicle, hz=60.0):
    t_s, s_s, v_s = sol["t"], sol["s"], sol["v"]
    t_end = t_s[-1]
    
    # Vecteur temps régulier (60Hz)
    t = np.arange(0, t_end, 1/hz)
    
    # Interpolation des données spatiales vers temporelles
    s_t = np.interp(t, t_s, s_s)
    v_t = np.interp(t, t_s, v_s)
    
    # Interpolation Track Data (Optimisation possible ici)
    x_t = np.interp(s_t, track.s, track.x)
    y_t = np.interp(s_t, track.s, track.y)
    h_t = np.interp(s_t, track.s, track.heading)
    k_t = np.interp(s_t, track.s, track.kappa)
    
    # Dérivées
    a_long = np.gradient(v_t, t)
    
    # Simulation des Systèmes (Boîte, Thermique, Pédales)
    n_frames = len(t)
    rpm = np.zeros(n_frames)
    gear = np.zeros(n_frames, dtype=int)
    temp = np.zeros(n_frames)
    pwr = np.zeros(n_frames)
    thr = np.zeros(n_frames)
    brk = np.zeros(n_frames)
    
    # État initial thermique (Venant du véhicule)
    current_temp = vehicle.t_init
    current_gear = 1
    last_shift = -1.0
    
    for i in range(n_frames):
        v = v_t[i]
        al = a_long[i]
        
        # 1. Gearbox
        # Ratio actuel
        G = vehicle.gear_ratios[current_gear-1]
        r_curr = (v * 60 * G) / (2 * math.pi * vehicle.r_wheel)
        
        if (t[i] - last_shift) > 0.2: # Shift time
            if r_curr > vehicle.n_upshift and current_gear < vehicle.gear_count:
                current_gear += 1
                last_shift = t[i]
            elif r_curr < vehicle.n_downshift and current_gear > 1:
                current_gear -= 1
                last_shift = t[i]
                
        gear[i] = current_gear
        # Recalcul RPM final
        G = vehicle.gear_ratios[current_gear-1]
        r_final = (v * 60 * G) / (2 * math.pi * vehicle.r_wheel)
        rpm[i] = np.clip(r_final, vehicle.n_idle, vehicle.n_max)
        
        # 2. Pedals (Inference simple depuis l'accel)
        if al >= 0:
            brk[i] = 0
            thr[i] = np.clip(al / 5.0, 0, 1) # ~0.5G = 100% throttle approx
        else:
            thr[i] = 0
            brk[i] = np.clip(-al / 10.0, 0, 1) # ~1G = 100% brake
            
        # 3. Thermique & Puissance Réelle
        # Puissance théorique à ce RPM (déjà dégradée si vehicle.trq est modifié)
        p_avail = vehicle.power_kw_from_rpm(rpm[i]) * 1000.0 # kW -> W
        p_out = p_avail * thr[i]
        pwr[i] = p_out
        
        # Modèle thermique simple
        # Chauffe = proportionnelle à la puissance sortie
        # Refroidissement = proportionnel à la vitesse (flux d'air) + base
        heat_in = vehicle.alpha_heat * p_out
        heat_out = (vehicle.p_cool0 + vehicle.k_cool * v)
        
        dt_frame = 1.0/hz
        delta_T = (heat_in - heat_out) / vehicle.c_th * dt_frame
        current_temp += delta_T
        temp[i] = current_temp

    g_lat = (v_t**2 * k_t) / 9.81
    g_long = a_long / 9.81
    
    return {
        "time_s": t, "s_m": s_t, "speed_kph": v_t * 3.6,
        "rpm": rpm, "gear": gear, 
        "throttle_pct": thr * 100, "brake_pct": brk * 100,
        "g_lat": g_lat, "g_long": g_long,
        "engine_temp_c": temp, "engine_power_w": pwr,
        "x_m": x_t, "y_m": y_t, "heading_rad": h_t
    }

# ----------------------------
# 3. Stint Manager (Boucle Multi-Tours)
# ----------------------------

def run_stint(track: TrackProfile, vehicle: F1_2026, tuning: PlayerTuning, n_laps: int, hz: float):
    # 1. Sauvegarde courbe couple originale
    base_torque = np.copy(vehicle.trq)
    
    full_telemetry = {
        "time_s": [], "s_m": [], "speed_kph": [], "rpm": [], "gear": [],
        "throttle_pct": [], "brake_pct": [], "g_lat": [], "g_long": [],
        "engine_temp_c": [], "engine_power_w": [], 
        "x_m": [], "y_m": [], "heading_rad": []
    }
    
    total_time = 0.0
    total_dist = 0.0
    
    print(f"=== STINT START: {n_laps} LAPS | Init Temp: {vehicle.t_init}°C ===")

    for lap in range(1, n_laps + 1):
        # A. Calcul Derating (Basé sur la température de départ du tour)
        if vehicle.t_init > vehicle.t_soft:
            excess = vehicle.t_init - vehicle.t_soft
            # Perte violente : 2% par degré au dessus de la limite
            derate = max(0.6, 1.0 - (excess * 0.02)) 
            vehicle.trq = base_torque * derate
            print(f"Lap {lap}: OVERHEAT ({vehicle.t_init:.1f}°C). Power reduced to {derate*100:.0f}%")
        else:
            vehicle.trq = np.copy(base_torque)
            print(f"Lap {lap}: OK ({vehicle.t_init:.1f}°C). Power 100%")
            
        # B. Simulation du Tour
        sol = compute_speed_profile(track, vehicle)
        data = resample_telemetry(track, sol, vehicle, hz=hz)
        
        # C. Update État Véhicule
        # La température finale devient l'initiale du tour suivant
        vehicle.t_init = data["engine_temp_c"][-1]
        
        # D. Concaténation
        full_telemetry["time_s"].extend(data["time_s"] + total_time)
        full_telemetry["s_m"].extend(data["s_m"] + total_dist)
        
        for k in ["speed_kph", "rpm", "gear", "throttle_pct", "brake_pct", 
                  "g_lat", "g_long", "engine_temp_c", "engine_power_w",
                  "x_m", "y_m", "heading_rad"]:
            full_telemetry[k].extend(data[k])
            
        total_time += data["time_s"][-1]
        total_dist += data["s_m"][-1]
        
        print(f" -> Lap Time: {data['time_s'][-1]:.3f}s")

    return full_telemetry

def export_csv(path, tel):
    df = pd.DataFrame(tel)
    df.to_csv(path, index=False)

# ----------------------------
# Main Entry Point
# ----------------------------

def main(
    track_csv: str,
    out_csv: str = "telemetry.csv",
    hz: float = 60.0,
    laps: int = 3,
    aero: int = 10,
    chassis: int = 10,
    cooling: int = 10,
    engine: int = 10,
    downforce: float = 0.5,
    gear_ratio: float = 0.5,
    export: bool = False,
):
    # 1. Load Track
    try:
        df = pd.read_csv(track_csv)
        track = TrackProfile(
            s=df["s_m"].values,
            x=df["x_m"].values,
            y=df["y_m"].values,
            z=df["z_m"].values,
            kappa=df["curvature_radpm"].values,
            slope=df["slope_pct"].values,
            heading=df["heading_rad"].values,
        )
    except Exception as e:
        raise RuntimeError(f"Error loading track: {e}") from e

    # 2. Setup Vehicle
    tuning = PlayerTuning(
        aero, chassis, cooling, engine,
        downforce, gear_ratio
    )

    vehicle = F1_2026()
    vehicle.apply_tuning(tuning)

    # 3. Run Stint
    tel = run_stint(track, vehicle, tuning, n_laps=laps, hz=hz)

    # 4. Export
    if export:
        export_csv(out_csv, tel)

    return tel

if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser()
    ap.add_argument("--track_csv", required=True)
    ap.add_argument("--out_csv", default="telemetry.csv")
    ap.add_argument("--hz", type=float, default=60.0)
    ap.add_argument("--laps", type=int, default=3, help="Nombre de tours") # Stint length

    # Tuning
    ap.add_argument("--aero", type=int, default=10)
    ap.add_argument("--chassis", type=int, default=10)
    ap.add_argument("--cooling", type=int, default=10)
    ap.add_argument("--engine", type=int, default=10)
    ap.add_argument("--downforce", type=float, default=0.5)
    ap.add_argument("--gear_ratio", type=float, default=0.5)

    args = ap.parse_args()

    try:
        main(
            track_csv=args.track_csv,
            out_csv=args.out_csv,
            hz=args.hz,
            laps=args.laps,
            aero=args.aero,
            chassis=args.chassis,
            cooling=args.cooling,
            engine=args.engine,
            downforce=args.downforce,
            gear_ratio=args.gear_ratio,
            export=True,
        )
    except RuntimeError as e:
        print(e)
        sys.exit(1)
