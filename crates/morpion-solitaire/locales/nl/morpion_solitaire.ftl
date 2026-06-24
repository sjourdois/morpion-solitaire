app-title = Morpion Solitaire
variant-label = Variant
score-label = Zetten
legal-moves-label = Beschikbaar
algo-label = Algoritme
nrpa-level-label = NRPA-niveau
nrpa-level-hint = 3 = snel (~99 in een minuut); 4+ zoekt dieper maar loont alleen bij runs van meerdere uren
algo-nrpa = NRPA
algo-beam = Beam Search
algo-systematic = Systematisch
algo-perturbation = Perturbatie
perturbation-hint = Optimaliseert het geladen spel lokaal: verwijder de laatste K zetten, zoek het einde opnieuw, houd het beste, in een lus. Laad eerst een record en laat het lopen.
btn-start = Start
btn-stop = Stop
btn-undo = Ongedaan maken
btn-redo = Opnieuw
btn-new = Nieuw spel
btn-import = Importeren
btn-rotate = Draaien
btn-flip = Spiegelen
btn-recenter = Centreren
btn-arrows = Pijlen
btn-numbers = Nummers
btn-silence = 🔔 RECORD VERBROKEN — Dempen
load-record = Een record laden
nodes-explored-label = Onderzochte knopen
nodes-per-second-label = Knopen/s
wasm-rate-disclaimer = Browserversie: native is meerdere × sneller (snelheid niet vergelijkbaar)
time-label = Tijd
records-label = Records
btn-load-best = Resultaat laden
btn-dismiss-preview = Negeren
btn-checkpoint = Zoekopdracht opslaan
btn-resume-search = Zoekopdracht hervatten
language-label = Taal
btn-load = Laden
btn-cancel = Annuleren
import-hint = Plak een opslag (JSON of Pentasol):
status-copied = Positie naar klembord gekopieerd
status-imported = Geïmporteerd: {$score} zetten
status-import-error = Ongeldige import: {$error}
status-record-saved = Record {$score} opgeslagen: {$path}
status-record-save-error = Record opslaan mislukt: {$error}
status-record-web = Record {$score} bereikt
status-checkpoint = Zoekopdracht opgeslagen
status-resumed = Zoekopdracht hervat
status-no-checkpoint = Geen opgeslagen zoekopdracht
status-search-paused = ⏸ Zoekopdracht gepauzeerd
status-search-resumed = ▶ Zoekopdracht hervat
status-record-beaten = 🔔 RECORD VERBROKEN: {$score} zetten (5T-wereldrecord = {$record})!
status-overflow = ⚠ RASTEROVERLOOP {$grid}×{$grid} (bereikt bij {$score} zetten) — zoekopdracht gestopt, beste spel opgeslagen onder records/overflow/. Verbreed `Row` in board.rs om het raster te vergroten.

# ── CLI-runtimeberichten ───────────────────────────────────────────────────
btn-pause = Pauze
btn-resume = Hervatten
start-point-label = Beginpunt
start-empty = Leeg kruis
start-seeded = Leeg kruis, voorbereid met het geladen spel
start-continue = Geladen spel voortzetten
start-needs-game = Laad of speel eerst een spel.
resume-saved = Opgeslagen
format-label = Exportformaat
btn-copy = Kopiëren
btn-export-file = Exporteren…
status-exported = Geëxporteerd: { $path }
status-png-web = Afbeeldingsklembord is niet beschikbaar op het web.
start-terminal = Het geladen spel is afgelopen — niets te verkennen.
search-section = Automatisch zoeken
variant-tip = Lijnen van { $len } punten · { $mode }
touch-touching = gedeelde eindpunten toegestaan
touch-disjoint = disjuncte lijnen
game-section = Spel
btn-theme = Licht / donker thema
btn-shortcuts = Sneltoetsen
shortcuts-title = Sneltoetsen
searching-label = Zoeken…
confirm-discard-title = Niet-opgeslagen wijzigingen
confirm-discard-body = Huidig spel opslaan?
btn-save = Opslaan
btn-dont-save = Niet opslaan
rules-title = Regels
rules-hide = Niet tonen bij opstarten
btn-close = Sluiten
rules-body =
    Doel: de langst mogelijke reeks zetten maken.
    Het speelveld begint als een kruis van punten. Een zet plaatst een punt op een leeg vakje, mits daarmee 5 uitgelijnde vakjes (horizontaal, verticaal of diagonaal) compleet worden waarvan de andere 4 al punten zijn; je trekt dan de lijn door die 5 punten.
    Het ingevulde vakje mag aan een uiteinde of in het midden van de lijn liggen. (In de 4-varianten zijn het 4 uitgelijnde vakjes: 3 punten plus 1.)
    Twee lijnen in dezelfde richting mogen elkaar nooit overlappen. In de disjuncte (D) varianten mogen ze elkaar zelfs niet aan een uiteinde raken; in de rakende (T) varianten mogen ze één uiteinde delen.
    Mogelijke zetten zijn gemarkeerd — klik om te spelen, of laat de computer zoeken via Automatisch zoeken.

meta-title = Metagegevens
meta-author = Auteur
meta-source = Bron
meta-transcribed-by = Getranscribeerd door
meta-description = Beschrijving
meta-tags = Labels
meta-tags-hint = door kommas gescheiden
author-prompt-title = Uw naam
author-prompt-body = Voer uw naam in om uw exports te ondertekenen (veld “Auteur”).
author-prompt-remember = Mij onthouden
author-prompt-ok = Opslaan
author-prompt-skip = Overslaan

exhausted-title = Hele ruimte doorzocht
exhausted-body = De spelboom is volledig doorzocht in { $time }. De beste score, { $score }, is daarmee het bewezen optimum voor deze variant.

status-no-msr-data = Dit bestand bevat geen Morpion Solitaire-gegevens.
status-copied-png-no-record = Afbeelding gekopieerd (zonder het ingesloten record — exporteer naar een PNG-bestand om het op te nemen).
drop-hint = Sleep een .msr-, .png- of .svg-bestand hierheen om het te laden
link-docs = Docs
link-source = Broncode

# Line picker mode (Aim = cursor + scroll wheel, Click = click to lock + aim + click to play)
pick-mode-label = Keuze
pick-mode-aim = Richten
pick-mode-click = Klik
pick-mode-aim-hint = Richt met de cursor, scrollwiel wisselt de lijn, klik om te spelen.
pick-mode-click-hint = Klik om het punt te vergrendelen, beweeg om te richten, klik nogmaals om te spelen.
pick-locked-hint = Richt de lijn · klik om te spelen · rechtsklik of Esc om te annuleren

# Engine-afstemopties (generiek gerenderd vanuit het plugin-register)
opt-level = NRPA-niveau
opt-level-hint = Nestdiepte. 3 = snel (~99 in een minuut); 4+ zoekt dieper maar loont alleen bij lange runs.
opt-width = Bundelbreedte
opt-width-hint = Kandidaten die per diepte behouden blijven. Breder = grondiger maar trager.
opt-symmetry = Symmetriecodering
opt-symmetry-hint = Canonieke D4-zetcodering. Uit (alleen identiteitsframe) voor ~+16% doorvoer bij neutrale score — handig voor koude recordruns.
opt-clamp = Logit-begrenzing (C)
opt-clamp-hint = Stabilized-NRPA-begrenzing. 3 is het zoete punt voor recordjacht; 0 schakelt het uit.
opt-alpha = Stapgrootte (α)
opt-alpha-hint = Aanpassingsstap van het beleid. Standaard 1.0; alleen bijstellen voor experimenten.
opt-crossover = Crossover-percentage
opt-crossover-hint = Alleen perturbatie: kans dat een ronde twee gearchiveerde spellen hercombineert in plaats van vernietigen/herstellen. 0 = uit.
opt-neural-scale = Sterkte van de neurale prior
opt-neural-scale-hint = β-schaal van de neurale zet-prior; optimaal ≈ 4. Alleen van toepassing met een geladen prior.

# Paneel voor de neurale prior (functie `neural`)
prior-section = Neurale prior
prior-none = Geen
prior-bundled = Meegeleverd
prior-corpus = Corpus
prior-tabula-rasa = Tabula rasa
prior-file = Bestand
prior-none-hint = Gewone NRPA — geen geleerde zet-prior.
prior-bundled-hint = De meegeleverde from-scratch-prior — direct, zonder training of menselijke records.
prior-corpus-hint = Traint een prior op de meegeleverde menselijke records (~40 s op CPU).
prior-tabula-rasa-hint = Traint vanaf nul met Expert Iteration — zonder records. Hier minuten; een serieuze run hoort op de CLI.
prior-file-hint = Laad een eerder opgeslagen prior (safetensors).
btn-load-prior = Laden…
btn-cancel-training = Training annuleren
prior-status-training = Prior wordt getraind…
prior-status-ready = Prior gereed ✓
prior-status-error = Fout: { $error }
algo-puct = PUCT
opt-c-puct = PUCT-exploratie (c)
opt-c-puct-hint = PUCT-exploratieconstante — hoger verkent meer. Standaard 1.5.
opt-feat-adapt = Feature-ruimte-NRPA
opt-feat-adapt-hint = Past online een kop aan over de bevroren netwerk-features (φ-B) in plaats van een vaste prior-bias. Vereist een prior. Experimenteel.
opt-feat-alpha = Feature-ruimte-stap (α_θ)
opt-feat-alpha-hint = Stapgrootte van de kop voor feature-ruimte-NRPA. Standaard 0.1. Alleen indien actief.
