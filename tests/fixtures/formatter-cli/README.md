# formatter-cli fixtures

`likec4 fmt`（公式 TypeScript 実装、CLI）をオラクルとして生成したフォーマッタ互換性テスト用フィクスチャ。

- `tests/fixtures/formatter/`（既存 48 件）は language-server の spec 由来。
- `tests/corpus/examples*/`（既存、39 件相当）は実サンプルプロジェクト由来。
- ここ `formatter-cli/` は、上記 2 つで拾いきれていないコーナーケース（コメント位置、1 行ブロック、タグ区切り、クォート混在、トップレベルの整形バグなど）を狙って新規に書き下ろし、公式 CLI に通した結果をそのまま `expected` として保存したもの。

## likec4 バージョン

`likec4@1.59.3`（`likec4 --version` で確認）。

## 生成コマンド

各ケースは 1 ファイル 1 コーナーケースとして独立に書いた（`cases/NN-slug.c4`）。

`likec4 fmt <dir>` は指定ディレクトリ配下の全 `.c4` を、1 つのワークスペース（1 プロジェクト）としてまとめて読み込む。そのため複数ケースを同じディレクトリに並べると、`element system` や `a`/`b` のような使い回した識別子がケース間で衝突し、`Duplicate element kind` 等のエラーが大量に出る（このエラー自体はフォーマット処理をブロックしない＝出力は正しいが、ログが汚れて検証しづらい）。そこで、各ケースを 1 ファイルだけを含む独立ディレクトリにコピーしてから 1 件ずつフォーマットした。

```bash
# 作業ディレクトリ: scratchpad/oracle/
# 1) 各ケースを個別ディレクトリに複製
for f in cases/*.c4; do
  name=$(basename "$f" .c4)
  mkdir -p "cases-formatted/$name"
  cp "$f" "cases-formatted/$name/$name.c4"
done

# 2) ケースごとに公式 CLI で整形（in-place 書き換え）
for d in cases-formatted/*/; do
  node_modules/.bin/likec4 fmt "$d"
done

# 3) 結果をフラット化し、input/expected として保存
for f in cases/*.c4; do
  name=$(basename "$f" .c4)
  cp "$f" ".../formatter-cli/${name}.input.c4"
  cp "cases-formatted/$name/$name.c4" ".../formatter-cli/${name}.expected.c4"
done
```

`expected.c4` は上記コマンドの生の出力そのままで、手編集はしていない（18 のみ例外。詳細は後述）。

## 参照解決について

各ケースは構文的に妥当な単独の LikeC4 文書だが、要素参照が同一ファイル内で解決できる必要はない（`import` のケースなど）。`likec4 fmt` は `Could not resolve reference to ...` のような ERROR ログを出しても、それが構文エラーでない限りフォーマット自体はスキップしない（実測で確認済み）。構文エラー（パース不能）の場合のみ、そのファイルは無変更のままスキップされる。今回の 52 件はいずれも構文エラーなしで、全件フォーマット処理が実行された。

## ケース一覧

| # | slug | 狙い | 整形で変化 |
|---|---|---|---|
| 01 | comment-between-elements | 兄弟要素の間にある行コメントの位置保持 | なし |
| 02 | comment-before-closing-brace | `}` 直前の行コメント | なし |
| 03 | comment-trailing-same-line | 要素ヘッダ/タグと同じ行の末尾コメント | なし |
| 04 | comment-block-misindented | インデントが揃わない複数行 `/* */` ブロックコメント | あり |
| 05 | comment-file-start-and-end | ファイル先頭・末尾（どの宣言にも属さない）コメント | なし |
| 06 | oneline-model-block | `model { a = system }` の1行ブロック | なし |
| 07 | block-spacing-tight-vs-loose | 要素ボディの空白なし/過剰な空白での1行表現 | あり |
| 08 | empty-body-variants | 空ボディ `{}` / `{ }` / `{\n}` の3形態 | あり |
| 09 | leading-blank-lines-and-indented-model | ファイル先頭の空行 + インデントされた `model` キーワード | あり |
| 10 | indented-deployment-block | インデントされたトップレベル `deployment { }` | あり |
| 11 | multiple-blank-lines-between-blocks | トップレベルブロック間の複数空行 | なし |
| 12 | tags-trailing-semicolon | タグリスト末尾の `;` | なし |
| 13 | tags-in-relation-header | 関係ヘッダ内のタグ列挙 `#t1, #t2` | なし |
| 14 | title-colon-spacing | `title` プロパティのコロン有無・空白・`;` | あり |
| 15 | description-markdown-block | 複数行 markdown description（内部インデント保持） | なし |
| 16 | link-with-title | `link https://... 'title'` | なし |
| 17 | icon-libicon | `icon aws:lambda` | なし |
| 18 | metadata-array-values | `metadata` の配列値 `['a','b']` | あり |
| 19 | relation-extra-spaces-around-arrow | `a  ->  b` の余剰空白 | あり |
| 20 | relation-dotkind | `a .uses b`（dotKind 構文） | なし |
| 21 | relation-bracket-kind | `a -[uses]-> b`（bracket kind 構文） | なし |
| 22 | relation-bidirectional | `a <-> b` | なし |
| 23 | relation-implicit-source-in-element | 要素ボディ内の暗黙ソース `-> b` | なし |
| 24 | relation-multiline-title-desc-tech | `title`/`description`/`technology` が複数行にまたがる関係 | あり |
| 25 | include-wildcard-only | `include *` | なし |
| 26 | include-multi-oneline | `include *, a.*, c._` を1行で | なし |
| 27 | include-multi-multiline | 同じリストを複数行で | なし |
| 28 | include-leading-comma-multiline | 継続行の先頭にカンマを置くスタイル | あり |
| 29 | exclude-chain | `exclude -> a ->` | なし |
| 30 | with-oneline | `include * with { color red }` を1行で | なし |
| 31 | with-multiline | 同じ内容を複数行で | あり |
| 32 | rank-same | `rank same { a, b }` | なし |
| 33 | group-untitled | タイトルなし `group { include a, b }` | なし |
| 34 | dynamic-simple-step | 単純な `a -> b 'x'` | なし |
| 35 | dynamic-catch-same-line-as-try-close | `try` の閉じ `}` と同じ行の `catch`/`finally` | なし |
| 36 | dynamic-multiline-chain-odd-indent | 複数行チェーンで継続行のインデントがズレている | あり |
| 37 | deployment-node-with-instances | 名前なし/ありの `instanceOf` | なし |
| 38 | deployment-extend-empty | ネストした `deploymentNode` への空 `extend` | なし |
| 39 | deployment-relation-toplevel | `deployment {}` 直下のトップレベル関係 | なし |
| 40 | quotes-majority-single | シングル多数・ダブル少数 → auto は single | あり |
| 41 | quotes-majority-double | ダブル多数・シングル少数 → auto は double | あり |
| 42 | quotes-tie-goes-double | 同数 → auto は double | あり |
| 43 | quotes-internal-escaped-both | 内部エスケープ済みクォートを含む単一/二重の混在 | あり |
| 44 | quotes-markdown-triple-in-mixed-file | markdown 3連クォートも auto 投票の1票として数えられる（single 1票 + markdown(double) 1票の同数で double に倒れ、無関係な文字列側が変換される） | あり |
| 45 | import-braced-multi | `import { a, b } from 'proj'` | なし |
| 46 | import-single-semicolon | `import a from "proj";` | なし |
| 47 | global-style-group | `global { styleGroup g { style * {...} } }` | なし |
| 48 | specification-element-with-style | `element` 内 `style { shape: rectangle }` | なし |
| 49 | specification-tag-color-body-not-reformatted | `tag` の `{ color ... }` ボディは整形対象外 | なし |
| 50 | specification-custom-color-rgb | `color custom rgb(1,2,3)` | なし |
| 51 | specification-relationship-style | `relationship` 内 `line`/`head` スタイル | なし |
| 52 | spec-idempotent-source | `tests/fixtures/formatter/48-is-idempotent.input.c4` と同一のソース（spec の "is idempotent" テストは公式スナップショットを持たず expected == input を仮定しているだけなので、同じ文書を公式 CLI に通して実オラクルを取った） | あり |

合計 52 件、うち整形で変化したのは 17 件（04, 07, 08, 09, 10, 14, 19, 24, 28, 31, 36, 40, 41, 42, 43, 44, 52。18 は含まない、理由は後述）。skip されたケースはなし（全件、構文エラーなしでフォーマット処理が実行された）。45, 46 は import 先プロジェクトが実在しないため `Could not resolve reference` 等の ERROR ログが出るが、これは意図通り（要素参照は同一ファイル内で解決できる必要はないという前提どおり、フォーマットはブロックされない）。

## 既存フィクスチャとの重複回避

以下は `tests/fixtures/formatter/` 側で既に厚く網羅されているため、意図的に対象から外した、または最小限にとどめた：

- `include`/`exclude` の空白揺れ全般 → 16
- `where` 式の `and`/`or`/`not`/`()` → 18, 19
- `global style x` の参照 → 20
- `style *, a, b { }` の複数ターゲット → 22
- `autoLayout` のパラメータ付き → 25
- タイトル付き `group` とネスト → 26
- `predicateGroup`/`dynamicPredicateGroup` → 06
- `chained`/`multiline chained`/`parallel` な dynamic view step → 27, 28, 29
- try/catch/finally が別行の catch → 30
- metadata のコロン/空白揺れ（スカラー値） → 09

代わりに、上記にない角度（`exclude` 単体、`rank`、タイトルなし `group`、`styleGroup`、catch が同じ行、metadata の配列値、markdown フェンスの quote 投票参加など）を優先した。

## 意図的な差異（likec4-fmt が公式と異なる箇所）

- `18-metadata-array-values.expected.c4` の `k2: ['a','b']` は手で修正した唯一の例外である。公式フォーマッタは
  `key: value`（metadata 属性と `link:`）に対して「`key` の後ろに 1 スペース挿入」と「`:` の前のスペース除去」の
  2 つの編集を同じ空の位置に生成し、両方を適用してしまうため、`k2: [...]` → `k2 : [...]` → `k2: [...]` と
  整形のたびに振動する（likec4 v1.59.3 で実測）。likec4-fmt は後勝ちにして `key: value` に安定させている。手で
  `k2 : [...]` を `k2: [...]` に戻したことで、たまたま input と同一のバイト列になっている（= 上の「整形で変化した
  17 件」に 18 を含めていない理由）。上記の再生成コマンドをそのまま再実行すると 18 の `expected.c4` は公式の
  生出力（`k2 : [...]`、振動する側）で上書きされてしまうため、再生成のたびにこの 1 ファイルだけ手で戻す必要がある。
- 同じ「同じ空の位置に 2 つの編集」という公式のバグは `key: value` に限らない。`import{a}from`、`try'x'{`、
  `include b -[uses]->c` でも同様に公式は 1 回目の整形で余分な空白を残し、2 回目でようやく収束する。likec4-fmt は
  どのケースでも後勝ちにして公式の収束後の形にいきなり一致させている（`tests/fixtures/formatter-quirks/same-gap-last-wins`
  に該当ケースをまとめて固定化した。今回の `formatter-cli/` 配下の 52 件には `import{a}from` / `try'x'{` /
  `-[kind]->` タイトなブラケット記法のいずれの形も踏む入力はない）。
- markdown 文字列（`'''...'''` / `"""..."""`）の内容が切替先クォートで終わる場合、likec4-fmt は正規化を
  スキップする（正規化すると公式が読めない `"""...\""""` のような出力になるため）。`formatter-cli/` の
  43（`quotes-internal-escaped-both`）・44（`quotes-markdown-triple-in-mixed-file`）はこの境界条件には該当しない
  （どちらも通常どおり正規化される）。
