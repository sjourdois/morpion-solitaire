app-title = Morpion Solitaire
variant-label = Variante
score-label = Jugadas
legal-moves-label = Disponibles
algo-label = Algoritmo
nrpa-level-label = Nivel NRPA
nrpa-level-hint = 3 = rápido (~99 en un minuto); 4+ busca más a fondo pero solo compensa en ejecuciones de varias horas
algo-nrpa = NRPA
algo-beam = Beam Search
algo-systematic = Sistemático
algo-perturbation = Perturbación
perturbation-hint = Optimiza localmente la partida cargada: destruye las últimas K jugadas, vuelve a buscar el final, conserva lo mejor, en bucle. Carga primero un récord y déjalo correr.
btn-start = Iniciar
btn-stop = Detener
btn-undo = Deshacer
btn-redo = Rehacer
btn-new = Nueva partida
btn-import = Importar
btn-rotate = Girar
btn-flip = Voltear
btn-recenter = Recentrar
btn-arrows = Flechas
btn-numbers = Números
btn-silence = 🔔 RÉCORD BATIDO — Silenciar
load-record = Cargar un récord
nodes-explored-label = Nodos explorados
nodes-per-second-label = Nodos/s
wasm-rate-disclaimer = Versión navegador: la nativa es varias × más rápida (tasa no comparable)
time-label = Tiempo
records-label = Récords
btn-load-best = Cargar resultado
btn-dismiss-preview = Descartar
btn-checkpoint = Guardar búsqueda
btn-resume-search = Reanudar búsqueda
language-label = Idioma
btn-load = Cargar
btn-cancel = Cancelar
import-hint = Pega una partida guardada (JSON o Pentasol):
status-copied = Posición copiada al portapapeles
status-imported = Importado: {$score} jugadas
status-import-error = Importación no válida: {$error}
status-record-saved = Récord {$score} guardado: {$path}
status-record-save-error = No se pudo guardar el récord: {$error}
status-record-web = Récord {$score} alcanzado
status-checkpoint = Búsqueda guardada
status-resumed = Búsqueda reanudada
status-no-checkpoint = No hay búsqueda guardada
status-search-paused = ⏸ Búsqueda en pausa
status-search-resumed = ▶ Búsqueda reanudada
status-record-beaten = 🔔 ¡RÉCORD BATIDO: {$score} jugadas (récord mundial 5T = {$record})!
status-overflow = ⚠ DESBORDAMIENTO DE CUADRÍCULA {$grid}×{$grid} (alcanzado en {$score} jugadas) — búsqueda detenida, mejor partida guardada en records/overflow/. Amplía `Row` en board.rs para agrandar la cuadrícula.

# ── Mensajes de ejecución de la CLI ────────────────────────────────────────
btn-pause = Pausa
btn-resume = Reanudar
start-point-label = Punto de partida
start-empty = Cruz vacía
start-seeded = Cruz vacía, sembrada con la partida cargada
start-continue = Continuar la partida cargada
start-needs-game = Carga o juega una partida primero.
resume-saved = Guardado
format-label = Formato de exportación
btn-copy = Copiar
btn-export-file = Exportar…
status-exported = Exportado: { $path }
status-png-web = El portapapeles de imagen no está disponible en la web.
start-terminal = La partida cargada está terminada — nada que explorar.
search-section = Búsqueda automática
variant-tip = Líneas de { $len } puntos · { $mode }
touch-touching = extremos compartidos permitidos
touch-disjoint = líneas disjuntas
game-section = Partida
btn-theme = Tema claro / oscuro
btn-shortcuts = Atajos de teclado
shortcuts-title = Atajos de teclado
searching-label = Buscando…
confirm-discard-title = Cambios sin guardar
confirm-discard-body = ¿Guardar la partida actual?
btn-save = Guardar
btn-dont-save = No guardar
rules-title = Reglas
rules-hide = No mostrar al inicio
btn-close = Cerrar
rules-body =
    Objetivo: lograr la cadena de movimientos más larga posible.
    La cuadrícula empieza como una cruz de puntos. Un movimiento coloca un punto en una casilla vacía, siempre que así se completen 5 casillas alineadas (horizontal, vertical o diagonal) cuyas otras 4 ya son puntos; entonces se traza la línea por esos 5 puntos.
    La casilla completada puede estar en un extremo o en el medio de la línea. (En las variantes 4 son 4 casillas alineadas: 3 puntos más 1.)
    Dos líneas de la misma dirección nunca pueden solaparse. En las variantes disjuntas (D) ni siquiera pueden tocarse por un extremo; en las variantes de contacto (T) pueden compartir un extremo.
    Los movimientos posibles están resaltados — haz clic para jugar, o deja que el ordenador busque con la búsqueda automática.

meta-title = Metadatos
meta-author = Autor
meta-source = Fuente
meta-transcribed-by = Transcrito por
meta-description = Descripción
meta-tags = Etiquetas
meta-tags-hint = separadas por comas
author-prompt-title = Tu nombre
author-prompt-body = Escribe tu nombre para firmar tus exportaciones (campo «Autor»).
author-prompt-remember = Recordarme
author-prompt-ok = Guardar
author-prompt-skip = Omitir

exhausted-title = Espacio explorado por completo
exhausted-body = El árbol de juego se exploró exhaustivamente en { $time }. La mejor puntuación, { $score }, es por tanto el óptimo demostrado para esta variante.

status-no-msr-data = Este archivo no contiene datos de Morpion Solitaire.
status-copied-png-no-record = Imagen copiada (sin el registro incrustado: exporta a un archivo PNG para incluirlo).
drop-hint = Suelta un archivo .msr, .png o .svg para cargarlo
link-docs = Docs
link-source = Código

# Line picker mode (Aim = cursor + scroll wheel, Click = click to lock + aim + click to play)
pick-mode-label = Selección
pick-mode-aim = Apuntar
pick-mode-click = Clic
pick-mode-aim-hint = Apunta con el cursor, rueda para cambiar de línea, clic para jugar.
pick-mode-click-hint = Clic para fijar el punto, mueve para apuntar, clic de nuevo para jugar.
pick-locked-hint = Apunta la línea · clic para jugar · clic derecho o Esc para cancelar

# Opciones de ajuste del motor (renderizadas genéricamente desde el registro de plugins)
opt-level = Nivel NRPA
opt-level-hint = Profundidad de anidamiento. 3 = rápido (~99 en un minuto); 4+ busca más profundo pero solo compensa en ejecuciones largas.
opt-width = Anchura del haz
opt-width-hint = Candidatos conservados en cada profundidad. Más ancho = más exhaustivo pero más lento.
opt-symmetry = Codificación por simetría
opt-symmetry-hint = Codificación canónica D4 de jugadas. Desactívala (solo marco identidad) para ~+16 % de rendimiento con puntuación neutra — útil para ejecuciones en frío.
opt-clamp = Recorte de logits (C)
opt-clamp-hint = Recorte Stabilized-NRPA. 3 es el punto óptimo para cazar récords; 0 lo desactiva.
opt-alpha = Tamaño de paso (α)
opt-alpha-hint = Paso de adaptación de la política. 1.0 por defecto; ajústalo solo para experimentos.
opt-crossover = Tasa de cruce
opt-crossover-hint = Solo perturbación: probabilidad de que una ronda recombine dos partidas archivadas en vez de destruir/reparar. 0 = desactivado.
opt-neural-scale = Fuerza del prior neuronal
opt-neural-scale-hint = Escala β del prior neuronal de jugadas; óptimo ≈ 4. Solo se aplica con un prior cargado.

# Panel del prior neuronal (función `neural`)
prior-section = Prior neuronal
prior-none = Ninguno
prior-bundled = Incluido
prior-corpus = Corpus
prior-tabula-rasa = Tabula rasa
prior-file = Archivo
prior-none-hint = NRPA simple — sin prior de jugadas aprendido.
prior-bundled-hint = El prior «from scratch» incluido — instantáneo, sin entrenamiento ni récords humanos.
prior-corpus-hint = Entrena un prior con los récords humanos incluidos (~40 s en CPU).
prior-tabula-rasa-hint = Entrena desde cero por Expert Iteration — sin récords. Aquí minutos; una ejecución seria va en la CLI.
prior-file-hint = Carga un prior guardado antes (safetensors).
btn-load-prior = Cargar…
btn-cancel-training = Cancelar entrenamiento
prior-status-training = Entrenando el prior…
prior-status-ready = Prior listo ✓
prior-status-error = Error: { $error }
algo-puct = PUCT
opt-c-puct = Exploración PUCT (c)
opt-c-puct-hint = Constante de exploración PUCT — más alto explora más. Predeterminado 1.5.
