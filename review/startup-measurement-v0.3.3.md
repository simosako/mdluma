# MDLuma v0.3.3 起動時間測定結果

測定対象: MDLuma v0.3.3 release build

測定ログ: `mdluma-startup-91680-1778301673151.log`

測定方法: `MDLUMA_STARTUP_TRACE=1` を指定し、起動トレースを有効化した状態で release build の `mdluma.exe` を Markdown ファイル指定付きで起動した。

## 要約

初回表示までの時間は約 `301.1ms` だった。

この測定では、起動時間の大半は Sciter 側の初期化、window 作成、HTML ロード、window 表示に集中している。Markdown ファイル読み込みと Comrak 変換は合計でも約 `0.4ms` であり、recent file 記録を含めても startup markdown prepare 全体は約 `1.2ms` に留まった。

したがって、このケースでは Markdown 読み込みと Comrak 変換を Sciter 初期化と並列化しても、理論上の短縮上限は約 `1.2ms` 程度であり、実装コストに対する効果は小さい。

## 初回表示までの内訳

`run begin` から `viewer initial show end` までを初回表示までの起動時間として扱う。

| フェーズ | 時間 | 初回表示までに占める割合 |
| --- | ---: | ---: |
| Sciter DLL load + API binding | `110.395ms` | 約 `36.7%` |
| Sciter app init | `26.974ms` | 約 `9.0%` |
| Sciter create window | `65.786ms` | 約 `21.9%` |
| Sciter initial load_html | `74.887ms` | 約 `24.9%` |
| Sciter show window | `19.603ms` | 約 `6.5%` |
| Markdown file read | `0.108ms` | 約 `0.04%` |
| Comrak render | `0.306ms` | 約 `0.1%` |
| Recent file record | `0.650ms` | 約 `0.2%` |
| HTML shell render | `0.202ms` | 約 `0.07%` |

## 大きい処理

起動時間に対する影響が大きい順に並べると以下の通り。

1. `sciter dll load and api binding`: `110.395ms`
2. `sciter initial load_html`: `74.887ms`
3. `sciter create window`: `65.786ms`
4. `sciter app init`: `26.974ms`
5. `sciter show window`: `19.603ms`

Sciter 関連処理だけで初回表示までのほぼ全体を占めている。

## Markdown 処理

起動時の Markdown 関連処理は以下だった。

| フェーズ | 時間 |
| --- | ---: |
| `startup markdown file read` | `0.108ms` |
| `startup comrak render` | `0.306ms` |
| `startup recent file record` | `0.650ms` |
| `startup markdown prepare` 全体 | `1.193ms` |

この結果から、少なくとも今回の入力ファイルでは Markdown 読み込みや Comrak 変換は起動時間の支配要因ではない。

## Sciter event loop lifetime について

ログ上の `sciter event loop lifetime` は `2182.724ms` だったが、これは初回表示後にイベントループが動作していた時間であり、起動処理そのものではない。

ユーザーが window を閉じるまでの時間を含むため、起動最適化の対象からは外して扱う。

## 判断

今回の測定結果では、Sciter 初期化と Markdown 読み込み/Comrak 変換の並列化は優先度が低い。

理由は以下の通り。

- Markdown 読み込みと Comrak 変換が約 `0.4ms` と非常に短い。
- Recent file 記録まで含めても Markdown prepare は約 `1.2ms` に留まる。
- 初回表示までの約 `301.1ms` の大半は Sciter 側で消費されている。
- 並列化しても短縮できる最大値は Markdown prepare の約 `1.2ms` 程度である。

## 次に確認すべき点

次の調査では、Markdown/Comrak ではなく Sciter 側を優先して確認する。

1. `SciterLoadHtml` に渡す HTML サイズと内容の影響
2. HTML shell 内の CSS、JavaScript、アイコン data URL 量の影響
3. `SciterCreateWindow` と `SciterExec(SCITER_APP_INIT)` の固定コスト
4. DLL load が初回だけ重いのか、2回目以降も同程度なのかを複数回測定する

## 元ログ抜粋

```text
[+     628us] sciter runtime validation and load begin
[+     930us] sciter dll load and api binding begin
[+  111534us] sciter dll load and api binding end duration_us=110395
[+  111603us] sciter runtime validation and load end duration_us=110892
[+  111670us] sciter window construction begin
[+  111812us] sciter app init begin
[+  138811us] sciter app init end duration_us=26974
[+  138844us] sciter create window begin
[+  204651us] sciter create window end duration_us=65786
[+  204891us] sciter window construction end duration_us=93204
[+  205122us] startup markdown prepare begin
[+  205143us] startup markdown file read begin
[+  205272us] startup markdown file read end duration_us=108
[+  205295us] startup comrak render begin
[+  205622us] startup comrak render end duration_us=306
[+  206336us] startup markdown prepare end duration_us=1193
[+  206418us] viewer initial show begin
[+  206436us] html shell render begin
[+  206655us] html shell render end duration_us=202
[+  206683us] sciter initial load_html begin
[+  281590us] sciter initial load_html end duration_us=74887
[+  281627us] sciter show window begin
[+  301251us] sciter show window end duration_us=19603
[+  301306us] viewer initial show end duration_us=94871
```
