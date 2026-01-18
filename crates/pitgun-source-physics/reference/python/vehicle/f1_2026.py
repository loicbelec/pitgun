import logging
import numpy as np

from .vehicle import Vehicle

logger = logging.getLogger(__name__)
logger.addHandler(logging.NullHandler())
logger.setLevel(logging.DEBUG)

class F1_2026(Vehicle):
    def __init__(self):
        super().__init__()
        # Baseline physical params (F1 2026 approx)
        self.m = 768.0             
        self.rho = 1.225           
        self.g = 9.81              
        self.r_wheel = 0.36        
        self.mu = 1.7              
        self.c_rr = 0.015          

        # Aero modes
        self.cdA_X = 0.85          
        self.cdA_Z = 1.50          
        self.clA_X = 2.6           
        self.clA_Z = 4.13          

        # Power curve
        self.n = np.arange(0, 15001, 250)
        self.trq = np.concatenate((np.linspace(0.44, 0.59, 43) , np.linspace(0.57, 0.455, 8) , np.linspace(0.44, 0.32, 9) , [0.16]))

        # Gearbox
        self.g1_total = 14.0
        self.g_last_total = 4.7
        self.gear_count = 8
        self.n_upshift = 12300.0
        self.n_downshift = 5500.0
        self.gear_ratios = self.generate_gear_ratios_total(self)

        # Thermals & Derating
        self.t_amb = 35.0          
        self.t_init = 90.0         
        self.t_soft = 110.0        
        self.c_th = 500000.0       
        self.alpha_heat = 0.45     
        self.p_cool0 = 15000.0     
        self.k_cool = 1100.0       
        self.beta_derate = 0.004

    def apply_tuning(self, tuning):
        df = float(np.clip(tuning.downforce_slider, 0.0, 1.0))
        gr = float(np.clip(tuning.gear_ratio_slider, 0.0, 1.0))
        aero_k = 1.0 + 0.10 * (tuning.aero_points / 20.0)

        drag_blend = 0.85 + 0.30 * df      
        df_blend   = 0.75 + 0.55 * df      

        self.cdA_X *= aero_k * drag_blend * 0.95
        self.cdA_Z *= aero_k * drag_blend * 1.05
        self.clA_X *= aero_k * df_blend  * 0.95
        self.clA_Z *= aero_k * df_blend  * 1.05

        self.mu *= (1.0 + 0.08 * (tuning.chassis_points / 20.0))
        
        cool_k = (1.0 + 0.35 * (tuning.cooling_points / 20.0))
        self.p_cool0 *= cool_k
        self.k_cool  *= cool_k

        self.trq *= 1 + 0.01 * (tuning.engine_points / 20.0)


        scale = 1.10 - 0.20 * gr 
        self.g1_total *= scale
        self.g_last_total *= scale
        self.gear_ratios = self.generate_gear_ratios_total(self)
