app-title = Morpion Solitaire
variant-label = バリアント
score-label = 手数
legal-moves-label = 着手可能
algo-label = アルゴリズム
nrpa-level-label = NRPA レベル
nrpa-level-hint = 3 = 高速（1分で約99）。4以上はより深く探索するが、数時間の実行でのみ効果がある
algo-nrpa = NRPA
algo-beam = ビームサーチ
algo-systematic = 全探索
algo-perturbation = 摂動
perturbation-hint = 読み込んだ対局を局所的に最適化します：最後の K 手を破棄し、終盤を再探索し、最良を保持する、を繰り返します。まず記録を読み込んで実行してください。
btn-start = 開始
btn-stop = 停止
btn-undo = 元に戻す
btn-redo = やり直し
btn-new = 新しい対局
btn-import = インポート
btn-rotate = 回転
btn-flip = 反転
btn-recenter = 中央に戻す
btn-arrows = 矢印
btn-numbers = 番号
btn-silence = 🔔 記録更新 — 消音
load-record = 記録を読み込む
nodes-explored-label = 探索ノード数
nodes-per-second-label = ノード/秒
wasm-rate-disclaimer = ブラウザ版：ネイティブは数倍高速（この値はネイティブと比較不可）
time-label = 時間
records-label = 記録
btn-load-best = 結果を読み込む
btn-dismiss-preview = 破棄
btn-checkpoint = 探索を保存
btn-resume-search = 探索を再開
language-label = 言語
btn-load = 読み込む
btn-cancel = キャンセル
import-hint = セーブを貼り付け（JSON または Pentasol）：
status-copied = 局面をクリップボードにコピーしました
status-imported = インポート: {$score} 手
status-import-error = インポートが無効です: {$error}
status-record-saved = 記録 {$score} を保存しました: {$path}
status-record-save-error = 記録の保存に失敗しました: {$error}
status-record-web = 記録 {$score} に到達
status-checkpoint = 探索を保存しました
status-resumed = 探索を再開しました
status-no-checkpoint = 保存された探索はありません
status-search-paused = ⏸ 探索を一時停止
status-search-resumed = ▶ 探索を再開
status-record-beaten = 🔔 記録更新: {$score} 手（5T 世界記録 = {$record}）！
status-overflow = ⚠ グリッドオーバーフロー {$grid}×{$grid}（{$score} 手で到達）— 探索を停止し、最良の対局を records/overflow/ に保存しました。グリッドを拡大するには board.rs の `Row` を広げてください。

# ── CLI 実行時メッセージ ───────────────────────────────────────────────────
btn-pause = 一時停止
btn-resume = 再開
start-point-label = 開始位置
start-empty = 空の十字
start-seeded = 空の十字（読み込んだ対局で初期化）
start-continue = 読み込んだ対局を続行
start-needs-game = 先に対局を読み込むかプレイしてください。
resume-saved = 保存
format-label = エクスポート形式
btn-copy = コピー
btn-export-file = ファイルに保存…
status-exported = 保存しました: { $path }
status-png-web = 画像のクリップボードはウェブでは利用できません。
start-terminal = 読み込んだ対局は終了しています — 探索する手がありません。
search-section = 自動探索
variant-tip = { $len }点の線・{ $mode }
touch-touching = 端点の共有を許可
touch-disjoint = 線は重ならない
game-section = 対局
btn-theme = テーマ切り替え
btn-shortcuts = キーボードショートカット
shortcuts-title = キーボードショートカット
searching-label = 探索中…
confirm-discard-title = 未保存の変更
confirm-discard-body = 現在の対局を保存しますか？
btn-save = 保存
btn-dont-save = 保存しない
rules-title = ルール
rules-hide = 起動時に表示しない
btn-close = 閉じる
rules-body =
    目標：できるだけ長く手を続けること。
    盤面は点の十字から始まります。空きマスに点を置く手は、それで縦・横・斜めに5マスが一直線にそろい、残り4マスがすでに点であるときに可能で、その5点を結ぶ線を引きます。
    補う空きマスは線の端でも途中でもかまいません。（4系では4マス：3点＋1。）
    同じ向きの線どうしは決して重なれません。離散（D）系では端どうしも接してはいけません。接触（T）系では端を1つだけ共有できます。
    着手できるマスは強調表示されます。クリックして打つか、「自動探索」でコンピュータに探させましょう。

meta-title = メタデータ
meta-author = 作者
meta-source = 出典
meta-transcribed-by = 転記者
meta-description = 説明
meta-tags = タグ
meta-tags-hint = カンマ区切り
author-prompt-title = お名前
author-prompt-body = エクスポートに署名する名前を入力してください（「作者」欄）。
author-prompt-remember = 記憶する
author-prompt-ok = 保存
author-prompt-skip = スキップ

exhausted-title = 全空間を探索完了
exhausted-body = ゲーム木を { $time } で網羅的に探索しました。最高スコア { $score } は、このバリアントの証明された最適値です。

status-no-msr-data = このファイルには Morpion Solitaire のデータが含まれていません。
status-copied-png-no-record = 画像をコピーしました（埋め込みデータなし。PNG ファイルにエクスポートすると含まれます）。
drop-hint = .msr・.png・.svg ファイルをドロップして読み込み
link-docs = ドキュメント
link-source = ソース

# Line picker mode (Aim = cursor + scroll wheel, Click = click to lock + aim + click to play)
pick-mode-label = 選択
pick-mode-aim = 照準
pick-mode-click = クリック
pick-mode-aim-hint = カーソルで照準、ホイールで線を切り替え、クリックで着手。
pick-mode-click-hint = クリックで点を固定、移動して照準、もう一度クリックで着手。
pick-locked-hint = 線を狙う · クリックで配置 · 右クリックまたは Esc で取消

# エンジン調整オプション（プラグインレジストリから汎用的に描画）
opt-level = NRPA レベル
opt-level-hint = 入れ子の深さ。3 = 高速（1 分で約 99）。4 以上はより深く探索するが、長時間実行でのみ効果がある。
opt-width = ビーム幅
opt-width-hint = 各深さで保持する候補数。広いほど網羅的だが遅くなる。
opt-symmetry = 対称符号化
opt-symmetry-hint = 手の正準 D4 符号化。オフ（恒等フレームのみ）にするとスコアは同等で約 +16% 高速 — コールドな記録探索に有効。
opt-clamp = ロジットクランプ (C)
opt-clamp-hint = Stabilized-NRPA のクランプ。記録狙いでは 3 が最適。0 で無効。
opt-alpha = ステップ幅 (α)
opt-alpha-hint = 方策の適応ステップ。既定は 1.0。実験時のみ調整。
opt-crossover = 交叉率
opt-crossover-hint = 摂動のみ：1 ラウンドが破壊・修復の代わりに 2 つの保存済みゲームを組み換える確率。0 = 無効。
opt-neural-scale = ニューラル事前分布の強さ
opt-neural-scale-hint = ニューラル手事前分布の β スケール。最適値は約 4。事前分布を読み込んだ場合のみ有効。

# ニューラル事前分布パネル（機能 `neural`）
prior-section = ニューラル事前分布
prior-none = なし
prior-bundled = 同梱
prior-corpus = コーパス
prior-tabula-rasa = タブラ・ラサ
prior-file = ファイル
prior-none-hint = 素の NRPA — 学習済みの手事前分布なし。
prior-bundled-hint = 同梱の「ゼロから」事前分布 — 即時、学習不要、人間の記録不要。
prior-corpus-hint = 同梱の人間の記録で事前分布を学習（CPU で約 40 秒）。
prior-tabula-rasa-hint = Expert Iteration でゼロから学習 — 記録なし。ここでは数分；本格的な実行は CLI で。
prior-file-hint = 以前保存した事前分布を読み込む（safetensors）。
btn-load-prior = 読み込み…
btn-cancel-training = 学習を中止
prior-status-training = 事前分布を学習中…
prior-status-ready = 事前分布の準備完了 ✓
prior-status-error = エラー: { $error }
algo-puct = PUCT
opt-c-puct = PUCT 探索 (c)
opt-c-puct-hint = PUCT 探索定数 — 大きいほど探索的。既定 1.5。
opt-feat-adapt = 特徴空間 NRPA
opt-feat-adapt-hint = 固定の事前バイアスの代わりに、ネットの凍結特徴上のヘッドをオンライン適応（φ-B）。事前分布が必要。実験的。
opt-feat-alpha = 特徴空間ステップ (α_θ)
opt-feat-alpha-hint = 特徴空間 NRPA のヘッドのステップ幅。既定 0.1。有効時のみ。
