app-title = Morpion Solitaire
variant-label = Variante
score-label = Jogadas
legal-moves-label = Disponíveis
algo-label = Algoritmo
nrpa-level-label = Nível NRPA
nrpa-level-hint = 3 = rápido (~99 em um minuto); 4+ busca mais a fundo, mas só compensa em execuções de várias horas
algo-nrpa = NRPA
algo-beam = Beam Search
algo-systematic = Sistemático
algo-perturbation = Perturbação
perturbation-hint = Otimiza localmente o jogo carregado: destrói as últimas K jogadas, busca o final de novo, guarda o melhor, em laço. Carregue primeiro um recorde e deixe rodar.
btn-start = Iniciar
btn-stop = Parar
btn-undo = Desfazer
btn-redo = Refazer
btn-new = Novo jogo
btn-import = Importar
btn-rotate = Girar
btn-flip = Inverter
btn-recenter = Recentralizar
btn-arrows = Setas
btn-numbers = Números
btn-silence = 🔔 RECORDE BATIDO — Silenciar
load-record = Carregar um recorde
nodes-explored-label = Nós explorados
nodes-per-second-label = Nós/s
time-label = Tempo
records-label = Recordes
btn-load-best = Carregar resultado
btn-dismiss-preview = Descartar
btn-checkpoint = Salvar busca
btn-resume-search = Retomar busca
language-label = Idioma
btn-load = Carregar
btn-cancel = Cancelar
import-hint = Cole um jogo salvo (JSON ou Pentasol):
status-copied = Posição copiada para a área de transferência
status-imported = Importado: {$score} jogadas
status-import-error = Importação inválida: {$error}
status-record-saved = Recorde {$score} salvo: {$path}
status-record-save-error = Falha ao salvar o recorde: {$error}
status-record-web = Recorde {$score} alcançado
status-checkpoint = Busca salva
status-resumed = Busca retomada
status-no-checkpoint = Nenhuma busca salva
status-search-paused = ⏸ Busca pausada
status-search-resumed = ▶ Busca retomada
status-record-beaten = 🔔 RECORDE BATIDO: {$score} jogadas (recorde mundial 5T = {$record})!
status-overflow = ⚠ ESTOURO DA GRADE {$grid}×{$grid} (alcançado em {$score} jogadas) — busca interrompida, melhor jogo salvo em records/overflow/. Amplie `Row` em board.rs para aumentar a grade.

# ── Mensagens de execução da CLI ───────────────────────────────────────────
btn-pause = Pausa
btn-resume = Retomar
start-point-label = Ponto de partida
start-empty = Cruz vazia
start-seeded = Cruz vazia, semeada pela partida carregada
start-continue = Continuar a partida carregada
start-needs-game = Carrega ou joga uma partida primeiro.
resume-saved = Guardado
format-label = Formato de exportação
btn-copy = Copiar
btn-export-file = Exportar…
status-exported = Exportado: { $path }
status-png-web = A área de transferência de imagem não está disponível na web.
start-terminal = A partida carregada está terminada — nada a explorar.
search-section = Busca automática
variant-tip = Linhas de { $len } pontos · { $mode }
touch-touching = extremidades partilhadas permitidas
touch-disjoint = linhas disjuntas
game-section = Partida
btn-theme = Tema claro / escuro
btn-shortcuts = Atalhos de teclado
shortcuts-title = Atalhos de teclado
searching-label = A procurar…
confirm-discard-title = Alterações não guardadas
confirm-discard-body = Guardar a partida atual?
btn-save = Guardar
btn-dont-save = Não guardar
rules-title = Regras
rules-hide = Não mostrar no arranque
btn-close = Fechar
rules-body =
    Objetivo: conseguir a maior sequência de jogadas possível.
    A grelha começa como uma cruz de pontos. Uma jogada coloca um ponto numa casa vazia, desde que assim se completem 5 casas alinhadas (horizontal, vertical ou diagonal) cujas outras 4 já são pontos; traça-se então a linha por esses 5 pontos.
    A casa completada pode estar numa extremidade ou no meio da linha. (Nas variantes 4 são 4 casas alinhadas: 3 pontos mais 1.)
    Duas linhas da mesma direção nunca podem sobrepor-se. Nas variantes disjuntas (D) não podem sequer tocar-se numa extremidade; nas variantes de contacto (T) podem partilhar uma extremidade.
    As jogadas possíveis estão destacadas — clica para jogar, ou deixa o computador procurar com a busca automática.

meta-title = Metadados
meta-author = Autor
meta-source = Fonte
meta-transcribed-by = Transcrito por
meta-description = Descrição
meta-tags = Etiquetas
meta-tags-hint = separadas por vírgulas
author-prompt-title = O seu nome
author-prompt-body = Introduza o seu nome para assinar as suas exportações (campo «Autor»).
author-prompt-remember = Lembrar-me
author-prompt-ok = Guardar
author-prompt-skip = Ignorar

exhausted-title = Espaço totalmente explorado
exhausted-body = A árvore de jogo foi explorada exaustivamente em { $time }. A melhor pontuação, { $score }, é portanto o ótimo comprovado para esta variante.

status-no-msr-data = Este ficheiro não contém dados de Morpion Solitaire.
status-copied-png-no-record = Imagem copiada (sem o registo incorporado — exporte para um ficheiro PNG para o incluir).
drop-hint = Largue um ficheiro .msr, .png ou .svg para o carregar
link-docs = Docs
link-source = Código
