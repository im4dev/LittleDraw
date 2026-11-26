# Créditos y avisos de terceros

LittleDraw tiene licencia [MIT](LICENSE). Incluye código portado y recursos de otros proyectos, cuyos avisos de copyright y licencias se reproducen aquí como esas licencias exigen.

## Código portado

| Proyecto | Qué se portó | Dónde |
|---|---|---|
| [Excalidraw](https://github.com/excalidraw/excalidraw) | Diseño de la interfaz, comportamiento de las herramientas, formato `.excalidraw`, texto ligado a contenedores, flechas ancladas, ajuste de líneas de texto | `crates/core/src/` (`bound.rs`, `binding.rs`, `text.rs`, `editor.rs`, `ui.rs`) |
| [rough.js](https://github.com/rough-stuff/rough) | Trazos y rellenos con aspecto dibujado a mano | `crates/core/src/rough.rs` |
| [perfect-freehand](https://github.com/steveruizok/perfect-freehand) | Contorno de grosor variable de los trazos libres | `crates/core/src/freehand.rs` |

## Fuentes

Las fuentes embebidas tienen licencia SIL Open Font License 1.1, salvo Comic Shanns (MIT). El detalle por familia, con su copyright, está en [`crates/core/fonts/README.md`](crates/core/fonts/README.md), y el texto de la OFL en [`crates/core/fonts/OFL.txt`](crates/core/fonts/OFL.txt).

## Dependencias de Rust

Las bibliotecas de crates.io que se compilan dentro del binario (`vello`, `wgpu`, `winit`, `kurbo`, `skrifa`, `serde`, `image` y sus dependencias) tienen licencias permisivas: MIT, Apache-2.0, BSD, Zlib, ISC y Unicode-3.0. Cada una incluye su texto de licencia en su paquete.

## Licencias

### Excalidraw

```text
MIT License

Copyright (c) 2020 Excalidraw

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### rough.js

```text
MIT License

Copyright (c) 2019 Preet Shihn

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### perfect-freehand

```text
MIT License

Copyright (c) 2021 Stephen Ruiz Ltd

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
