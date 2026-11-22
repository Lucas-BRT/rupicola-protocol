# Pomodoro Timer (`examples/apps/pomodoro-timer`)

> Idioma: [English](README.md) | **Português (BR)**

Aplicação clássica de timer Pomodoro demonstrando SVG animado (`stroke-dashoffset`), alternância de presets (`Focus` / `Short` / `Long`), estado temporal (`Start` / `Pause` / `Reset`) e troca de tema escuro/claro sem nenhuma instrumentação do Rupicola no código-fonte.

## Como rodar

```sh
python3 -m http.server 8081 --directory protocol/examples/apps/pomodoro-timer
```

Abra `http://127.0.0.1:8081` no navegador.

## Capturas geradas

![Preview da interface](media/preview.png)

![Preview com anotações de componentes](media/preview-annotated.png)

- Vídeo do fluxo interativo (timer em execução, troca de modo e tema): [walkthrough.mp4](media/walkthrough.mp4)
