# LittleDraw

![LittleDraw](docs/screenshots/littledraw.png)

Motor de dibujo vectorial **nativo en Rust**, con un gestor de proyectos y pizarras. Abre y guarda archivos `.excalidraw` y se ejecuta directamente en GPU, **sin Chromium ni Electron**.

> **Basado en Excalidraw:** reproduce su diseño, su estética *hand-drawn*, el comportamiento de sus herramientas y la compatibilidad con el formato `.excalidraw`, portando algoritmos de [rough.js](https://github.com/rough-stuff/rough) y [perfect-freehand](https://github.com/steveruizok/perfect-freehand). [Excalidraw](https://github.com/excalidraw/excalidraw) es MIT.

## Capturas

> Las capturas del gestor y del editor usan **datos de ejemplo inventados**, no un espacio de trabajo real.

### Gestor de proyectos

| Tema claro | Tema oscuro |
|---|---|
| ![Gestor de proyectos en tema claro](docs/screenshots/manager-light.png) | ![Gestor de proyectos en tema oscuro](docs/screenshots/manager-dark.png) |

### Editor

| Tema claro | Tema oscuro |
|---|---|
| ![Editor en tema claro, con una figura seleccionada y el panel de estilos](docs/screenshots/editor-light.png) | ![Editor en tema oscuro, con una figura seleccionada y el panel de estilos](docs/screenshots/editor-dark.png) |

### Herramientas, texto y figuras
![Interfaz del editor con barra de herramientas, panel de estilos, figuras, flecha anclada y edición de texto](docs/screenshots/editor-ui.png)

### Trazos con presión de lápiz (Wayland)
![Trazos libres con presión real de tableta gráfica en Wayland](docs/screenshots/pen-pressure.png)

### Renderizado de `samples/demo.excalidraw`
![Exportación a PNG de samples/demo.excalidraw mostrando rellenos hachure, cross-hatch, solid, trazos punteados, flechas curvas y rotación](docs/screenshots/demo-render.png)

## Características

**Motor de dibujo**

- **Renderizado GPU directo:** rasterización de curvas Bézier con compute shaders de `vello` (`0.12`) sobre `wgpu`, con caché de geometría por elemento.
- **Trazos libres con presión real:** contorno de grosor variable (`perfect-freehand`) que guarda `pressures` y `simulatePressure` en `.excalidraw`. Usa la presión real del lápiz en Wayland (`winit 0.31`) y simula la presión por velocidad con ratón.
- **Figuras, texto e imágenes:** rectángulos, rombos, elipses, líneas y flechas multipunto con *bindings*, texto multilínea con fuentes WOFF2 embebidas, etiquetas dentro de figuras, imágenes (`PNG`, `JPEG`, `GIF`, `WebP`, `BMP`), rellenos (`solid`, `hachure`, `cross-hatch`, `zigzag`), rugosidad, esquinas redondeadas, rotación, paleta de colores y orden de capas.
- **Interoperabilidad `.excalidraw`:** abre, edita y guarda archivos preservando los campos desconocidos, copia y pega en formato Excalidraw, y exporta a PNG o SVG desde la aplicación (`Ctrl+Shift+E` en el editor, o «Exportar imagen» en el menú de cada pizarra) y desde la línea de comandos.

**Gestor de proyectos y pizarras**

- Proyectos con color, lista de **recientes**, búsqueda y tarjetas con **miniatura** de cada pizarra.
- Crear, renombrar, duplicar, mover entre proyectos y eliminar pizarras; crear, renombrar, cambiar de color y eliminar proyectos.
- **Importar y exportar** pizarras `.excalidraw`, y abrir la carpeta de datos desde la propia aplicación.
- **Autoguardado** (700 ms después de la última edición) y miniaturas que se actualizan como máximo cada 5 s, con un lado máximo de 640 px.

## Modo claro y oscuro

- El gestor y el editor comparten **un solo tema**. Se cambia con el botón de la barra lateral del gestor ("Claro" / "Oscuro"), con el botón de la esquina superior derecha del editor o con `Alt+Shift+D`.
- Como en Excalidraw, el tema solo cambia cómo se muestran los colores: el archivo no se modifica y las imágenes conservan los suyos.
- Por defecto arranca en **claro** (no detecta el tema del sistema). La última elección se recuerda en `~/.config/board/theme` (o `$XDG_CONFIG_HOME/board/theme`), y `--dark` o `--light` la sustituyen solo para esa ejecución.
- Las miniaturas del gestor se generan siempre con fondo claro, también en el tema oscuro.

## Compilar y ejecutar

```bash
# Gestor de proyectos (sin argumentos)
cargo run --release -p board-app

# Abrir un archivo .excalidraw directamente en el editor
cargo run --release -p board-app -- samples/demo.excalidraw

# Usar otra carpeta de datos para el gestor
cargo run --release -p board-app -- --workspace ~/mis-pizarras

# Forzar el tema en esta ejecución
cargo run --release -p board-app -- --dark

# Exportación sin ventana a SVG / PNG
cargo run --release -p board-app -- samples/demo.excalidraw --svg salida.svg --png salida.png --scale 2

# Captura de la interfaz del editor sin abrir ventana (añade "dark" para el tema oscuro)
cargo run -p board-app --example ui_snapshot -- docs/screenshots/editor-ui.png

# Pruebas del núcleo
cargo test -p board-core
```

### Datos del gestor

El gestor guarda todo en una carpeta, por defecto `~/.config/Boards/workspace` (o `$XDG_CONFIG_HOME/Boards/workspace`):

```text
workspace/
├── workspace.json       # índice de proyectos y pizarras
├── boards/*.excalidraw  # una escena por pizarra
└── thumbnails/*.png     # miniaturas
```

Se puede cambiar con `--workspace DIR` o con la variable de entorno `BOARDS_WORKSPACE` (la opción `--workspace` tiene prioridad). `workspace.json` usa el mismo esquema que la versión de escritorio anterior del gestor (Electron), así que ambos espacios son intercambiables. El nombre de la carpeta sigue siendo `Boards` para no dejar sin acceso a los espacios de trabajo existentes.

## Arquitectura

```text
.
├── Cargo.toml
├── crates/
│   ├── core/                # board-core: núcleo puro, sin ventana, GPU ni reloj
│   │   ├── src/
│   │   │   ├── element.rs     # Modelo de escena y serialización JSON .excalidraw
│   │   │   ├── rough.rs       # Trazos y rellenos hand-drawn deterministas (rough.js)
│   │   │   ├── freehand.rs    # Contornos de grosor variable (perfect-freehand)
│   │   │   ├── shape.rs       # Geometría de elementos → operaciones de dibujo
│   │   │   ├── text.rs        # Fuentes WOFF2, medición, wrapping y glifos
│   │   │   ├── font_data.rs   # Fuentes de Excalidraw embebidas en el binario
│   │   │   ├── bound.rs       # Texto ligado dentro de contenedores
│   │   │   ├── binding.rs     # Flechas ancladas a figuras
│   │   │   ├── hit.rs         # Hit-testing
│   │   │   ├── editor.rs      # Herramientas, selección, edición, undo/redo, Frame
│   │   │   ├── ui.rs          # Barra de herramientas, panel de estilos y tema
│   │   │   ├── manager_ui.rs  # Interfaz del gestor: barra lateral, tarjetas y diálogos
│   │   │   ├── workspace.rs   # Modelo del índice workspace.json
│   │   │   ├── draw.rs        # Lista de dibujo (Frame, DrawOp, Color)
│   │   │   └── export.rs      # Exportación headless a SVG y lista de dibujo para PNG
│   │   └── fonts/           # Fuentes WOFF2 embebidas
│   └── app/                 # board-app: aplicación de escritorio (winit + vello/wgpu)
│       ├── src/
│       │   ├── main.rs      # Ventana, eventos, portapapeles, diálogos y CLI
│       │   ├── render.rs    # Rasterización GPU con vello y exportación PNG
│       │   └── store.rs     # Persistencia en disco del espacio de trabajo
│       └── examples/
│           └── ui_snapshot.rs
├── resources/icon.png
├── samples/demo.excalidraw
└── docs/screenshots/
```

El núcleo no depende de ventana, GPU ni reloj: recibe eventos de entrada y devuelve un `Frame` (una lista de operaciones de dibujo) que la aplicación pinta. Eso permite generar capturas y exportaciones sin abrir ninguna ventana, y mantiene el núcleo independiente de la plataforma.

## Estado y límites

- **Plataforma:** pensado primero para Linux (Wayland). Abrir la carpeta de datos usa `xdg-open`. Windows y la web son objetivos futuros.
- **Pruebas:** el núcleo tiene 90 pruebas (`cargo test -p board-core`); la aplicación incluye pruebas del almacenamiento del espacio de trabajo.
- **Sin sincronización:** los datos son archivos locales; no hay cuentas ni nube.

## Créditos

El diseño, el comportamiento y las fuentes provienen de [Excalidraw](https://github.com/excalidraw/excalidraw) (MIT); las fuentes se incluyen sin modificar. Los algoritmos de trazo provienen de [rough.js](https://github.com/rough-stuff/rough) y [perfect-freehand](https://github.com/steveruizok/perfect-freehand).
