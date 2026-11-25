import copy
from fractions import Fraction
import json
from pathlib import Path
import unittest
from policy import allocate,preparation,project,scale
from run import case,checks,neighbors

class BudgetTests(unittest.TestCase):
    def test_full_ranges_floors_exact_anchors_and_no_extrapolation(self):
        for lo,hi,f,base,anchors in [(8,12,[0,0,0,0],[0,3,3,2],[8,10,12]),(12,24,[2,1,4,1],[2,9,12,1],[12,18,24])]:
            a={str(b):scale(base,b,f)for b in anchors}
            for b in range(lo,hi+1):
                v=allocate(a,b,f);self.assertEqual(sum(v),b);self.assertTrue(all(f[i]<=v[i]<=20 for i in range(4)))
                self.assertEqual(v,allocate(a,b,f))
            for b in anchors:self.assertEqual(allocate(a,b,f),a[str(b)])
            for b in [lo-1,hi+1]:
                with self.assertRaises(ValueError):allocate(a,b,f)
        self.assertEqual(project([Fraction(2)]*4,9,[0]*4),[3,2,2,2])
        self.assertEqual(scale([0,0,80,0],80,[0]*4),[20,20,20,20])
    def test_construction_accepts_only_context_and_available_cap(self):
        p={'contexts':[{'era':2,'circuit':'MONZA','floors':[2,1,4,1],'actors':{'franz':{'anchors':{'12':[2,3,5,2],'24':[6,5,8,5]},'gear':.5,'downforce':.2,'strategy':{'stints':[]}}}}]}
        r=preparation(p,2,'MONZA',16,'franz');self.assertEqual(sum(r['effective_allocation']),16)
        r['effective_allocation'][0]=99;self.assertNotEqual(r,preparation(p,2,'MONZA',16,'franz'))
    def test_equal_player_search_allowance_and_legal_bounds(self):
        for era,f,b,pts in [(1,[0,1,0,0],12,[0,6,6,0]),(2,[2,1,4,1],24,[2,9,12,1])]:
            c={'id':'p','effective_allocation':pts,'gear':0.,'downforce':.5 if era==1 else 0.,'effective_budget':b}
            rows=neighbors(c,era,f);self.assertEqual(len(rows),5)
            self.assertEqual(len({json.dumps({k:v for k,v in x.items()if k!='id'},sort_keys=True)for x in rows}),5)
            for x in rows:self.assertEqual(sum(x['effective_allocation']),b);self.assertTrue(all(x['effective_allocation'][i]>=f[i] for i in range(4)))
    def test_plan_excludes_synthetic_or_unsupported_sources_and_uses_all_contexts(self):
        p=json.loads((Path(__file__).parent/'plan-v1.json').read_text());self.assertEqual(len(p['contexts']),10)
        for ctx in p['contexts']:
            for prop in ctx['proposals']:
                self.assertIn(len(prop['strategy']['pit_laps']),[1,2]);self.assertEqual(sum(s['laps']for s in prop['strategy']['stints']),ctx['laps'])
                if ctx['era']==1:self.assertEqual(prop['downforce'],.5)
        c=case(p['contexts'][3],p['contexts'][3]['proposals'][0],'balanced_reference',12,kind='career',stage='entry')
        self.assertEqual(sum(c['effective_allocation']),12);self.assertEqual(c['downforce'],.5)

if __name__=='__main__':unittest.main()
