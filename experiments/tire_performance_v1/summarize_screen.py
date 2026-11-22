#!/usr/bin/env python3
"""Derived physical-screen report; no selection or coefficient changes."""
import collections
import pathlib
from kernel import EXP, ROOT, key, load, sha, write

def main():
 out=EXP/'results';receipt=load(out/'screen-campaign-v1.json');decisions=load(out/'profile-decisions-v1.json');selection=load(out/'profile-selection-lock-v1.json')
 for name,digest in receipt['files'].items():
  if sha((out/name).read_bytes())!=digest:raise RuntimeError('retained receipt differs:'+name)
 pin=receipt['provenance']['kernel_pin']
 for path,digest in pin['sources'].items():
  if sha((ROOT/path).read_bytes())!=digest:raise RuntimeError('physical source differs:'+path)
 unique={}
 for phase in ['profile-screen','driving-modes','five-lap-cooling','selected-exact-repeat','legacy-equivalence']:
  for r in load(out/(phase+'-v1.json.gz')):unique[key(r['case'])]=r
 carry=[max(r['data']['maximum_carry_jumps'][i] for r in unique.values()) for i in range(5)];failed=[{'case':r['case'],'outcome':r['data']['outcome']} for r in unique.values() if not r['data']['outcome']['ok']]
 service_residual=max(abs(r['data']['outcome']['time_s']-sum(r['data']['lap_times_s'])-r['data']['receipt']['resolved_pit_loss_s']*len(r['case']['strategy']['pit_laps'])) for r in unique.values() if r['data']['outcome']['ok'])
 summary={'schema':'pitgun.tire-performance-summary/v1','status':'NATIVE_PHYSICAL_SCREEN_ONLY','profile':selection['profile'],'phase_stats':receipt['phase_stats'],'legacy_equivalence':load(out/'legacy-equivalence-receipt-v1.json')['phase_stats'],'unique_semantic_cases':len(unique),'fresh_native_executions':sum(x['fresh'] for x in receipt['phase_stats'].values())+load(out/'legacy-equivalence-receipt-v1.json')['phase_stats']['fresh']+2,'qualification_native_controls':2,'failed_simulations':failed,'maximum_carry_jumps_speed_engine_surface_carcass_wear':carry,'maximum_pit_accounting_residual_s':service_residual,'profiles':decisions,'field_reserved_seeds_opened':False,'no_runtime_or_catalog_admission':True}
 write(out/'summary-v1.json',summary)
 lines=['# Pneus : correction de l’incitation aux arrêts','',f'**Candidat natif retenu : `{selection["profile"]}`.** Il passe le protocole physique réduit ; il n’est pas encore admis dans le jeu, WASM ou Authority/Verifier. Les caractéristiques et le plateau des neuf IA restent à évaluer sous ce modèle.','', '## Résultats de la campagne','',f'- {summary["fresh_native_executions"]:_} exécutions natives fraîches, dont 100 GP source exacts, 2 contrôles historiques et 4 répétitions choisies.'.replace('_',' '),f'- {len(failed)} simulation en échec ; continuité vitesse/moteur/surface/carcasse/usure : `{carry}`.',f'- Coût réel conservé : 22 s par arrêt. Résidu maximal du bilan temps total/tours/stands : `{service_residual:.3g} s`.','- Aucun seed réservé du futur plateau n’a été ouvert.','', 'Les stratégies normales comportent un ou deux arrêts. Les plans de 3 à 12 arrêts sont explicitement diagnostiques : ils ne peuvent pas devenir des plans du plateau. La colonne « marge 3+ » donne le meilleur temps diagnostique moins le meilleur temps légal ; une valeur positive favorise le plan légal.','', '| Courbe | Contexte | Arrêts du meilleur plan légal | Temps légal (s) | Marge 3+ (s) | Critères |','|---|---|---:|---:|---:|---|']
 for d in decisions:
  for c in d['contexts']:
   lines.append(f'| {d["profile"]} | Ère {c["context"][0]} · {c["context"][1]} | {len(c["best_legal_case"]["strategy"]["pit_laps"])} | {c["best_legal_time_s"]:.2f} | {c["extra_stops_margin_s"]:+.2f} | {"Passe" if c["eligible"] else "REFINE"} |')
 lines+=['','## Sens et limites','', 'La loi progressive conserve l’accumulation énergétique et thermique d’usure. Elle réduit surtout la pénalité prématurée de grip. La fenêtre thermique et les écarts de grip entre composés sont des hypothèses explicites de modèle réduit, communes à tous les circuits ; aucun temps final n’est corrigé artificiellement.','', 'La sélection est globale, selon la priorité et les critères enregistrés avant calcul. Ces comparaisons discrètes ne prouvent pas un optimum global ni une calibration industrielle. Elles couvrent quatre contextes des ères I/II à réglages fixés. Le transfert aux autres circuits, aux neuf pilotes, aux budgets et aux conditions réservées fait partie de la campagne suivante.','', 'Le bilan des modes de conduite et les essais de cooling sur cinq tours sont disponibles dans les reçus ; le +1 cooling hors budget isole un effet physique et ne représente pas un duel légal à budget égal. Une température initiale de 70 °C est une hypothèse de préparation du jeu, pas une reconstitution historique de toutes les ères.','', 'Références, équations et protocole : [README](README.md), [profils](profile-candidates-v1.json), [plan figé](plan-v1.json). Reçus complets : [campagne](results/screen-campaign-v1.json), [résumé JSON](results/summary-v1.json).']
 (EXP/'RESULTS_V1.md').write_text('\n'.join(lines)+'\n');print({'profile':summary['profile'],'fresh_native_executions':summary['fresh_native_executions'],'failures':len(failed),'pit_residual_s':service_residual})

if __name__=='__main__':main()
