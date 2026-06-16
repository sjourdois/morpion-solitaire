#!/usr/bin/env python3
"""Generate the localized landing pages for morpion-solitaire.io.

GitHub Pages serves static files only — it does no Accept-Language negotiation —
so internationalization is done at build time: one self-contained page per
language at a distinct URL (/en/, /fr/, …), cross-linked with hreflang, plus a
root /index.html that redirects the visitor to their preferred language on the
client (with a <noscript> fallback to English).

Run from the repo root:
    python3 tools/landing/build.py --out _site --version 0.1.2

Translations live in TEXT below. The Japanese, Dutch and German copy is
best-effort and should be reviewed by a native speaker.
"""
import argparse
import html
import json
import os

SITE = "https://morpion-solitaire.io"
LANGS = ["en", "fr", "de", "es", "it", "ja", "nl", "pt"]
LANG_NAMES = {
    "en": "English",
    "fr": "Français",
    "de": "Deutsch",
    "es": "Español",
    "it": "Italiano",
    "ja": "日本語",
    "nl": "Nederlands",
    "pt": "Português",
}

# Per-language strings. Keys are identical across languages; English is the
# reference. Proper nouns kept verbatim everywhere: MSR, NRPA, Pentasol,
# WebAssembly, GPL-3.0-or-later, MIT, Apache-2.0, crates.io, 5T/5D/4T/4D.
TEXT = {
    "en": {
        "title": "Morpion Solitaire — play online & solve records",
        "desc": "A fast Morpion Solitaire player and solver: play the four variants in your browser, hunt records, and read/write the self-describing MSR record format.",
        "og_title": "Morpion Solitaire — player & solver",
        "og_desc": "Play the four standard variants in your browser, hunt records (5T world record: 178), and read/write the self-describing MSR format.",
        "tagline": "A fast player and solver. Play the four standard variants in your browser, hunt records with several search engines (NRPA, large-neighbourhood perturbation, exhaustive), and read or write MSR — a self-describing record format meant to supersede Pentasol.",
        "play": "▶ Play in your browser",
        "nav_docs": "Documentation",
        "nav_spec": "MSR spec",
        "nav_source": "Source (GitHub)",
        "what_h": "What is Morpion Solitaire?",
        "what_p": "Morpion Solitaire is a single-player pencil-and-paper game played on a grid of dots. Starting from a cross-shaped pattern of 36 dots, you repeatedly place one new dot and draw a straight line through five consecutive dots — horizontal, vertical, or diagonal. Every move must create a brand-new line; the game ends when no legal move remains. The aim is simply to make as many moves as possible.",
        "variants_h": "The four standard variants",
        "variants_p": "Variants differ by line length (5 or 4 dots) and by whether two parallel lines may share a point: Touching (T) allows it, Disjoint (D) does not.",
        "th_variant": "Variant",
        "th_line": "Line",
        "th_overlap": "Overlap",
        "th_best": "Best known",
        "line5": "5 dots",
        "line4": "4 dots",
        "touching": "Touching",
        "disjoint": "Disjoint",
        "best_5t": "178 (world record)",
        "best_5d": "82",
        "best_4t": "62 (proven optimal)",
        "best_4d": "35 (proven optimal)",
        "features_h": "What you can do here",
        "feat1": "Play all four variants directly in the browser — no install, multi-threaded WebAssembly.",
        "feat2": "Hunt records with several search engines: NRPA, large-neighbourhood perturbation, and exhaustive search.",
        "feat3": "Read, write, import, and export the self-describing MSR record format (a successor to Pentasol).",
        "feat4": "Free and open source — the app is GPL-3.0-or-later; the msr format library is MIT OR Apache-2.0.",
        "faq_h": "Frequently asked questions",
        "q1": "Can I play Morpion Solitaire online?",
        "a1": "Yes. You can play all four standard variants directly in your browser, with nothing to install.",
        "q2": "What is the Morpion Solitaire world record?",
        "a2": "The best known 5T score is 178 moves. The 4T (62) and 4D (35) variants have been proven optimal.",
        "q3": "Is it free?",
        "a3": "Yes. It is free and open source. The app is licensed GPL-3.0-or-later and the MSR format library is MIT OR Apache-2.0.",
        "q4": "What is the MSR format?",
        "a4": "MSR is a self-describing record format for Morpion Solitaire games, designed to supersede Pentasol. Its specification is published on this site.",
        "footer": "The 5T world record is 178; 4T (62) and 4D (35) are solved. The web build runs multi-threaded WebAssembly. Free software — the app is GPL-3.0-or-later, the msr format library is MIT OR Apache-2.0.",
        "langs_label": "Language",
        "redirect": "Redirecting…",
    },
    "fr": {
        "title": "Morpion Solitaire — jouer en ligne & battre des records",
        "desc": "Un joueur et solveur de Morpion Solitaire rapide : jouez aux quatre variantes dans votre navigateur, chassez les records, et lisez/écrivez le format de partie auto-descriptif MSR.",
        "og_title": "Morpion Solitaire — joueur & solveur",
        "og_desc": "Jouez aux quatre variantes standard dans votre navigateur, chassez les records (record du monde 5T : 178), et lisez/écrivez le format auto-descriptif MSR.",
        "tagline": "Un joueur et solveur rapide. Jouez aux quatre variantes standard dans votre navigateur, chassez les records avec plusieurs moteurs de recherche (NRPA, perturbation de grand voisinage, exhaustif), et lisez ou écrivez MSR — un format de partie auto-descriptif destiné à remplacer Pentasol.",
        "play": "▶ Jouer dans le navigateur",
        "nav_docs": "Documentation",
        "nav_spec": "Spéc. MSR",
        "nav_source": "Source (GitHub)",
        "what_h": "Qu'est-ce que le Morpion Solitaire ?",
        "what_p": "Le Morpion Solitaire est un jeu de papier-crayon en solitaire joué sur une grille de points. À partir d'une croix de 36 points, vous placez à chaque tour un nouveau point et tracez une ligne droite passant par cinq points consécutifs — horizontale, verticale ou diagonale. Chaque coup doit créer une ligne inédite ; la partie s'arrête quand plus aucun coup n'est possible. Le but est simplement de jouer le plus de coups possible.",
        "variants_h": "Les quatre variantes standard",
        "variants_p": "Les variantes diffèrent par la longueur des lignes (5 ou 4 points) et par le fait que deux lignes parallèles peuvent partager un point : Avec contact (T) l'autorise, Sans contact (D) l'interdit.",
        "th_variant": "Variante",
        "th_line": "Ligne",
        "th_overlap": "Contact",
        "th_best": "Meilleur connu",
        "line5": "5 points",
        "line4": "4 points",
        "touching": "Avec contact",
        "disjoint": "Sans contact",
        "best_5t": "178 (record du monde)",
        "best_5d": "82",
        "best_4t": "62 (optimum prouvé)",
        "best_4d": "35 (optimum prouvé)",
        "features_h": "Ce que vous pouvez faire ici",
        "feat1": "Jouer aux quatre variantes directement dans le navigateur — sans installation, en WebAssembly multi-thread.",
        "feat2": "Chasser les records avec plusieurs moteurs : NRPA, perturbation de grand voisinage et recherche exhaustive.",
        "feat3": "Lire, écrire, importer et exporter le format auto-descriptif MSR (un successeur de Pentasol).",
        "feat4": "Libre et open source — l'application est en GPL-3.0-or-later ; la bibliothèque du format msr est en MIT OR Apache-2.0.",
        "faq_h": "Questions fréquentes",
        "q1": "Puis-je jouer au Morpion Solitaire en ligne ?",
        "a1": "Oui. Vous pouvez jouer aux quatre variantes standard directement dans votre navigateur, sans rien installer.",
        "q2": "Quel est le record du monde du Morpion Solitaire ?",
        "a2": "Le meilleur score connu en 5T est de 178 coups. Les variantes 4T (62) et 4D (35) ont été prouvées optimales.",
        "q3": "Est-ce gratuit ?",
        "a3": "Oui. C'est gratuit et open source. L'application est sous licence GPL-3.0-or-later et la bibliothèque du format MSR sous MIT OR Apache-2.0.",
        "q4": "Qu'est-ce que le format MSR ?",
        "a4": "MSR est un format de partie auto-descriptif pour le Morpion Solitaire, conçu pour remplacer Pentasol. Sa spécification est publiée sur ce site.",
        "footer": "Le record du monde 5T est de 178 ; 4T (62) et 4D (35) sont résolus. La version web tourne en WebAssembly multi-thread. Logiciel libre — l'application est en GPL-3.0-or-later, la bibliothèque du format msr en MIT OR Apache-2.0.",
        "langs_label": "Langue",
        "redirect": "Redirection…",
    },
    "de": {
        "title": "Morpion Solitaire — online spielen & Rekorde lösen",
        "desc": "Ein schneller Morpion-Solitaire-Spieler und -Löser: Spiele die vier Varianten im Browser, jage Rekorde und lies/schreibe das selbstbeschreibende MSR-Aufzeichnungsformat.",
        "og_title": "Morpion Solitaire — Spieler & Löser",
        "og_desc": "Spiele die vier Standardvarianten im Browser, jage Rekorde (5T-Weltrekord: 178) und lies/schreibe das selbstbeschreibende MSR-Format.",
        "tagline": "Ein schneller Spieler und Löser. Spiele die vier Standardvarianten im Browser, jage Rekorde mit mehreren Suchmaschinen (NRPA, Large-Neighbourhood-Perturbation, erschöpfend) und lies oder schreibe MSR — ein selbstbeschreibendes Aufzeichnungsformat, das Pentasol ablösen soll.",
        "play": "▶ Im Browser spielen",
        "nav_docs": "Dokumentation",
        "nav_spec": "MSR-Spezifikation",
        "nav_source": "Quellcode (GitHub)",
        "what_h": "Was ist Morpion Solitaire?",
        "what_p": "Morpion Solitaire ist ein Solitärspiel mit Papier und Bleistift, gespielt auf einem Punkteraster. Ausgehend von einem Kreuz aus 36 Punkten setzt du wiederholt einen neuen Punkt und zeichnest eine gerade Linie durch fünf aufeinanderfolgende Punkte — waagerecht, senkrecht oder diagonal. Jeder Zug muss eine neue Linie erzeugen; das Spiel endet, wenn kein Zug mehr möglich ist. Ziel ist es, möglichst viele Züge zu machen.",
        "variants_h": "Die vier Standardvarianten",
        "variants_p": "Die Varianten unterscheiden sich durch die Linienlänge (5 oder 4 Punkte) und ob sich zwei parallele Linien einen Punkt teilen dürfen: Mit Berührung (T) erlaubt es, Ohne Berührung (D) nicht.",
        "th_variant": "Variante",
        "th_line": "Linie",
        "th_overlap": "Berührung",
        "th_best": "Bestwert",
        "line5": "5 Punkte",
        "line4": "4 Punkte",
        "touching": "Mit Berührung",
        "disjoint": "Ohne Berührung",
        "best_5t": "178 (Weltrekord)",
        "best_5d": "82",
        "best_4t": "62 (bewiesen optimal)",
        "best_4d": "35 (bewiesen optimal)",
        "features_h": "Was du hier tun kannst",
        "feat1": "Alle vier Varianten direkt im Browser spielen — ohne Installation, mehrthreadiges WebAssembly.",
        "feat2": "Rekorde mit mehreren Suchmaschinen jagen: NRPA, Large-Neighbourhood-Perturbation und erschöpfende Suche.",
        "feat3": "Das selbstbeschreibende MSR-Format lesen, schreiben, importieren und exportieren (ein Nachfolger von Pentasol).",
        "feat4": "Frei und quelloffen — die App steht unter GPL-3.0-or-later; die msr-Formatbibliothek unter MIT OR Apache-2.0.",
        "faq_h": "Häufige Fragen",
        "q1": "Kann ich Morpion Solitaire online spielen?",
        "a1": "Ja. Du kannst alle vier Standardvarianten direkt im Browser spielen, ohne etwas zu installieren.",
        "q2": "Was ist der Weltrekord bei Morpion Solitaire?",
        "a2": "Der beste bekannte 5T-Wert sind 178 Züge. Die Varianten 4T (62) und 4D (35) wurden als optimal bewiesen.",
        "q3": "Ist es kostenlos?",
        "a3": "Ja. Es ist kostenlos und quelloffen. Die App steht unter GPL-3.0-or-later und die MSR-Formatbibliothek unter MIT OR Apache-2.0.",
        "q4": "Was ist das MSR-Format?",
        "a4": "MSR ist ein selbstbeschreibendes Aufzeichnungsformat für Morpion-Solitaire-Partien, das Pentasol ablösen soll. Seine Spezifikation ist auf dieser Website veröffentlicht.",
        "footer": "Der 5T-Weltrekord liegt bei 178; 4T (62) und 4D (35) sind gelöst. Die Web-Version läuft als mehrthreadiges WebAssembly. Freie Software — die App unter GPL-3.0-or-later, die msr-Formatbibliothek unter MIT OR Apache-2.0.",
        "langs_label": "Sprache",
        "redirect": "Weiterleitung…",
    },
    "es": {
        "title": "Morpion Solitaire — juega en línea y resuelve récords",
        "desc": "Un jugador y solucionador rápido de Morpion Solitaire: juega las cuatro variantes en tu navegador, persigue récords y lee/escribe el formato de partida autodescriptivo MSR.",
        "og_title": "Morpion Solitaire — jugador y solucionador",
        "og_desc": "Juega las cuatro variantes estándar en tu navegador, persigue récords (récord mundial 5T: 178) y lee/escribe el formato autodescriptivo MSR.",
        "tagline": "Un jugador y solucionador rápido. Juega las cuatro variantes estándar en tu navegador, persigue récords con varios motores de búsqueda (NRPA, perturbación de gran vecindario, exhaustivo) y lee o escribe MSR — un formato de partida autodescriptivo pensado para reemplazar a Pentasol.",
        "play": "▶ Jugar en el navegador",
        "nav_docs": "Documentación",
        "nav_spec": "Especificación MSR",
        "nav_source": "Código (GitHub)",
        "what_h": "¿Qué es el Morpion Solitaire?",
        "what_p": "El Morpion Solitaire es un juego de lápiz y papel para un jugador sobre una cuadrícula de puntos. Partiendo de una cruz de 36 puntos, colocas un nuevo punto por turno y trazas una línea recta que pasa por cinco puntos consecutivos — horizontal, vertical o diagonal. Cada jugada debe crear una línea nueva; la partida termina cuando no queda ninguna jugada legal. El objetivo es simplemente hacer tantas jugadas como sea posible.",
        "variants_h": "Las cuatro variantes estándar",
        "variants_p": "Las variantes difieren en la longitud de la línea (5 o 4 puntos) y en si dos líneas paralelas pueden compartir un punto: Con contacto (T) lo permite, Sin contacto (D) no.",
        "th_variant": "Variante",
        "th_line": "Línea",
        "th_overlap": "Contacto",
        "th_best": "Mejor conocido",
        "line5": "5 puntos",
        "line4": "4 puntos",
        "touching": "Con contacto",
        "disjoint": "Sin contacto",
        "best_5t": "178 (récord mundial)",
        "best_5d": "82",
        "best_4t": "62 (óptimo demostrado)",
        "best_4d": "35 (óptimo demostrado)",
        "features_h": "Lo que puedes hacer aquí",
        "feat1": "Jugar las cuatro variantes directamente en el navegador — sin instalación, WebAssembly multihilo.",
        "feat2": "Perseguir récords con varios motores: NRPA, perturbación de gran vecindario y búsqueda exhaustiva.",
        "feat3": "Leer, escribir, importar y exportar el formato autodescriptivo MSR (un sucesor de Pentasol).",
        "feat4": "Libre y de código abierto — la aplicación es GPL-3.0-or-later; la biblioteca del formato msr es MIT OR Apache-2.0.",
        "faq_h": "Preguntas frecuentes",
        "q1": "¿Puedo jugar al Morpion Solitaire en línea?",
        "a1": "Sí. Puedes jugar las cuatro variantes estándar directamente en tu navegador, sin instalar nada.",
        "q2": "¿Cuál es el récord mundial de Morpion Solitaire?",
        "a2": "La mejor puntuación conocida en 5T es de 178 jugadas. Las variantes 4T (62) y 4D (35) se han demostrado óptimas.",
        "q3": "¿Es gratis?",
        "a3": "Sí. Es gratis y de código abierto. La aplicación está bajo GPL-3.0-or-later y la biblioteca del formato MSR bajo MIT OR Apache-2.0.",
        "q4": "¿Qué es el formato MSR?",
        "a4": "MSR es un formato de partida autodescriptivo para el Morpion Solitaire, diseñado para reemplazar a Pentasol. Su especificación está publicada en este sitio.",
        "footer": "El récord mundial 5T es 178; 4T (62) y 4D (35) están resueltos. La versión web funciona con WebAssembly multihilo. Software libre — la aplicación es GPL-3.0-or-later, la biblioteca del formato msr es MIT OR Apache-2.0.",
        "langs_label": "Idioma",
        "redirect": "Redirigiendo…",
    },
    "it": {
        "title": "Morpion Solitaire — gioca online e risolvi i record",
        "desc": "Un giocatore e risolutore veloce di Morpion Solitaire: gioca le quattro varianti nel browser, insegui i record e leggi/scrivi il formato di partita autodescrittivo MSR.",
        "og_title": "Morpion Solitaire — giocatore e risolutore",
        "og_desc": "Gioca le quattro varianti standard nel browser, insegui i record (record mondiale 5T: 178) e leggi/scrivi il formato autodescrittivo MSR.",
        "tagline": "Un giocatore e risolutore veloce. Gioca le quattro varianti standard nel browser, insegui i record con diversi motori di ricerca (NRPA, perturbazione di grande vicinato, esaustivo) e leggi o scrivi MSR — un formato di partita autodescrittivo pensato per sostituire Pentasol.",
        "play": "▶ Gioca nel browser",
        "nav_docs": "Documentazione",
        "nav_spec": "Specifica MSR",
        "nav_source": "Sorgente (GitHub)",
        "what_h": "Che cos'è il Morpion Solitaire?",
        "what_p": "Il Morpion Solitaire è un gioco carta e matita per un solo giocatore su una griglia di punti. Partendo da una croce di 36 punti, a ogni turno piazzi un nuovo punto e tracci una linea retta che passa per cinque punti consecutivi — orizzontale, verticale o diagonale. Ogni mossa deve creare una linea nuova; la partita finisce quando non resta alcuna mossa valida. Lo scopo è semplicemente fare il maggior numero di mosse possibile.",
        "variants_h": "Le quattro varianti standard",
        "variants_p": "Le varianti differiscono per la lunghezza della linea (5 o 4 punti) e per il fatto che due linee parallele possano condividere un punto: Con contatto (T) lo consente, Senza contatto (D) no.",
        "th_variant": "Variante",
        "th_line": "Linea",
        "th_overlap": "Contatto",
        "th_best": "Miglior noto",
        "line5": "5 punti",
        "line4": "4 punti",
        "touching": "Con contatto",
        "disjoint": "Senza contatto",
        "best_5t": "178 (record mondiale)",
        "best_5d": "82",
        "best_4t": "62 (ottimo dimostrato)",
        "best_4d": "35 (ottimo dimostrato)",
        "features_h": "Cosa puoi fare qui",
        "feat1": "Giocare tutte e quattro le varianti direttamente nel browser — nessuna installazione, WebAssembly multi-thread.",
        "feat2": "Inseguire i record con diversi motori: NRPA, perturbazione di grande vicinato e ricerca esaustiva.",
        "feat3": "Leggere, scrivere, importare ed esportare il formato autodescrittivo MSR (un successore di Pentasol).",
        "feat4": "Libero e open source — l'app è GPL-3.0-or-later; la libreria del formato msr è MIT OR Apache-2.0.",
        "faq_h": "Domande frequenti",
        "q1": "Posso giocare al Morpion Solitaire online?",
        "a1": "Sì. Puoi giocare tutte e quattro le varianti standard direttamente nel browser, senza installare nulla.",
        "q2": "Qual è il record mondiale di Morpion Solitaire?",
        "a2": "Il miglior punteggio noto in 5T è di 178 mosse. Le varianti 4T (62) e 4D (35) sono state dimostrate ottimali.",
        "q3": "È gratis?",
        "a3": "Sì. È gratuito e open source. L'app è sotto licenza GPL-3.0-or-later e la libreria del formato MSR sotto MIT OR Apache-2.0.",
        "q4": "Che cos'è il formato MSR?",
        "a4": "MSR è un formato di partita autodescrittivo per il Morpion Solitaire, progettato per sostituire Pentasol. La sua specifica è pubblicata su questo sito.",
        "footer": "Il record mondiale 5T è 178; 4T (62) e 4D (35) sono risolti. La versione web gira come WebAssembly multi-thread. Software libero — l'app è GPL-3.0-or-later, la libreria del formato msr è MIT OR Apache-2.0.",
        "langs_label": "Lingua",
        "redirect": "Reindirizzamento…",
    },
    "ja": {
        "title": "Morpion Solitaire — オンラインでプレイ＆記録に挑戦",
        "desc": "高速な Morpion Solitaire のプレイヤー兼ソルバー。4 つのバリアントをブラウザで遊び、記録を狙い、自己記述型の MSR 記録フォーマットを読み書きできます。",
        "og_title": "Morpion Solitaire — プレイヤー＆ソルバー",
        "og_desc": "4 つの標準バリアントをブラウザでプレイし、記録（5T 世界記録：178）に挑戦し、自己記述型の MSR フォーマットを読み書きできます。",
        "tagline": "高速なプレイヤー兼ソルバー。4 つの標準バリアントをブラウザで遊び、複数の探索エンジン（NRPA、大近傍摂動、全探索）で記録を狙い、MSR を読み書きできます。MSR は Pentasol に代わることを目指した自己記述型の記録フォーマットです。",
        "play": "▶ ブラウザでプレイ",
        "nav_docs": "ドキュメント",
        "nav_spec": "MSR 仕様",
        "nav_source": "ソース (GitHub)",
        "what_h": "Morpion Solitaire とは？",
        "what_p": "Morpion Solitaire は、点の格子の上で遊ぶ一人用の紙とペンのゲームです。36 個の点でできた十字形から始め、毎手 1 つ新しい点を置き、連続する 5 つの点を通る直線（横・縦・斜め）を引きます。各手は必ず新しい線を作らなければならず、合法手がなくなるとゲーム終了です。目的は、できるだけ多くの手を打つことです。",
        "variants_h": "4 つの標準バリアント",
        "variants_p": "バリアントは線の長さ（5 点または 4 点）と、平行な 2 本の線が点を共有できるかどうかで異なります。接触あり (T) は許可し、接触なし (D) は禁止します。",
        "th_variant": "バリアント",
        "th_line": "線",
        "th_overlap": "接触",
        "th_best": "既知の最高",
        "line5": "5 点",
        "line4": "4 点",
        "touching": "接触あり",
        "disjoint": "接触なし",
        "best_5t": "178（世界記録）",
        "best_5d": "82",
        "best_4t": "62（最適性証明済み）",
        "best_4d": "35（最適性証明済み）",
        "features_h": "ここでできること",
        "feat1": "4 つのバリアントすべてをブラウザで直接プレイ — インストール不要、マルチスレッド WebAssembly。",
        "feat2": "複数の探索エンジンで記録に挑戦：NRPA、大近傍摂動、全探索。",
        "feat3": "自己記述型の MSR 記録フォーマット（Pentasol の後継）を読み・書き・取り込み・書き出し。",
        "feat4": "フリーかつオープンソース — アプリは GPL-3.0-or-later、msr フォーマットライブラリは MIT OR Apache-2.0。",
        "faq_h": "よくある質問",
        "q1": "Morpion Solitaire をオンラインで遊べますか？",
        "a1": "はい。4 つの標準バリアントすべてを、インストール不要でブラウザから直接プレイできます。",
        "q2": "Morpion Solitaire の世界記録は？",
        "a2": "5T の既知の最高スコアは 178 手です。4T (62) と 4D (35) のバリアントは最適であることが証明されています。",
        "q3": "無料ですか？",
        "a3": "はい。無料かつオープンソースです。アプリは GPL-3.0-or-later、MSR フォーマットライブラリは MIT OR Apache-2.0 のライセンスです。",
        "q4": "MSR フォーマットとは？",
        "a4": "MSR は Morpion Solitaire の対局向けの自己記述型記録フォーマットで、Pentasol に代わることを目指しています。その仕様はこのサイトで公開されています。",
        "footer": "5T 世界記録は 178、4T (62) と 4D (35) は解決済みです。Web 版はマルチスレッド WebAssembly で動作します。フリーソフトウェア — アプリは GPL-3.0-or-later、msr フォーマットライブラリは MIT OR Apache-2.0。",
        "langs_label": "言語",
        "redirect": "リダイレクト中…",
    },
    "nl": {
        "title": "Morpion Solitaire — online spelen & records oplossen",
        "desc": "Een snelle Morpion Solitaire-speler en -oplosser: speel de vier varianten in je browser, jaag op records en lees/schrijf het zelfbeschrijvende MSR-recordformaat.",
        "og_title": "Morpion Solitaire — speler & oplosser",
        "og_desc": "Speel de vier standaardvarianten in je browser, jaag op records (5T-wereldrecord: 178) en lees/schrijf het zelfbeschrijvende MSR-formaat.",
        "tagline": "Een snelle speler en oplosser. Speel de vier standaardvarianten in je browser, jaag op records met meerdere zoekmachines (NRPA, large-neighbourhood-perturbatie, uitputtend) en lees of schrijf MSR — een zelfbeschrijvend recordformaat bedoeld als opvolger van Pentasol.",
        "play": "▶ Speel in je browser",
        "nav_docs": "Documentatie",
        "nav_spec": "MSR-specificatie",
        "nav_source": "Broncode (GitHub)",
        "what_h": "Wat is Morpion Solitaire?",
        "what_p": "Morpion Solitaire is een potlood-en-papierspel voor één speler op een raster van punten. Vanuit een kruis van 36 punten plaats je telkens één nieuw punt en trek je een rechte lijn door vijf opeenvolgende punten — horizontaal, verticaal of diagonaal. Elke zet moet een nieuwe lijn maken; het spel eindigt als er geen geldige zet meer is. Het doel is simpelweg zoveel mogelijk zetten te doen.",
        "variants_h": "De vier standaardvarianten",
        "variants_p": "Varianten verschillen in lijnlengte (5 of 4 punten) en of twee parallelle lijnen een punt mogen delen: Met contact (T) staat het toe, Zonder contact (D) niet.",
        "th_variant": "Variant",
        "th_line": "Lijn",
        "th_overlap": "Contact",
        "th_best": "Beste bekend",
        "line5": "5 punten",
        "line4": "4 punten",
        "touching": "Met contact",
        "disjoint": "Zonder contact",
        "best_5t": "178 (wereldrecord)",
        "best_5d": "82",
        "best_4t": "62 (bewezen optimaal)",
        "best_4d": "35 (bewezen optimaal)",
        "features_h": "Wat je hier kunt doen",
        "feat1": "Alle vier varianten direct in de browser spelen — geen installatie, multithreaded WebAssembly.",
        "feat2": "Op records jagen met meerdere zoekmachines: NRPA, large-neighbourhood-perturbatie en uitputtend zoeken.",
        "feat3": "Het zelfbeschrijvende MSR-formaat lezen, schrijven, importeren en exporteren (een opvolger van Pentasol).",
        "feat4": "Vrij en open source — de app is GPL-3.0-or-later; de msr-formaatbibliotheek is MIT OR Apache-2.0.",
        "faq_h": "Veelgestelde vragen",
        "q1": "Kan ik Morpion Solitaire online spelen?",
        "a1": "Ja. Je kunt alle vier standaardvarianten direct in je browser spelen, zonder iets te installeren.",
        "q2": "Wat is het wereldrecord van Morpion Solitaire?",
        "a2": "De beste bekende 5T-score is 178 zetten. De varianten 4T (62) en 4D (35) zijn bewezen optimaal.",
        "q3": "Is het gratis?",
        "a3": "Ja. Het is gratis en open source. De app valt onder GPL-3.0-or-later en de MSR-formaatbibliotheek onder MIT OR Apache-2.0.",
        "q4": "Wat is het MSR-formaat?",
        "a4": "MSR is een zelfbeschrijvend recordformaat voor Morpion Solitaire-partijen, ontworpen als opvolger van Pentasol. De specificatie is op deze site gepubliceerd.",
        "footer": "Het 5T-wereldrecord is 178; 4T (62) en 4D (35) zijn opgelost. De webversie draait als multithreaded WebAssembly. Vrije software — de app is GPL-3.0-or-later, de msr-formaatbibliotheek is MIT OR Apache-2.0.",
        "langs_label": "Taal",
        "redirect": "Doorverwijzen…",
    },
    "pt": {
        "title": "Morpion Solitaire — jogue online e resolva recordes",
        "desc": "Um jogador e solucionador rápido de Morpion Solitaire: jogue as quatro variantes no navegador, persiga recordes e leia/escreva o formato de partida autodescritivo MSR.",
        "og_title": "Morpion Solitaire — jogador e solucionador",
        "og_desc": "Jogue as quatro variantes padrão no navegador, persiga recordes (recorde mundial 5T: 178) e leia/escreva o formato autodescritivo MSR.",
        "tagline": "Um jogador e solucionador rápido. Jogue as quatro variantes padrão no navegador, persiga recordes com vários motores de busca (NRPA, perturbação de grande vizinhança, exaustivo) e leia ou escreva MSR — um formato de partida autodescritivo pensado para substituir o Pentasol.",
        "play": "▶ Jogar no navegador",
        "nav_docs": "Documentação",
        "nav_spec": "Especificação MSR",
        "nav_source": "Código (GitHub)",
        "what_h": "O que é o Morpion Solitaire?",
        "what_p": "O Morpion Solitaire é um jogo de papel e lápis para um jogador, jogado numa grelha de pontos. Partindo de uma cruz de 36 pontos, a cada jogada coloca um novo ponto e traça uma linha reta passando por cinco pontos consecutivos — horizontal, vertical ou diagonal. Cada jogada deve criar uma linha nova; o jogo termina quando não resta nenhuma jogada válida. O objetivo é simplesmente fazer o maior número de jogadas possível.",
        "variants_h": "As quatro variantes padrão",
        "variants_p": "As variantes diferem no comprimento da linha (5 ou 4 pontos) e em se duas linhas paralelas podem partilhar um ponto: Com contacto (T) permite, Sem contacto (D) não.",
        "th_variant": "Variante",
        "th_line": "Linha",
        "th_overlap": "Contacto",
        "th_best": "Melhor conhecido",
        "line5": "5 pontos",
        "line4": "4 pontos",
        "touching": "Com contacto",
        "disjoint": "Sem contacto",
        "best_5t": "178 (recorde mundial)",
        "best_5d": "82",
        "best_4t": "62 (ótimo comprovado)",
        "best_4d": "35 (ótimo comprovado)",
        "features_h": "O que pode fazer aqui",
        "feat1": "Jogar as quatro variantes diretamente no navegador — sem instalação, WebAssembly multi-thread.",
        "feat2": "Perseguir recordes com vários motores: NRPA, perturbação de grande vizinhança e busca exaustiva.",
        "feat3": "Ler, escrever, importar e exportar o formato autodescritivo MSR (um sucessor do Pentasol).",
        "feat4": "Livre e de código aberto — a aplicação é GPL-3.0-or-later; a biblioteca do formato msr é MIT OR Apache-2.0.",
        "faq_h": "Perguntas frequentes",
        "q1": "Posso jogar Morpion Solitaire online?",
        "a1": "Sim. Pode jogar as quatro variantes padrão diretamente no navegador, sem instalar nada.",
        "q2": "Qual é o recorde mundial de Morpion Solitaire?",
        "a2": "A melhor pontuação conhecida em 5T é de 178 jogadas. As variantes 4T (62) e 4D (35) foram comprovadas ótimas.",
        "q3": "É grátis?",
        "a3": "Sim. É grátis e de código aberto. A aplicação está sob GPL-3.0-or-later e a biblioteca do formato MSR sob MIT OR Apache-2.0.",
        "q4": "O que é o formato MSR?",
        "a4": "MSR é um formato de partida autodescritivo para o Morpion Solitaire, concebido para substituir o Pentasol. A sua especificação está publicada neste site.",
        "footer": "O recorde mundial 5T é 178; 4T (62) e 4D (35) estão resolvidos. A versão web corre como WebAssembly multi-thread. Software livre — a aplicação é GPL-3.0-or-later, a biblioteca do formato msr é MIT OR Apache-2.0.",
        "langs_label": "Idioma",
        "redirect": "A redirecionar…",
    },
}

CSS = """
    :root { color-scheme: dark; }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      background: radial-gradient(1200px 800px at 50% -10%, #25264a, #14142a 60%) no-repeat #14142a;
      color: #e6e6e9;
      font: 16px/1.6 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
    }
    a { color: #9bd; }
    .hero {
      min-height: 100vh; display: flex; flex-direction: column;
      align-items: center; justify-content: center; gap: 1.5rem;
      padding: 2rem; text-align: center;
    }
    h1 { font-size: clamp(2.2rem, 6vw, 3.6rem); margin: 0; letter-spacing: .5px; }
    .tagline { max-width: 42rem; color: #b9b9c6; margin: 0; }
    .play {
      display: inline-block; margin-top: .5rem; padding: .85rem 2.4rem;
      font-size: 1.15rem; font-weight: 600; text-decoration: none;
      color: #14142a; background: #8ad; border-radius: 999px;
      box-shadow: 0 6px 24px rgba(136,170,221,.35); transition: transform .1s ease;
    }
    .play:hover { transform: translateY(-2px); }
    .version { margin: -.5rem 0 0; color: #7c7c8c; font-size: .85rem; }
    nav { display: flex; flex-wrap: wrap; gap: 1.25rem; justify-content: center; }
    nav a { text-decoration: none; border-bottom: 1px solid transparent; }
    nav a:hover { border-bottom-color: #9bd; }
    main { max-width: 52rem; margin: 0 auto; padding: 0 1.5rem; }
    section { margin: 3.5rem 0; }
    h2 { font-size: 1.5rem; margin: 0 0 .8rem; color: #cdd6f4; }
    p { color: #c8c8d2; }
    .variants { width: 100%; border-collapse: collapse; margin-top: .5rem; }
    .variants th, .variants td {
      text-align: left; padding: .55rem .6rem; border-bottom: 1px solid #2a2b3c;
    }
    .variants th { color: #9aa0b4; font-weight: 600; }
    .variants td:first-child { font-weight: 600; color: #e6e6e9; white-space: nowrap; }
    .features { list-style: none; padding: 0; margin: 0; display: grid; gap: .6rem; }
    .features li { padding-left: 1.4rem; position: relative; color: #c8c8d2; }
    .features li::before { content: "▸"; position: absolute; left: 0; color: #8ad; }
    .faq dt { font-weight: 600; margin-top: 1.3rem; color: #e6e6e9; }
    .faq dd { margin: .35rem 0 0; color: #b9b9c6; }
    code { background: #2a2b3c; padding: .1em .4em; border-radius: 4px; }
    footer {
      max-width: 52rem; margin: 0 auto; padding: 2rem 1.5rem 1rem;
      color: #7c7c8c; font-size: .9rem; border-top: 1px solid #2a2b3c;
    }
    .langs { max-width: 52rem; margin: 0 auto; padding: .5rem 1.5rem 3rem; }
    .langs a { color: #9aa0b4; text-decoration: none; margin-right: .9rem; font-size: .9rem; }
    .langs a[aria-current="true"] { color: #e6e6e9; font-weight: 600; }
"""


def e(s):
    return html.escape(s, quote=True)


def hreflang_links():
    out = [
        '<link rel="alternate" hreflang="{l}" href="{site}/{l}/"/>'.format(l=l, site=SITE)
        for l in LANGS
    ]
    out.append('<link rel="alternate" hreflang="x-default" href="{site}/en/"/>'.format(site=SITE))
    return "\n  ".join(out)


def lang_switcher(cur):
    items = []
    for l in LANGS:
        attr = ' aria-current="true"' if l == cur else ""
        items.append(
            '<a href="{site}/{l}/" lang="{l}"{a}>{name}</a>'.format(
                site=SITE, l=l, a=attr, name=e(LANG_NAMES[l])
            )
        )
    return "".join(items)


def jsonld(lang, t):
    url = "{site}/{l}/".format(site=SITE, l=lang)
    graph = [
        {
            "@type": "WebSite",
            "@id": url + "#website",
            "url": url,
            "name": "Morpion Solitaire",
            "description": t["desc"],
            "inLanguage": lang,
        },
        {
            "@type": "WebApplication",
            "@id": SITE + "/play/#app",
            "name": t["og_title"],
            "url": SITE + "/play/",
            "applicationCategory": "GameApplication",
            "operatingSystem": "Web browser",
            "browserRequirements": "Requires JavaScript and WebAssembly",
            "description": t["og_desc"],
            "image": SITE + "/og-image.png",
            "inLanguage": lang,
            "isAccessibleForFree": True,
            "license": "https://www.gnu.org/licenses/gpl-3.0.html",
            "author": {"@type": "Person", "name": "Stéphane Jourdois"},
            "offers": {"@type": "Offer", "price": "0", "priceCurrency": "USD"},
        },
        {
            "@type": "FAQPage",
            "@id": url + "#faq",
            "mainEntity": [
                {
                    "@type": "Question",
                    "name": t["q%d" % i],
                    "acceptedAnswer": {"@type": "Answer", "text": t["a%d" % i]},
                }
                for i in (1, 2, 3, 4)
            ],
        },
    ]
    return json.dumps(
        {"@context": "https://schema.org", "@graph": graph},
        ensure_ascii=False,
        indent=2,
    )


def page(lang, version):
    t = TEXT[lang]
    url = "{site}/{l}/".format(site=SITE, l=lang)
    return """<!DOCTYPE html>
<html lang="{lang}">
<head>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1.0"/>
  <title>{title}</title>
  <meta name="description" content="{desc}"/>
  <link rel="canonical" href="{url}"/>
  <link rel="icon" href="/favicon.svg" type="image/svg+xml"/>
  {hreflang}

  <meta property="og:type" content="website"/>
  <meta property="og:site_name" content="Morpion Solitaire"/>
  <meta property="og:locale" content="{lang}"/>
  <meta property="og:title" content="{og_title}"/>
  <meta property="og:description" content="{og_desc}"/>
  <meta property="og:url" content="{url}"/>
  <meta property="og:image" content="{site}/og-image.png"/>
  <meta property="og:image:width" content="1200"/>
  <meta property="og:image:height" content="630"/>
  <meta name="twitter:card" content="summary_large_image"/>
  <style>{css}</style>

  <!-- Privacy-friendly analytics (no cookies, no consent banner) -->
  <script data-goatcounter="https://kwisatz.goatcounter.com/count"
          async src="//gc.zgo.at/count.js"></script>
</head>
<body>
  <header class="hero">
    <h1>Morpion&nbsp;Solitaire</h1>
    <p class="tagline">{tagline}</p>
    <a class="play" href="/play/">{play}</a>
    <p class="version">v{version}</p>
    <nav>
      <a href="/docs/">{nav_docs}</a>
      <a href="/docs/format/spec.html">{nav_spec}</a>
      <a href="https://github.com/sjourdois/morpion-solitaire">{nav_source}</a>
      <a href="https://crates.io/crates/morpion-solitaire">crates.io</a>
    </nav>
  </header>

  <main>
    <section>
      <h2>{what_h}</h2>
      <p>{what_p}</p>
    </section>

    <section>
      <h2>{variants_h}</h2>
      <p>{variants_p}</p>
      <table class="variants">
        <thead>
          <tr><th>{th_variant}</th><th>{th_line}</th><th>{th_overlap}</th><th>{th_best}</th></tr>
        </thead>
        <tbody>
          <tr><td>5T</td><td>{line5}</td><td>{touching}</td><td>{best_5t}</td></tr>
          <tr><td>5D</td><td>{line5}</td><td>{disjoint}</td><td>{best_5d}</td></tr>
          <tr><td>4T</td><td>{line4}</td><td>{touching}</td><td>{best_4t}</td></tr>
          <tr><td>4D</td><td>{line4}</td><td>{disjoint}</td><td>{best_4d}</td></tr>
        </tbody>
      </table>
    </section>

    <section>
      <h2>{features_h}</h2>
      <ul class="features">
        <li>{feat1}</li>
        <li>{feat2}</li>
        <li>{feat3}</li>
        <li>{feat4}</li>
      </ul>
    </section>

    <section>
      <h2>{faq_h}</h2>
      <dl class="faq">
        <dt>{q1}</dt><dd>{a1}</dd>
        <dt>{q2}</dt><dd>{a2}</dd>
        <dt>{q3}</dt><dd>{a3}</dd>
        <dt>{q4}</dt><dd>{a4}</dd>
      </dl>
    </section>
  </main>

  <footer>{footer}</footer>
  <nav class="langs" aria-label="{langs_label}">{switcher}</nav>

  <script type="application/ld+json">
{jsonld}
  </script>
</body>
</html>
""".format(
        lang=lang,
        site=SITE,
        url=url,
        css=CSS,
        version=version,
        hreflang=hreflang_links(),
        switcher=lang_switcher(lang),
        jsonld=jsonld(lang, t),
        title=e(t["title"]),
        desc=e(t["desc"]),
        og_title=e(t["og_title"]),
        og_desc=e(t["og_desc"]),
        tagline=e(t["tagline"]),
        play=e(t["play"]),
        nav_docs=e(t["nav_docs"]),
        nav_spec=e(t["nav_spec"]),
        nav_source=e(t["nav_source"]),
        what_h=e(t["what_h"]),
        what_p=e(t["what_p"]),
        variants_h=e(t["variants_h"]),
        variants_p=e(t["variants_p"]),
        th_variant=e(t["th_variant"]),
        th_line=e(t["th_line"]),
        th_overlap=e(t["th_overlap"]),
        th_best=e(t["th_best"]),
        line5=e(t["line5"]),
        line4=e(t["line4"]),
        touching=e(t["touching"]),
        disjoint=e(t["disjoint"]),
        best_5t=e(t["best_5t"]),
        best_5d=e(t["best_5d"]),
        best_4t=e(t["best_4t"]),
        best_4d=e(t["best_4d"]),
        features_h=e(t["features_h"]),
        feat1=e(t["feat1"]),
        feat2=e(t["feat2"]),
        feat3=e(t["feat3"]),
        feat4=e(t["feat4"]),
        faq_h=e(t["faq_h"]),
        q1=e(t["q1"]), a1=e(t["a1"]),
        q2=e(t["q2"]), a2=e(t["a2"]),
        q3=e(t["q3"]), a3=e(t["a3"]),
        q4=e(t["q4"]), a4=e(t["a4"]),
        footer=e(t["footer"]),
        langs_label=e(t["langs_label"]),
    )


def root():
    """Root redirector: send the visitor to their language, fall back to /en/."""
    langs_js = json.dumps(LANGS)
    return """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <title>Morpion Solitaire</title>
  <link rel="canonical" href="{site}/en/"/>
  <meta name="robots" content="noindex"/>
  <script>
    var supported = {langs};
    var prefs = navigator.languages || [navigator.language || "en"];
    var target = "en";
    for (var i = 0; i < prefs.length; i++) {{
      var base = String(prefs[i]).toLowerCase().split("-")[0];
      if (supported.indexOf(base) !== -1) {{ target = base; break; }}
    }}
    location.replace("/" + target + "/");
  </script>
  <meta http-equiv="refresh" content="0; url=/en/"/>
</head>
<body>
  <p>Redirecting to <a href="/en/">Morpion Solitaire</a>…</p>
</body>
</html>
""".format(site=SITE, langs=langs_js)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True, help="output site root (e.g. _site)")
    ap.add_argument("--version", default="0.0.0-dev", help="app version to show")
    args = ap.parse_args()

    for lang in LANGS:
        d = os.path.join(args.out, lang)
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, "index.html"), "w", encoding="utf-8") as f:
            f.write(page(lang, args.version))

    with open(os.path.join(args.out, "index.html"), "w", encoding="utf-8") as f:
        f.write(root())

    print("landing: wrote {} language pages + root redirector to {}".format(len(LANGS), args.out))


if __name__ == "__main__":
    main()
