# GTK4 Notes (`examples/apps/gtk4-notes`)

> Idioma: [English](README.md) | **Português (BR)**

Aplicação desktop nativa Linux feita com **GTK 4** (`PyGObject`) demonstrando `Gtk.ApplicationWindow`, `Gtk.Entry`, `Gtk.Button` e `Gtk.ListBox`, sem nenhum código do Rupicola dentro da aplicação.

## Como rodar

```sh
python3 protocol/examples/apps/gtk4-notes/app.py
```

Rodar em background headless com `Xvfb`:

```sh
Xvfb :99 -screen 0 1280x720x24 &
GDK_BACKEND=x11 DISPLAY=:99 python3 protocol/examples/apps/gtk4-notes/app.py
```

## Capturas geradas

![Preview do GTK4 Notes](media/preview.png)

![Componentes anotados no GTK4 Notes](media/preview-annotated.png)

- Vídeo capturado em background (`Xvfb` + `ffmpeg x11grab`): [walkthrough.mp4](media/walkthrough.mp4)
