app-title = Morpion Solitaire
variant-label = Variante
score-label = Mosse
legal-moves-label = Disponibili
algo-label = Algoritmo
nrpa-level-label = Livello NRPA
nrpa-level-hint = 3 = veloce (~99 in un minuto); 4+ cerca più a fondo ma conviene solo su esecuzioni di più ore
algo-nrpa = NRPA
algo-beam = Beam Search
algo-systematic = Sistematico
algo-perturbation = Perturbazione
perturbation-hint = Ottimizza localmente la partita caricata: elimina le ultime K mosse, cerca di nuovo il finale, tiene il migliore, in ciclo. Carica prima un record e lascialo girare.
btn-start = Avvia
btn-stop = Ferma
btn-undo = Annulla
btn-redo = Ripeti
btn-new = Nuova partita
btn-import = Importa
btn-rotate = Ruota
btn-flip = Capovolgi
btn-recenter = Ricentra
btn-arrows = Frecce
btn-numbers = Numeri
btn-silence = 🔔 RECORD BATTUTO — Silenzia
load-record = Carica un record
nodes-explored-label = Nodi esplorati
nodes-per-second-label = Nodi/s
wasm-rate-disclaimer = Versione browser: la nativa è diverse × più veloce (valore non comparabile)
time-label = Tempo
records-label = Record
btn-load-best = Carica risultato
btn-dismiss-preview = Scarta
btn-checkpoint = Salva ricerca
btn-resume-search = Riprendi ricerca
language-label = Lingua
btn-load = Carica
btn-cancel = Annulla
import-hint = Incolla un salvataggio (JSON o Pentasol):
status-copied = Posizione copiata negli appunti
status-imported = Importato: {$score} mosse
status-import-error = Importazione non valida: {$error}
status-record-saved = Record {$score} salvato: {$path}
status-record-save-error = Impossibile salvare il record: {$error}
status-record-web = Record {$score} raggiunto
status-checkpoint = Ricerca salvata
status-resumed = Ricerca ripresa
status-no-checkpoint = Nessuna ricerca salvata
status-search-paused = ⏸ Ricerca in pausa
status-search-resumed = ▶ Ricerca ripresa
status-record-beaten = 🔔 RECORD BATTUTO: {$score} mosse (record mondiale 5T = {$record})!
status-overflow = ⚠ OVERFLOW DELLA GRIGLIA {$grid}×{$grid} (raggiunto a {$score} mosse) — ricerca interrotta, partita migliore salvata in records/overflow/. Amplia `Row` in board.rs per ingrandire la griglia.

# ── Messaggi di runtime della CLI ──────────────────────────────────────────
btn-pause = Pausa
btn-resume = Riprendi
start-point-label = Punto di partenza
start-empty = Croce vuota
start-seeded = Croce vuota, innescata dalla partita caricata
start-continue = Continuare la partita caricata
start-needs-game = Carica o gioca prima una partita.
resume-saved = Salvataggio
format-label = Formato di esportazione
btn-copy = Copia
btn-export-file = Esporta…
status-exported = Esportato: { $path }
status-png-web = Gli appunti immagine non sono disponibili sul web.
start-terminal = La partita caricata è terminata — niente da esplorare.
search-section = Ricerca automatica
variant-tip = Linee di { $len } punti · { $mode }
touch-touching = estremi condivisi consentiti
touch-disjoint = linee disgiunte
game-section = Partita
btn-theme = Tema chiaro / scuro
btn-shortcuts = Scorciatoie da tastiera
shortcuts-title = Scorciatoie da tastiera
searching-label = Ricerca…
confirm-discard-title = Modifiche non salvate
confirm-discard-body = Salvare la partita corrente?
btn-save = Salva
btn-dont-save = Non salvare
rules-title = Regole
rules-hide = Non mostrare all'avvio
btn-close = Chiudi
rules-body =
    Obiettivo: ottenere la più lunga sequenza di mosse possibile.
    La griglia inizia come una croce di punti. Una mossa colloca un punto in una casella vuota, purché così si completino 5 caselle allineate (orizzontale, verticale o diagonale) le cui altre 4 sono già punti; si traccia poi la linea attraverso questi 5 punti.
    La casella completata può essere a un'estremità o al centro della linea. (Nelle varianti 4 sono 4 caselle allineate: 3 punti più 1.)
    Due linee nella stessa direzione non possono mai sovrapporsi. Nelle varianti disgiunte (D) non possono nemmeno toccarsi a un'estremità; nelle varianti a contatto (T) possono condividere un'estremità.
    Le mosse possibili sono evidenziate — fai clic per giocare, oppure lascia cercare il computer con la ricerca automatica.

meta-title = Metadati
meta-author = Autore
meta-source = Fonte
meta-transcribed-by = Trascritto da
meta-description = Descrizione
meta-tags = Etichette
meta-tags-hint = separate da virgole
author-prompt-title = Il tuo nome
author-prompt-body = Inserisci il tuo nome per firmare le tue esportazioni (campo «Autore»).
author-prompt-remember = Ricordami
author-prompt-ok = Salva
author-prompt-skip = Ignora

exhausted-title = Spazio esplorato per intero
exhausted-body = L'albero di gioco è stato esplorato esaustivamente in { $time }. Il punteggio migliore, { $score }, è quindi l'ottimo dimostrato per questa variante.

status-no-msr-data = Questo file non contiene dati di Morpion Solitaire.
status-copied-png-no-record = Immagine copiata (senza il record incorporato: esporta in un file PNG per includerlo).
drop-hint = Trascina qui un file .msr, .png o .svg per caricarlo
link-docs = Doc
link-source = Sorgente

# Line picker mode (Aim = cursor + scroll wheel, Click = click to lock + aim + click to play)
pick-mode-label = Selezione
pick-mode-aim = Mira
pick-mode-click = Clic
pick-mode-aim-hint = Mira col cursore, rotellina per cambiare linea, clic per giocare.
pick-mode-click-hint = Clic per bloccare il punto, muovi per mirare, di nuovo clic per giocare.
pick-locked-hint = Mira la linea · clic per giocare · clic destro o Esc per annullare

# Opzioni di messa a punto del motore (rese genericamente dal registro dei plugin)
opt-level = Livello NRPA
opt-level-hint = Profondità di annidamento. 3 = veloce (~99 in un minuto); 4+ cerca più a fondo ma conviene solo su esecuzioni lunghe.
opt-width = Ampiezza del fascio
opt-width-hint = Candidati mantenuti a ogni profondità. Più ampio = più esaustivo ma più lento.
opt-symmetry = Codifica per simmetria
opt-symmetry-hint = Codifica canonica D4 delle mosse. Disattivala (solo sistema identità) per ~+16% di velocità a punteggio neutro — utile per le ricerche a freddo.
opt-clamp = Limite dei logit (C)
opt-clamp-hint = Limite Stabilized-NRPA. 3 è il valore ideale per la caccia ai record; 0 lo disattiva.
opt-alpha = Passo di adattamento (α)
opt-alpha-hint = Passo di adattamento della policy. 1.0 di default; modificalo solo per esperimenti.
opt-crossover = Tasso di crossover
opt-crossover-hint = Solo perturbazione: probabilità che un round ricombini due partite archiviate invece di distruggere/riparare. 0 = disattivato.
opt-neural-scale = Forza del prior neurale
opt-neural-scale-hint = Scala β del prior neurale delle mosse; ottimo ≈ 4. Si applica solo con un prior caricato.

# Pannello del prior neurale (funzione `neural`)
prior-section = Prior neurale
prior-none = Nessuno
prior-bundled = Incluso
prior-corpus = Corpus
prior-tabula-rasa = Tabula rasa
prior-file = File
prior-none-hint = NRPA semplice — nessun prior di mosse appreso.
prior-bundled-hint = Il prior «from scratch» incluso — istantaneo, senza addestramento né record umani.
prior-corpus-hint = Addestra un prior sui record umani inclusi (~40 s su CPU).
prior-tabula-rasa-hint = Addestra da zero con Expert Iteration — senza record. Qui minuti; un run serio va sulla CLI.
prior-file-hint = Carica un prior salvato in precedenza (safetensors).
btn-load-prior = Carica…
btn-cancel-training = Annulla addestramento
prior-status-training = Addestramento del prior…
prior-status-ready = Prior pronto ✓
prior-status-error = Errore: { $error }
algo-puct = PUCT
opt-c-puct = Esplorazione PUCT (c)
opt-c-puct-hint = Costante di esplorazione PUCT — più alto esplora di più. Predefinito 1.5.
opt-feat-adapt = NRPA spazio-feature
opt-feat-adapt-hint = Adatta una testa sulle feature congelate della rete online (φ-B) invece di un bias fisso. Richiede un prior. Sperimentale.
opt-feat-alpha = Passo spazio-feature (α_θ)
opt-feat-alpha-hint = Passo della testa per NRPA spazio-feature. Predefinito 0.1. Solo se attivo.
opt-macros = Macro-azioni
opt-macros-hint = NRPA sceglie anche motivi multi-mossa estratti dai record (solo 5T). Sperimentale.
opt-macro-k = Lunghezza macro (k)
opt-macro-k-hint = Mosse per motivo (predefinito 2). Applicato al primo uso.
opt-macro-topn = Dimensione libreria macro
opt-macro-topn-hint = Mantieni gli N motivi più frequenti (0 = tutti; predefinito 32).
