# Desarrollo

## Compilar y probar

```bash
cargo run --release                                   # gestor de proyectos
cargo run --release -- samples/demo.excalidraw        # editor con un archivo
cargo test --workspace                                # todas las pruebas
cargo test -p littledraw-core                         # solo el núcleo
```

El paquete `littledraw` (en `crates/app`) genera el binario `littledraw`.

## Arquitectura

El proyecto es un workspace de Cargo con dos crates:

- **`littledraw-core`** (`crates/core`): el motor completo, sin ventana, GPU ni reloj. Recibe eventos de entrada (puntero, teclado, texto) y devuelve un `Frame`, que es una lista de operaciones de dibujo. No depende de la plataforma, así que también sirve para una versión web (WASM) y para generar capturas y exportaciones sin abrir ventana.
- **`littledraw`** (`crates/app`): la aplicación de escritorio. Abre la ventana con `winit`, pinta el `Frame` en la GPU con `vello` sobre `wgpu`, y se ocupa del disco, el portapapeles y los diálogos.

```text
crates/
├── core/src/
│   ├── element.rs     Modelo de escena y JSON .excalidraw (conserva campos desconocidos)
│   ├── editor.rs      Herramientas, selección, edición de texto, deshacer/rehacer, Frame
│   ├── ui.rs          Barra de herramientas, panel de estilos y tema
│   ├── manager_ui.rs  Interfaz del gestor: barra lateral, tarjetas y diálogos
│   ├── workspace.rs   Modelo del índice workspace.json
│   ├── shape.rs       Geometría de cada elemento → operaciones de dibujo
│   ├── rough.rs       Trazos y rellenos dibujados a mano (port de rough.js)
│   ├── freehand.rs    Contorno de grosor variable (port de perfect-freehand)
│   ├── plane.rs       Plano cartesiano y ajuste a su cuadrícula
│   ├── text.rs        Fuentes WOFF2, medición, ajuste de líneas, sub/superíndices
│   ├── font_data.rs   Fuentes embebidas en el binario
│   ├── bound.rs       Texto dentro de contenedores
│   ├── binding.rs     Flechas ancladas a figuras
│   ├── hit.rs         Detección de clics sobre elementos
│   ├── draw.rs        Frame, DrawOp, Color, Style
│   └── export.rs      Exportación a SVG y Frame para PNG
├── core/fonts/        Fuentes WOFF2 y sus licencias
└── app/
    ├── src/
    │   ├── main.rs    Ventana, eventos, portapapeles, diálogos y línea de comandos
    │   ├── render.rs  Rasterización con vello y exportación PNG
    │   └── store.rs   Carpeta de datos del gestor (workspace.json, escenas, miniaturas)
    └── examples/ui_snapshot.rs
resources/             Icono y entrada de escritorio (littledraw.desktop)
samples/               Escenas de ejemplo
docs/screenshots/      Capturas del README
```

### Formato de archivo

- Las escenas son `.excalidraw` estándar. Lo que LittleDraw añade va en campos que Excalidraw ya tolera:
  - el plano cartesiano guarda sus dimensiones en `customData.cartesianPlane` de su cuadrícula;
  - el relleno en cuadrícula usa `fillStyle: "grid"`, que Excalidraw muestra como hachure.
- `workspace.json` usa el mismo esquema (`version: 1`) que el gestor anterior, hecho en Electron.

### Nombres internos

En el código, una **pizarra** se llama `board` (`Board`, `boards/`, `board_id`). Es el término del modelo de datos, no el nombre del producto, y se mantiene para no romper `workspace.json` ni las carpetas existentes.

## Capturas

`docs/screenshots/` se genera sin abrir ventana:

```bash
# Editor con la interfaz (añade "dark" para el tema oscuro)
cargo run --release --example ui_snapshot -- docs/screenshots/editor-ui.png

# Exportación de una escena
cargo run --release -- samples/demo.excalidraw --png docs/screenshots/demo-render.png --scale 2
cargo run --release -- samples/matematicas.excalidraw --png docs/screenshots/math-tools.png --scale 2
```

Las capturas del gestor y `pen-pressure.png` se toman a mano desde la aplicación.

## Fuentes y licencias

Para añadir o cambiar una fuente, copia el WOFF2 en `crates/core/fonts/<Familia>/`, regístralo en `crates/core/src/font_data.rs` y añade su copyright y licencia a `crates/core/fonts/README.md`. Si la licencia es nueva, añade también su texto. Las dependencias de Rust deben tener licencias compatibles con MIT.
