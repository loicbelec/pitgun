import logging
import numpy as np
import math

logger = logging.getLogger(__name__)
logger.addHandler(logging.NullHandler())
logger.setLevel(logging.DEBUG)

class Vehicle:
    def __init__(self):
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

        # Limits
        self.n_idle = 4000.0       
        self.n_max = 15000.0       

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
        self.c_th = 500.0       
        self.alpha_heat = 0.45     
        self.p_cool0 = 10.0     
        self.k_cool = 0.01     
        self.beta_derate = 0.004

    @staticmethod
    def generate_gear_ratios_total(p: "Vehicle") -> list[float]:
        return [p.g1_total * ((p.g_last_total / p.g1_total) ** (k / (p.gear_count-1))) for k in range(p.gear_count)]
 
    def apply_tuning(self, tuning):
        logger.warning("Tuning not effective on base Vehicle class")
        pass

    def power_kw_from_rpm(self, rpm):
        return np.interp(rpm, self.n, self.trq, left = 0, right = 0) * rpm * math.pi / 30

    def max_engine_power(self, speed: float,temp: float) -> float:
        gear_choice = 1
        pwr_max = 0
        rpm_Pmax = 0
         
        for gear in range(self.gear_count):
            gear_ratio = self.gear_ratios[gear]
            rpm = speed * 60 * gear_ratio / (2 * math.pi * self.r_wheel)
            pwr = self.power_kw_from_rpm(rpm)
            if pwr > pwr_max:
                pwr_max = pwr
                gear_choice = gear+1
                rpm_Pmax = rpm

        if temp > self.t_soft:
            loss = (temp - self.t_soft) * self.beta_derate
            derate = max(0.2, 1.0 - loss)
            pwr_max *= derate


        return pwr_max, rpm_Pmax, gear_choice
    
