# Pomodoro Timer (`examples/apps/pomodoro-timer`)

> Language: **English** | [Português (BR)](README.pt-BR.md)

Classic Pomodoro timer application demonstrating animated SVG progress (`stroke-dashoffset`), preset switching (`Focus` / `Short` / `Long`), timer state (`Start` / `Pause` / `Reset`), and light/dark theme toggling without any Rupicola instrumentation in the app source.

## How to run

```sh
python3 -m http.server 8081 --directory protocol/examples/apps/pomodoro-timer
```

Open `http://127.0.0.1:8081` in your browser.

## Captured Media

![UI preview](media/preview.png)

![Annotated component boxes](media/preview-annotated.png)

- Interactive walkthrough video (running timer, mode switch, theme toggle): [walkthrough.mp4](media/walkthrough.mp4)
