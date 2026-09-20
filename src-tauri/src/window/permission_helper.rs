#[cfg(target_os = "macos")]
#[allow(deprecated)]
mod macos {
    use std::ffi::CString;
    use std::sync::atomic::{AtomicPtr, Ordering};
    use std::sync::OnceLock;

    use cocoa::base::{id, nil};
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};

    const NS_BACKING_STORE_BUFFERED: usize = 2;
    const NS_WINDOW_STYLE_MASK_TITLED: usize = 1;
    const NS_WINDOW_STYLE_MASK_CLOSABLE: usize = 1 << 1;
    const NS_WINDOW_STYLE_MASK_UTILITY_WINDOW: usize = 1 << 4;
    const NS_FLOATING_WINDOW_LEVEL: i64 = 3;
    const NS_DRAG_OPERATION_COPY: usize = 1;
    const PANEL_WIDTH: f64 = 360.0;
    const PANEL_HEIGHT: f64 = 178.0;

    static DRAG_VIEW_CLASS: OnceLock<usize> = OnceLock::new();
    static HELPER_PANEL: AtomicPtr<Object> = AtomicPtr::new(std::ptr::null_mut());
    static HELPER_TITLE: AtomicPtr<Object> = AtomicPtr::new(std::ptr::null_mut());

    extern "C" fn drag_view_mouse_down(view: &Object, _selector: Sel, event: id) {
        unsafe {
            let app_url = *view.get_ivar::<id>("appURL");
            if app_url == nil {
                return;
            }

            let dragging_item: id = msg_send![class!(NSDraggingItem), alloc];
            let dragging_item: id = msg_send![dragging_item, initWithPasteboardWriter: app_url];
            if dragging_item == nil {
                return;
            }

            let bounds: CGRect = msg_send![view, bounds];
            let image: id = msg_send![view, image];
            let _: () = msg_send![dragging_item, setDraggingFrame: bounds contents: image];
            let items: id = msg_send![class!(NSArray), arrayWithObject: dragging_item];
            let _: id =
                msg_send![view, beginDraggingSessionWithItems: items event: event source: view];
            let _: () = msg_send![dragging_item, release];
        }
    }

    extern "C" fn drag_operation(
        _view: &Object,
        _selector: Sel,
        _session: id,
        _context: usize,
    ) -> usize {
        NS_DRAG_OPERATION_COPY
    }

    fn drag_view_class() -> Option<&'static Class> {
        let class_address = *DRAG_VIEW_CLASS.get_or_init(|| {
            if let Some(existing_class) = Class::get("OvimPermissionDragView") {
                return existing_class as *const Class as usize;
            }

            let superclass = class!(NSImageView);
            let Some(mut declaration) = ClassDecl::new("OvimPermissionDragView", superclass) else {
                return 0;
            };

            unsafe {
                declaration.add_ivar::<id>("appURL");
                declaration.add_method(
                    sel!(mouseDown:),
                    drag_view_mouse_down as extern "C" fn(&Object, Sel, id),
                );
                declaration.add_method(
                    sel!(draggingSession:sourceOperationMaskForDraggingContext:),
                    drag_operation as extern "C" fn(&Object, Sel, id, usize) -> usize,
                );
            }

            declaration.register() as *const Class as usize
        });

        if class_address == 0 {
            None
        } else {
            Some(unsafe { &*(class_address as *const Class) })
        }
    }

    unsafe fn ns_string(value: &str) -> Result<id, String> {
        let value = CString::new(value).map_err(|error| error.to_string())?;
        let string: id = msg_send![class!(NSString), stringWithUTF8String: value.as_ptr()];
        Ok(string)
    }

    unsafe fn installed_app_url() -> Result<id, String> {
        let main_bundle: id = msg_send![class!(NSBundle), mainBundle];
        let bundle_url: id = msg_send![main_bundle, bundleURL];
        let extension: id = msg_send![bundle_url, pathExtension];
        let app_extension = ns_string("app")?;
        let extension_comparison: i64 = msg_send![extension, caseInsensitiveCompare: app_extension];
        if extension_comparison == 0 {
            return Ok(bundle_url);
        }

        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let bundle_identifier = ns_string("com.tonis.ovim")?;
        let installed_url: id =
            msg_send![workspace, URLForApplicationWithBundleIdentifier: bundle_identifier];
        if installed_url == nil {
            return Err("Install ovim in Applications before using the drag helper".to_string());
        }

        Ok(installed_url)
    }

    unsafe fn label(frame: CGRect, text: &str, font_size: f64, bold: bool) -> Result<id, String> {
        let field: id = msg_send![class!(NSTextField), alloc];
        let field: id = msg_send![field, initWithFrame: frame];
        if field == nil {
            return Err("Could not create permission helper label".to_string());
        }

        let _: () = msg_send![field, setBezeled: false];
        let _: () = msg_send![field, setDrawsBackground: false];
        let _: () = msg_send![field, setEditable: false];
        let _: () = msg_send![field, setSelectable: false];
        let _: () = msg_send![field, setStringValue: ns_string(text)?];
        let font: id = if bold {
            msg_send![class!(NSFont), boldSystemFontOfSize: font_size]
        } else {
            msg_send![class!(NSFont), systemFontOfSize: font_size]
        };
        let _: () = msg_send![field, setFont: font];
        Ok(field)
    }

    unsafe fn create_panel(app_url: id) -> Result<id, String> {
        let frame = CGRect::new(
            &CGPoint::new(0.0, 0.0),
            &CGSize::new(PANEL_WIDTH, PANEL_HEIGHT),
        );
        let style = NS_WINDOW_STYLE_MASK_TITLED
            | NS_WINDOW_STYLE_MASK_CLOSABLE
            | NS_WINDOW_STYLE_MASK_UTILITY_WINDOW;
        let panel: id = msg_send![class!(NSPanel), alloc];
        let panel: id = msg_send![
            panel,
            initWithContentRect: frame
            styleMask: style
            backing: NS_BACKING_STORE_BUFFERED
            defer: false
        ];
        if panel == nil {
            return Err("Could not create permission helper panel".to_string());
        }

        let _: () = msg_send![panel, setTitle: ns_string("Add ovim to System Settings")?];
        let _: () = msg_send![panel, setLevel: NS_FLOATING_WINDOW_LEVEL];
        let _: () = msg_send![panel, setReleasedWhenClosed: false];
        let _: () = msg_send![panel, setHidesOnDeactivate: false];
        let content_view: id = msg_send![panel, contentView];
        if content_view == nil {
            return Err("Permission helper panel has no content view".to_string());
        }

        let title = label(
            CGRect::new(&CGPoint::new(112.0, 103.0), &CGSize::new(220.0, 24.0)),
            "Accessibility permission",
            15.0,
            true,
        )?;
        let instruction = label(
            CGRect::new(&CGPoint::new(112.0, 55.0), &CGSize::new(220.0, 48.0)),
            "Drag the ovim icon into the app list, then enable its switch.",
            12.0,
            false,
        )?;
        let _: () = msg_send![instruction, setMaximumNumberOfLines: 3usize];
        let _: () = msg_send![instruction, setLineBreakMode: 0usize];

        let drag_frame = CGRect::new(&CGPoint::new(28.0, 50.0), &CGSize::new(64.0, 64.0));
        let drag_view_class = drag_view_class()
            .ok_or_else(|| "Could not register permission drag view class".to_string())?;
        let drag_view: id = msg_send![drag_view_class, alloc];
        let drag_view: id = msg_send![drag_view, initWithFrame: drag_frame];
        if drag_view == nil {
            return Err("Could not create permission drag view".to_string());
        }
        let retained_app_url: id = msg_send![app_url, retain];
        (*drag_view).set_ivar("appURL", retained_app_url);

        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app_path: id = msg_send![app_url, path];
        let icon: id = msg_send![workspace, iconForFile: app_path];
        let icon_size = CGSize::new(64.0, 64.0);
        let _: () = msg_send![icon, setSize: icon_size];
        let _: () = msg_send![drag_view, setImage: icon];
        let _: () = msg_send![drag_view, setImageScaling: 3usize];
        let _: () =
            msg_send![drag_view, setToolTip: ns_string("Drag ovim into the System Settings list")?];

        let _: () = msg_send![content_view, addSubview: title];
        let _: () = msg_send![content_view, addSubview: instruction];
        let _: () = msg_send![content_view, addSubview: drag_view];

        HELPER_TITLE.store(title, Ordering::Release);
        Ok(panel)
    }

    unsafe fn position_panel(panel: id) {
        let screen: id = msg_send![class!(NSScreen), mainScreen];
        if screen == nil {
            return;
        }

        let visible_frame: CGRect = msg_send![screen, visibleFrame];
        let origin = CGPoint::new(
            visible_frame.origin.x + visible_frame.size.width - PANEL_WIDTH - 24.0,
            visible_frame.origin.y + visible_frame.size.height - PANEL_HEIGHT - 24.0,
        );
        let _: () = msg_send![panel, setFrameOrigin: origin];
    }

    pub fn show(permission_name: &str) -> Result<(), String> {
        unsafe {
            let app_url = installed_app_url()?;
            let mut panel = HELPER_PANEL.load(Ordering::Acquire);
            if panel.is_null() {
                panel = create_panel(app_url)?;
                HELPER_PANEL.store(panel, Ordering::Release);
            }

            let title = HELPER_TITLE.load(Ordering::Acquire);
            if !title.is_null() {
                let title_text = format!("{permission_name} permission");
                let _: () = msg_send![title, setStringValue: ns_string(&title_text)?];
            }

            position_panel(panel);
            let _: () = msg_send![panel, orderFrontRegardless];
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
pub fn show_permission_drag_helper(permission_name: &str) -> Result<(), String> {
    macos::show(permission_name)
}

#[cfg(not(target_os = "macos"))]
pub fn show_permission_drag_helper(_permission_name: &str) -> Result<(), String> {
    Ok(())
}
