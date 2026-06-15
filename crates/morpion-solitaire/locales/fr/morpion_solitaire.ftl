app-title = Morpion Solitaire
variant-label = Variante
score-label = Coups
legal-moves-label = Disponibles
algo-label = Algorithme
nrpa-level-label = Niveau NRPA
nrpa-level-hint = 3 = rapide (~99 en une minute) ; 4+ cherche plus profond mais ne paie que sur des runs de plusieurs heures
algo-nrpa = NRPA
algo-beam = Beam Search
algo-systematic = Systématique
algo-perturbation = Perturbation
perturbation-hint = Optimise localement la partie chargée : détruit les K derniers coups, re-cherche la fin, garde le meilleur, en boucle. Charge un record d'abord et laisse tourner.
btn-start = Démarrer
btn-stop = Arrêter
btn-undo = Annuler
btn-redo = Rétablir
btn-new = Nouvelle partie
btn-import = Importer
btn-rotate = Tourner
btn-flip = Inverser
btn-recenter = Recentrer
btn-arrows = Flèches
btn-numbers = Numéros
btn-silence = 🔔 RECORD BATTU — Silence
load-record = Charger un record
nodes-explored-label = Nœuds explorés
nodes-per-second-label = Nœuds/s
wasm-rate-disclaimer = Version navigateur : le natif est plusieurs × plus rapide (débit non comparable)
time-label = Temps
records-label = Records
btn-load-best = Charger le résultat
btn-dismiss-preview = Rejeter
btn-checkpoint = Sauvegarder la recherche
btn-resume-search = Reprendre la recherche
language-label = Langue
btn-load = Charger
btn-cancel = Annuler
import-hint = Coller une sauvegarde (JSON ou Pentasol) :
status-copied = Position copiée dans le presse-papiers
status-imported = Importé : {$score} coups
status-import-error = Import invalide : {$error}
status-record-saved = Record {$score} sauvegardé : {$path}
status-record-save-error = Échec de la sauvegarde du record : {$error}
status-record-web = Record {$score} atteint
status-checkpoint = Recherche sauvegardée
status-resumed = Recherche reprise
status-no-checkpoint = Aucune sauvegarde de recherche
status-search-paused = ⏸ Recherche en pause
status-search-resumed = ▶ Recherche reprise
status-record-beaten = 🔔 RECORD BATTU : {$score} coups (record mondial 5T = {$record}) !
status-overflow = ⚠ DÉBORDEMENT DE GRILLE {$grid}×{$grid} (atteint à {$score} coups) — recherche arrêtée, meilleur jeu sauvegardé dans records/overflow/. Élargis `Row` dans board.rs pour agrandir la grille.

# ── Messages d'exécution de la CLI (les clés GUI sont au-dessus) ────────────
btn-pause = Pause
btn-resume = Reprendre
start-point-label = Point de départ
start-empty = Croix vierge
start-seeded = Croix vierge, amorcée par la partie chargée
start-continue = Continuer la partie chargée
start-needs-game = Charge ou joue d'abord une partie.
resume-saved = Sauvegarde
format-label = Format d'export
btn-copy = Copier
btn-export-file = Exporter…
status-exported = Exporté : { $path }
status-png-web = Le presse-papier image n'est pas disponible sur le web.
start-terminal = La partie chargée est terminée — rien à explorer.
search-section = Recherche automatique
variant-tip = Lignes de { $len } points · { $mode }
touch-touching = extrémités communes autorisées
touch-disjoint = lignes disjointes
game-section = Partie
btn-theme = Thème clair / sombre
btn-shortcuts = Raccourcis clavier
shortcuts-title = Raccourcis clavier
searching-label = Recherche…
confirm-discard-title = Modifications non enregistrées
confirm-discard-body = Enregistrer la partie en cours ?
btn-save = Enregistrer
btn-dont-save = Ne pas enregistrer
rules-title = Règles
rules-hide = Ne plus afficher au démarrage
btn-close = Fermer
rules-body =
    But : réaliser la plus longue suite de coups possible.
    Au début, la grille forme une croix de points. Un coup pose un point dans une case vide, à condition de compléter ainsi 5 cases alignées (horizontale, verticale ou diagonale) dont les 4 autres sont déjà des points ; on trace alors la ligne de ces 5 points.
    La case complétée peut être à une extrémité ou au milieu de la ligne. (Dans les variantes 4, c'est 4 cases alignées : 3 points plus 1.)
    Deux lignes de même direction ne peuvent jamais se recouvrir. En variante disjointe (D), elles ne peuvent même pas se toucher par une extrémité ; en variante touchante (T), elles peuvent partager une extrémité.
    Les coups possibles sont surlignés — cliquez pour jouer, ou laissez l'ordinateur chercher via « Recherche automatique ».

meta-title = Métadonnées
meta-author = Auteur
meta-source = Source
meta-transcribed-by = Transcrit par
meta-description = Description
meta-tags = Étiquettes
meta-tags-hint = séparées par des virgules
author-prompt-title = Votre nom
author-prompt-body = Indiquez votre nom pour signer vos exports (champ « Auteur »).
author-prompt-remember = Se souvenir de moi
author-prompt-ok = Enregistrer
author-prompt-skip = Ignorer

exhausted-title = Espace entièrement exploré
exhausted-body = L'arbre de jeu a été entièrement exploré en { $time }. Le meilleur score, { $score }, est donc l'optimum prouvé pour cette variante.

status-no-msr-data = Ce fichier ne contient pas de données Morpion Solitaire.
status-copied-png-no-record = Image copiée (sans l'enregistrement intégré — l'export en fichier PNG l'inclut).
drop-hint = Déposez un fichier .msr, .png ou .svg pour le charger
link-docs = Doc
link-source = Source
