# Reader fonts

Typefaces offered in the reader's font setting. `reader.js` loads them with `@font-face` from this
directory, which the reader's web view serves with the rest of `resource/assets/reader/`.

| File | Family | Source | Upstream version |
|---|---|---|---|
| `Montserrat-Regular.ttf` | Montserrat | [JulietaUla/Montserrat](https://github.com/JulietaUla/Montserrat) `fonts/ttf/Montserrat-Regular.ttf`, commit `679ead9bcd0d` | 9.000 |
| `NotoSans.ttf` | Noto Sans | [notofonts/notofonts.github.io](https://github.com/notofonts/notofonts.github.io) `fonts/NotoSans/unhinted/variable-ttf/NotoSans[wdth,wght].ttf`, commit `28b15b4b43b7` | 2.015 |
| `NotoSerif.ttf` | Noto Serif | [notofonts/notofonts.github.io](https://github.com/notofonts/notofonts.github.io) `fonts/NotoSerif/unhinted/variable-ttf/NotoSerif[wdth,wght].ttf`, commit `28b15b4b43b7` | 2.015 |

The two Noto files are variable fonts with weight and width axes, so a book's bold and semibold
text draws in real weights. Montserrat is the regular weight only; the web view synthesizes bold.

All three are licensed under the SIL Open Font License 1.1: `LICENSE-Montserrat.txt` ("Copyright
2024 The Montserrat.Git Project Authors") and `LICENSE-Noto.txt` ("Copyright 2022 The Noto
Project Authors"). The license files ship inside the app with the fonts, as the OFL requires.
