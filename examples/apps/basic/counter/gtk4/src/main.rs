//! A native GTK4 counter, without protocol instrumentation.
use std::ffi::{c_char, c_int, c_void};
use std::ptr;

#[link(name = "gtk-4")]
#[link(name = "gio-2.0")]
#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn gtk_application_new(id: *const c_char, flags: c_int) -> *mut c_void;
    fn g_application_run(app: *mut c_void, argc: c_int, argv: *mut *mut c_char) -> c_int;
    fn g_object_unref(object: *mut c_void);
    fn g_signal_connect_data(
        instance: *mut c_void,
        signal: *const c_char,
        handler: *const c_void,
        data: *mut c_void,
        destroy: *const c_void,
        flags: c_int,
    ) -> usize;
    fn gtk_application_window_new(app: *mut c_void) -> *mut c_void;
    fn gtk_window_set_title(window: *mut c_void, title: *const c_char);
    fn gtk_window_set_default_size(window: *mut c_void, width: c_int, height: c_int);
    fn gtk_window_set_child(window: *mut c_void, child: *mut c_void);
    fn gtk_window_present(window: *mut c_void);
    fn gtk_box_new(orientation: c_int, spacing: c_int) -> *mut c_void;
    fn gtk_box_append(container: *mut c_void, child: *mut c_void);
    fn gtk_widget_set_margin_top(widget: *mut c_void, margin: c_int);
    fn gtk_widget_set_margin_bottom(widget: *mut c_void, margin: c_int);
    fn gtk_widget_set_margin_start(widget: *mut c_void, margin: c_int);
    fn gtk_widget_set_margin_end(widget: *mut c_void, margin: c_int);
    fn gtk_label_new(text: *const c_char) -> *mut c_void;
    fn gtk_label_set_text(label: *mut c_void, text: *const c_char);
    fn gtk_button_new_with_label(text: *const c_char) -> *mut c_void;
}

unsafe extern "C" fn increment(_: *mut c_void, label: *mut c_void) {
    // State belongs to the GTK main thread and lasts until application shutdown.
    unsafe {
        let value = gtk_count_get(label).saturating_add(1);
        gtk_count_set(label, value);
    }
}

unsafe fn gtk_count_get(label: *mut c_void) -> i32 {
    unsafe extern "C" {
        fn gtk_label_get_text(label: *mut c_void) -> *const c_char;
    }
    unsafe {
        std::ffi::CStr::from_ptr(gtk_label_get_text(label))
            .to_string_lossy()
            .parse()
            .unwrap_or(0)
    }
}
unsafe fn gtk_count_set(label: *mut c_void, value: i32) {
    let text = std::ffi::CString::new(value.to_string()).unwrap();
    unsafe {
        gtk_label_set_text(label, text.as_ptr());
    }
}
unsafe extern "C" fn reset(_: *mut c_void, label: *mut c_void) {
    unsafe {
        gtk_count_set(label, 0);
    }
}
unsafe extern "C" fn activate(app: *mut c_void, _: *mut c_void) {
    unsafe {
        let window = gtk_application_window_new(app);
        gtk_window_set_title(window, c"Counter".as_ptr());
        gtk_window_set_default_size(window, 320, 200);
        let root = gtk_box_new(1, 12);
        gtk_widget_set_margin_top(root, 24);
        gtk_widget_set_margin_bottom(root, 24);
        gtk_widget_set_margin_start(root, 24);
        gtk_widget_set_margin_end(root, 24);
        let label = gtk_label_new(c"0".as_ptr());
        let add = gtk_button_new_with_label(c"Increment".as_ptr());
        let clear = gtk_button_new_with_label(c"Reset".as_ptr());
        gtk_box_append(root, label);
        gtk_box_append(root, add);
        gtk_box_append(root, clear);
        g_signal_connect_data(
            add,
            c"clicked".as_ptr(),
            increment as *const c_void,
            label,
            ptr::null(),
            0,
        );
        g_signal_connect_data(
            clear,
            c"clicked".as_ptr(),
            reset as *const c_void,
            label,
            ptr::null(),
            0,
        );
        gtk_window_set_child(window, root);
        gtk_window_present(window);
    }
}
fn main() {
    unsafe {
        let app = gtk_application_new(c"org.rupicola.example.counter".as_ptr(), 0);
        g_signal_connect_data(
            app,
            c"activate".as_ptr(),
            activate as *const c_void,
            ptr::null_mut(),
            ptr::null(),
            0,
        );
        let status = g_application_run(app, 0, ptr::null_mut());
        g_object_unref(app);
        std::process::exit(status);
    }
}
