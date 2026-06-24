app-title = Morpion Solitaire
variant-label = Variante
score-label = Züge
legal-moves-label = Verfügbar
algo-label = Algorithmus
nrpa-level-label = NRPA-Stufe
nrpa-level-hint = 3 = schnell (~99 in einer Minute); 4+ sucht tiefer, lohnt sich aber nur bei mehrstündigen Läufen
algo-nrpa = NRPA
algo-beam = Beam Search
algo-systematic = Systematisch
algo-perturbation = Perturbation
perturbation-hint = Optimiert das geladene Spiel lokal: die letzten K Züge verwerfen, das Ende neu suchen, das Beste behalten – in einer Schleife. Lade zuerst einen Rekord und lass es laufen.
btn-start = Start
btn-stop = Stopp
btn-undo = Rückgängig
btn-redo = Wiederholen
btn-new = Neues Spiel
btn-import = Importieren
btn-rotate = Drehen
btn-flip = Spiegeln
btn-recenter = Zentrieren
btn-arrows = Pfeile
btn-numbers = Nummern
btn-silence = 🔔 REKORD GEBROCHEN — Stumm
load-record = Rekord laden
nodes-explored-label = Untersuchte Knoten
nodes-per-second-label = Knoten/s
wasm-rate-disclaimer = Browser-Version: nativ läuft um ein Vielfaches schneller (Rate nicht vergleichbar)
time-label = Zeit
records-label = Rekorde
btn-load-best = Ergebnis laden
btn-dismiss-preview = Verwerfen
btn-checkpoint = Suche speichern
btn-resume-search = Suche fortsetzen
language-label = Sprache
btn-load = Laden
btn-cancel = Abbrechen
import-hint = Speicherstand einfügen (JSON oder Pentasol):
status-copied = Position in die Zwischenablage kopiert
status-imported = Importiert: {$score} Züge
status-import-error = Ungültiger Import: {$error}
status-record-saved = Rekord {$score} gespeichert: {$path}
status-record-save-error = Rekord konnte nicht gespeichert werden: {$error}
status-record-web = Rekord {$score} erreicht
status-checkpoint = Suche gespeichert
status-resumed = Suche fortgesetzt
status-no-checkpoint = Keine gespeicherte Suche
status-search-paused = ⏸ Suche pausiert
status-search-resumed = ▶ Suche fortgesetzt
status-record-beaten = 🔔 REKORD GEBROCHEN: {$score} Züge (5T-Weltrekord = {$record})!
status-overflow = ⚠ RASTERÜBERLAUF {$grid}×{$grid} (bei {$score} Zügen erreicht) — Suche gestoppt, bestes Spiel unter records/overflow/ gespeichert. Erweitere `Row` in board.rs, um das Raster zu vergrößern.

# ── CLI-Laufzeitmeldungen ──────────────────────────────────────────────────
btn-pause = Pause
btn-resume = Fortsetzen
start-point-label = Startpunkt
start-empty = Leeres Kreuz
start-seeded = Leeres Kreuz, vorbereitet aus dem geladenen Spiel
start-continue = Geladenes Spiel fortsetzen
start-needs-game = Zuerst ein Spiel laden oder spielen.
resume-saved = Gespeichert
format-label = Exportformat
btn-copy = Kopieren
btn-export-file = Exportieren…
status-exported = Exportiert: { $path }
status-png-web = Bild-Zwischenablage ist im Web nicht verfügbar.
start-terminal = Das geladene Spiel ist beendet – nichts zu erkunden.
search-section = Automatische Suche
variant-tip = Linien aus { $len } Punkten · { $mode }
touch-touching = gemeinsame Endpunkte erlaubt
touch-disjoint = disjunkte Linien
game-section = Spiel
btn-theme = Helles / dunkles Design
btn-shortcuts = Tastenkürzel
shortcuts-title = Tastenkürzel
searching-label = Suche…
confirm-discard-title = Nicht gespeicherte Änderungen
confirm-discard-body = Aktuelles Spiel speichern?
btn-save = Speichern
btn-dont-save = Nicht speichern
rules-title = Regeln
rules-hide = Beim Start nicht mehr zeigen
btn-close = Schließen
rules-body =
    Ziel: die längstmögliche Zugfolge erreichen.
    Das Spielfeld beginnt als Kreuz aus Punkten. Ein Zug setzt einen Punkt auf ein leeres Feld, sofern damit 5 ausgerichtete Felder (waagerecht, senkrecht oder diagonal) vervollständigt werden, deren übrige 4 bereits Punkte sind; dann zieht man die Linie durch diese 5 Punkte.
    Das vervollständigte Feld darf am Ende oder in der Mitte der Linie liegen. (In den 4-Varianten sind es 4 Felder: 3 Punkte plus 1.)
    Zwei Linien gleicher Richtung dürfen sich nie überlappen. In den disjunkten (D) Varianten dürfen sie sich nicht einmal an einem Ende berühren; in den berührenden (T) Varianten dürfen sie ein Ende teilen.
    Mögliche Züge sind hervorgehoben — klicken zum Spielen, oder den Computer über die automatische Suche suchen lassen.

meta-title = Metadaten
meta-author = Autor
meta-source = Quelle
meta-transcribed-by = Transkribiert von
meta-description = Beschreibung
meta-tags = Schlagwörter
meta-tags-hint = durch Komma getrennt
author-prompt-title = Ihr Name
author-prompt-body = Geben Sie Ihren Namen ein, um Ihre Exporte zu signieren (Feld „Autor“).
author-prompt-remember = Angaben merken
author-prompt-ok = Speichern
author-prompt-skip = Überspringen

exhausted-title = Gesamter Raum durchsucht
exhausted-body = Der Spielbaum wurde in { $time } vollständig durchsucht. Das beste Ergebnis, { $score }, ist somit das bewiesene Optimum für diese Variante.

status-no-msr-data = Diese Datei enthält keine Morpion-Solitaire-Daten.
status-copied-png-no-record = Bild kopiert (ohne eingebetteten Datensatz — als PNG-Datei exportieren, um ihn einzuschließen).
drop-hint = Ziehen Sie eine .msr-, .png- oder .svg-Datei hierher, um sie zu laden
link-docs = Doku
link-source = Quelltext

# Line picker mode (Aim = cursor + scroll wheel, Click = click to lock + aim + click to play)
pick-mode-label = Auswahl
pick-mode-aim = Zielen
pick-mode-click = Klick
pick-mode-aim-hint = Mit dem Cursor zielen, Mausrad wechselt die Linie, Klick spielt.
pick-mode-click-hint = Klicken zum Sperren des Punkts, bewegen zum Zielen, erneut klicken zum Spielen.
pick-locked-hint = Linie anvisieren · Klick zum Spielen · Rechtsklick oder Esc zum Abbrechen

# Engine-Feinabstimmung (generisch aus dem Plugin-Registry gerendert)
opt-level = NRPA-Stufe
opt-level-hint = Verschachtelungstiefe. 3 = schnell (~99 in einer Minute); 4+ sucht tiefer, lohnt sich aber erst über lange Läufe.
opt-width = Strahlbreite
opt-width-hint = Pro Tiefe behaltene Kandidaten. Breiter = gründlicher, aber langsamer.
opt-symmetry = Symmetrie-Kodierung
opt-symmetry-hint = Kanonische D4-Zugkodierung. Aus (nur Identitätsrahmen) für ~+16 % Durchsatz bei neutralem Score — gut für kalte Rekordläufe.
opt-clamp = Logit-Begrenzung (C)
opt-clamp-hint = Stabilized-NRPA-Begrenzung. 3 ist der Idealwert für die Rekordsuche; 0 schaltet sie ab.
opt-alpha = Schrittweite (α)
opt-alpha-hint = Anpassungsschritt der Strategie. Standard 1.0; nur für Experimente ändern.
opt-crossover = Crossover-Rate
opt-crossover-hint = Nur Perturbation: Wahrscheinlichkeit, dass eine Runde zwei archivierte Spiele rekombiniert statt zu zerstören/reparieren. 0 = aus.
opt-neural-scale = Stärke des neuronalen Priors
opt-neural-scale-hint = β-Skalierung des neuronalen Zug-Priors; Idealwert ≈ 4. Gilt nur mit geladenem Prior.
