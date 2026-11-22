#!/usr/bin/env python3
"""Native Linux GTK4 Notes application (zero Rupicola instrumentation)."""

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gio, Gtk  # noqa: E402


class NotesWindow(Gtk.ApplicationWindow):
    def __init__(self, app: Gtk.Application) -> None:
        super().__init__(application=app, title="GTK4 Notes · Rupicola Example")
        self.set_default_size(640, 480)

        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        root.set_margin_top(24)
        root.set_margin_bottom(24)
        root.set_margin_start(24)
        root.set_margin_end(24)
        self.set_child(root)

        header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        title = Gtk.Label(label="GTK4 Notes", xalign=0.0)
        title.add_css_class("title-1")
        title.set_hexpand(True)
        self.counter = Gtk.Label(label="2 notes", xalign=1.0)
        self.counter.add_css_class("dim-label")
        header.append(title)
        header.append(self.counter)
        root.append(header)

        row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self.entry = Gtk.Entry(placeholder_text="Write a quick note...")
        self.entry.set_hexpand(True)
        self.entry.connect("activate", self.on_add)
        add_btn = Gtk.Button(label="Add note")
        add_btn.add_css_class("suggested-action")
        add_btn.connect("clicked", self.on_add)
        row.append(self.entry)
        row.append(add_btn)
        root.append(row)

        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)
        self.listbox = Gtk.ListBox()
        self.listbox.set_selection_mode(Gtk.SelectionMode.NONE)
        self.listbox.add_css_class("boxed-list")
        scrolled.set_child(self.listbox)
        root.append(scrolled)

        footer = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        clear_btn = Gtk.Button(label="Clear all")
        clear_btn.connect("clicked", self.on_clear)
        footer.append(clear_btn)
        root.append(footer)

        self.notes = [
            "Capture native Linux GTK4 window in background",
            "Inspect layout bounds without modifying app code",
        ]
        self.render_notes()

    def render_notes(self) -> None:
        while (child := self.listbox.get_first_child()) is not None:
            self.listbox.remove(child)
        for text in self.notes:
            lbl = Gtk.Label(label=text, xalign=0.0)
            lbl.set_margin_top(10)
            lbl.set_margin_bottom(10)
            lbl.set_margin_start(12)
            lbl.set_margin_end(12)
            self.listbox.append(lbl)
        n = len(self.notes)
        self.counter.set_text(f"{n} note" if n == 1 else f"{n} notes")

    def on_add(self, _widget: Gtk.Widget) -> None:
        text = self.entry.get_text().strip()
        if not text:
            return
        self.notes.append(text)
        self.entry.set_text("")
        self.render_notes()

    def on_clear(self, _widget: Gtk.Widget) -> None:
        self.notes.clear()
        self.render_notes()


def main() -> None:
    app = Gtk.Application(
        application_id="dev.rupicola.example.Gtk4Notes",
        flags=Gio.ApplicationFlags.FLAGS_NONE,
    )
    app.connect("activate", lambda a: NotesWindow(a).present())
    app.run(None)


if __name__ == "__main__":
    main()
