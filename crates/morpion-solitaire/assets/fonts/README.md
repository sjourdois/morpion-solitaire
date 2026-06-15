# Bundled fonts

Both fonts are licensed under the **SIL Open Font License, Version 1.1**
(see [`OFL.txt`](OFL.txt)).

| File | Font | Copyright | Notes |
|------|------|-----------|-------|
| `AtkinsonHyperlegibleNext-Regular.ttf`, `-Bold.ttf` | Atkinson Hyperlegible Next | © Braille Institute of America, Inc. | The UI typeface (Latin). |
| `NotoSansJP-subset.otf` | Noto Sans CJK JP | © The Noto Project Authors (https://github.com/notofonts/noto-cjk) | **Subset** to the glyphs used by the Japanese locale plus the full kana/CJK-punctuation ranges, used as the CJK fallback. |

## Regenerating the Japanese subset

The Noto subset only contains the kanji actually used in `locales/ja`. If the
Japanese strings gain new kanji, re-subset from a full Noto Sans CJK JP:

Face 0 of the system `NotoSansCJK-Regular.ttc` is *Noto Sans CJK JP*. Subsetting
straight from the locale file keeps exactly the kanji it uses (plus the full
kana / full-width / CJK-punctuation ranges and the `日本語` endonym):

```sh
python3 -m fontTools.subset NotoSansCJK-Regular.ttc --font-number=0 \
  --text-file=locales/ja/morpion_solitaire.ftl --text="日本語" \
  --unicodes=3000-303F,3040-30FF,FF00-FFEF \
  --output-file=assets/fonts/NotoSansJP-subset.otf --no-hinting --desubroutinize
```
