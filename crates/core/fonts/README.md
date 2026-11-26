# Fuentes incluidas

Las fuentes se embeben en el binario (`src/font_data.rs`). Son los archivos WOFF2 que distribuye [Excalidraw](https://github.com/excalidraw/excalidraw/tree/master/packages/excalidraw/fonts), sin modificar, salvo Liberation Sans (ver abajo).

| Carpeta | Familia (`fontFamily`) | Copyright | Licencia |
|---|---|---|---|
| `Excalifont/` | Excalifont (5) | Copyright (c) 2024 by Excalidraw | SIL OFL 1.1 |
| `Virgil/` | Virgil (1) | Copyright (c) 2011 by Your Own Font Foundry | SIL OFL 1.1 |
| `Nunito/` | Nunito (6) | Copyright 2014 The Nunito Project Authors (https://github.com/googlefonts/nunito) | SIL OFL 1.1 |
| `Lilita/` | Lilita One (7) | Copyright (c) 2011 Juan Montoreano, with Reserved Font Name "Lilita One" | SIL OFL 1.1 |
| `Cascadia/` | Cascadia Code (3) | Copyright (c) 2019 - Present, Microsoft Corporation, with Reserved Font Name Cascadia Code | SIL OFL 1.1 |
| `Liberation/` | Liberation Sans (2, 9) | Digitized data copyright (c) 2010 Google Corporation, with Reserved Font Arimo, Tinos and Cousine. Copyright (c) 2012 Red Hat, Inc., with Reserved Font Name Liberation | SIL OFL 1.1 |
| `ComicShanns/` | Comic Shanns (8) | Copyright (c) 2018 Shannon Miwa, 2023 Jesus Gonzalez, 2023 Rodrigo Batista de Moraes, 2024 Fini Jastrow, 2024 Kyle Beechly | MIT |

- El texto de la SIL Open Font License 1.1 está en [`OFL.txt`](OFL.txt).
- La licencia MIT de Comic Shanns está en [`ComicShanns/LICENSE`](ComicShanns/LICENSE).
- **Liberation Sans** es la versión 2.1.5 del proyecto [liberation-fonts](https://github.com/liberationfonts/liberation-fonts), convertida de TTF a WOFF2 sin cambiar glifos. Excalidraw distribuye la 1.05, que tiene licencia GPLv2 con excepción de fuentes; la 2.x tiene licencia OFL y los mismos anchos de carácter, así que el texto se mide igual.
