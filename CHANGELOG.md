# Cambios

El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y las versiones siguen [SemVer](https://semver.org/lang/es/).

## [Sin publicar]

### Añadido
- Demo en el navegador ([im4dev.github.io/LittleDraw/demo](https://im4dev.github.io/LittleDraw/demo/)): el mismo editor compilado a WebAssembly y dibujado con Canvas 2D. Abre, guarda y exporta archivos, y recuerda el dibujo en el navegador.

### Cambiado
- Los dibujos nuevos guardan `"source": "littledraw"` en lugar de `"pizarra"`.

## [0.1.0] - 2026-11-27

### Añadido
- Versión para Windows, con instalador (`Setup.exe`) e icono en el ejecutable.
- Instaladores con actualización automática (Velopack): AppImage para Linux y `Setup.exe` para Windows. Las versiones nuevas se descargan en segundo plano y se instalan al cerrar la aplicación.
- Página web con descargas en GitHub Pages.
- Plano cartesiano (tecla `C`) con ajuste de líneas y flechas a su cuadrícula.
- Relleno en cuadrícula (`grid`); las líneas de hachure y cross-hatch ya no se salen de la figura.
- Selección de texto durante la edición, y subíndices y superíndices con `Ctrl+=` y `Ctrl++`.
- Exportar PNG o SVG desde el editor (`Ctrl+Shift+E`) y desde el menú de cada pizarra del gestor.
- Tema oscuro compartido por el editor y el gestor.
- Gestor de proyectos y pizarras nativo en Rust, que reemplaza al de Electron.
- Licencia MIT, avisos de terceros (`THIRD_PARTY_NOTICES.md`) y licencias de las fuentes.
- Entrada de escritorio `resources/littledraw.desktop`; en Wayland la ventana usa el app id `littledraw`.

### Cambiado
- El producto se llama **LittleDraw** en todo el proyecto: el binario es `littledraw` y los crates son `littledraw` y `littledraw-core`.
- Los datos del gestor pasan a `~/.local/share/littledraw/workspace`. Una carpeta anterior en `~/.config/Boards/workspace` se mueve sola y deja un enlace en su lugar.
- El tema se guarda en `~/.config/littledraw/theme`; el de `~/.config/board/theme` se sigue leyendo.
- La variable de entorno de la carpeta de datos es `LITTLEDRAW_WORKSPACE`; `BOARDS_WORKSPACE` ya no se usa.
- Liberation Sans pasa a la versión 2.1.5, con licencia OFL, en lugar de la 1.05, con licencia GPL. Los anchos de carácter no cambian.

### Corregido
- Las imágenes ya no desaparecen después de renderizar un frame sin imágenes.
