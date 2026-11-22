//! Native Linux GTK4 Notes application in pure Rust (zero external crates, zero Rupicola code).

use std::ffi::{CString, c_char, c_int, c_void};
use std::ptr;

#[link(name = "gtk-4")]
#[link(name = "gio-2.0")]
#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn gtk_application_new(application_id: *const c_char, flags: c_int) -> *mut c_void;
    fn g_application_run(app: *mut c_void, argc: c_int, argv: *mut *mut c_char) -> c_int;
    fn g_object_unref(obj: *mut c_void);
    fn g_signal_connect_data(
        instance: *mut c_void,
        detailed_signal: *const c_char,
        c_handler: *const c_void,
        data: *mut c_void,
        destroy_data: *const c_void,
        connect_flags: c_int,
    ) -> usize;

    fn gtk_application_window_new(app: *mut c_void) -> *mut c_void;
    fn gtk_window_set_title(window: *mut c_void, title: *const c_char);
    fn gtk_window_set_default_size(window: *mut c_void, width: c_int, height: c_int);
    fn gtk_window_set_child(window: *mut c_void, child: *mut c_void);
    fn gtk_window_present(window: *mut c_void);

    fn gtk_box_new(orientation: c_int, spacing: c_int) -> *mut c_void;
    fn gtk_box_append(box_w: *mut c_void, child: *mut c_void);
    fn gtk_widget_set_margin_top(widget: *mut c_void, margin: c_int);
    fn gtk_widget_set_margin_bottom(widget: *mut c_void, margin: c_int);
    fn gtk_widget_set_margin_start(widget: *mut c_void, margin: c_int);
    fn gtk_widget_set_margin_end(widget: *mut c_void, margin: c_int);
    fn gtk_widget_set_hexpand(widget: *mut c_void, expand: c_int);
    fn gtk_widget_set_vexpand(widget: *mut c_void, expand: c_int);
    fn gtk_widget_add_css_class(widget: *mut c_void, css_class: *const c_char);

    fn gtk_label_new(str_: *const c_char) -> *mut c_void;
    fn gtk_label_set_xalign(label: *mut c_void, xalign: f32);
    fn gtk_entry_new() -> *mut c_void;
    fn gtk_entry_set_placeholder_text(entry: *mut c_void, text: *const c_char);
    fn gtk_button_new_with_label(label: *const c_char) -> *mut c_void;
    fn gtk_list_box_new() -> *mut c_void;
    fn gtk_list_box_append(box_w: *mut c_void, child: *mut c_void);
    fn gtk_scrolled_window_new() -> *mut c_void;
    fn gtk_scrolled_window_set_child(scrolled: *mut c_void, child: *mut c_void);
}

const GTK_ORIENTATION_HORIZONTAL: c_int = 0;
const GTK_ORIENTATION_VERTICAL: c_int = 1;

fn c(s: &str) -> CString {
    CString::new(s).expect("valid C string")
}

unsafe extern "C" fn on_activate(app: *mut c_void, _user_data: *mut c_void) {
    unsafe {
        let win = gtk_application_window_new(app);
        gtk_window_set_title(win, c("GTK4 Notes · Rupicola Example").as_ptr());
        gtk_window_set_default_size(win, 640, 480);

        let root = gtk_box_new(GTK_ORIENTATION_VERTICAL, 12);
        gtk_widget_set_margin_top(root, 24);
        gtk_widget_set_margin_bottom(root, 24);
        gtk_widget_set_margin_start(root, 24);
        gtk_widget_set_margin_end(root, 24);
        gtk_window_set_child(win, root);

        let header = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
        let title = gtk_label_new(c("GTK4 Notes").as_ptr());
        gtk_label_set_xalign(title, 0.0);
        gtk_widget_set_hexpand(title, 1);
        gtk_widget_add_css_class(title, c("title-1").as_ptr());
        let counter = gtk_label_new(c("2 notes").as_ptr());
        gtk_widget_add_css_class(counter, c("dim-label").as_ptr());
        gtk_box_append(header, title);
        gtk_box_append(header, counter);
        gtk_box_append(root, header);

        let row = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
        let entry = gtk_entry_new();
        gtk_entry_set_placeholder_text(entry, c("Write a quick note...").as_ptr());
        gtk_widget_set_hexpand(entry, 1);
        let add_btn = gtk_button_new_with_label(c("Add note").as_ptr());
        gtk_widget_add_css_class(add_btn, c("suggested-action").as_ptr());
        gtk_box_append(row, entry);
        gtk_box_append(row, add_btn);
        gtk_box_append(root, row);

        let scrolled = gtk_scrolled_window_new();
        gtk_widget_set_vexpand(scrolled, 1);
        let listbox = gtk_list_box_new();
        gtk_widget_add_css_class(listbox, c("boxed-list").as_ptr());
        for note in [
            "Capture native Linux GTK4 window in background",
            "Inspect layout bounds without modifying app code",
        ] {
            let lbl = gtk_label_new(c(note).as_ptr());
            gtk_label_set_xalign(lbl, 0.0);
            gtk_widget_set_margin_top(lbl, 10);
            gtk_widget_set_margin_bottom(lbl, 10);
            gtk_widget_set_margin_start(lbl, 12);
            gtk_widget_set_margin_end(lbl, 12);
            gtk_list_box_append(listbox, lbl);
        }
        gtk_scrolled_window_set_child(scrolled, listbox);
        gtk_box_append(root, scrolled);

        let footer = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
        let clear_btn = gtk_button_new_with_label(c("Clear all").as_ptr());
        gtk_box_append(footer, clear_btn);
        gtk_box_append(root, footer);

        gtk_window_present(win);
    }
}

fn main() {
    unsafe {
        let app = gtk_application_new(c("dev.rupicola.example.Gtk4Notes").as_ptr(), 0);
        g_signal_connect_data(
            app,
            c("activate").as_ptr(),
            on_activate as *const c_void,
            ptr::null_mut(),
            ptr::null(),
            0,
        );
        let status = g_application_run(app, 0, ptr::null_mut());
        g_object_unref(app);
        std::process::exit(status);
    }
}
