use crate::core::{paint::Color, scene::linear_to_srgb, theme::Theme};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{ClassType, DeclaredClass, declare_class, msg_send, msg_send_id, mutability};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSColor, NSRectFill, NSTextField, NSView};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSString};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

declare_class!(
    struct StartupView;

    unsafe impl ClassType for StartupView {
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "HalcyonStartupView";
    }

    impl DeclaredClass for StartupView {
        type Ivars = Retained<NSColor>;
    }

    unsafe impl StartupView {
        #[method(drawRect:)]
        fn draw_rect(&self, dirty: NSRect) {
            // AppKit provides the current graphics context for this callback.
            unsafe {
                self.ivars().setFill();
                NSRectFill(dirty);
            }
        }

        #[method(hitTest:)]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }
    }
);

/// Owns the passive native startup view on AppKit's main thread.
pub(crate) struct NativeStartupPresentation(Retained<StartupView>);

impl NativeStartupPresentation {
    pub(crate) fn install(window: &winit::window::Window, theme: &Theme) -> Self {
        let mtm = MainThreadMarker::new().expect("macOS startup view requires the main thread");
        let handle = window.window_handle().expect("live macOS startup window");
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            unreachable!("macOS window supplies an AppKit view");
        };
        // Winit keeps this NSView alive for the borrowed AppKit window handle.
        let client: &NSView = unsafe { handle.ns_view.cast().as_ref() };
        let bounds = client.bounds();
        let background = native_color(theme.colors.surfaces.base0);
        let view: Retained<StartupView> =
            unsafe { msg_send_id![super(mtm.alloc().set_ivars(background)), initWithFrame: bounds] };
        let label = unsafe { NSTextField::labelWithString(&NSString::from_str("Loading…"), mtm) };
        let foreground = native_color(theme.colors.typography.primary);
        // The overlay and label belong to the live main-thread client view.
        unsafe {
            view.setAutoresizingMask(
                NSAutoresizingMaskOptions::NSViewWidthSizable
                    | NSAutoresizingMaskOptions::NSViewHeightSizable,
            );
            label.setTextColor(Some(&foreground));
            label.sizeToFit();
            let size = label.frame().size;
            label.setFrameOrigin(NSPoint::new(
                (bounds.size.width - size.width) / 2.0,
                (bounds.size.height - size.height) / 2.0,
            ));
            label.setAutoresizingMask(
                NSAutoresizingMaskOptions::NSViewMinXMargin
                    | NSAutoresizingMaskOptions::NSViewMaxXMargin
                    | NSAutoresizingMaskOptions::NSViewMinYMargin
                    | NSAutoresizingMaskOptions::NSViewMaxYMargin,
            );
            view.addSubview(&label);
            view.setWantsLayer(true);
            client.addSubview(&view);
            // AppKit owns this view's backing layer. Keep its ordinary drawing
            // above the wgpu sublayer that the backend will append later.
            let backing: Retained<AnyObject> = msg_send_id![&*view, layer];
            let _: () = msg_send![&*backing, setZPosition: 1.0f64];
        }
        tracing::debug!(target: "halcyon::native_startup", event = "loading_installed");
        Self(view)
    }
}

impl Drop for NativeStartupPresentation {
    fn drop(&mut self) {
        unsafe { self.0.removeFromSuperview() };
        tracing::debug!(target: "halcyon::native_startup", event = "loading_retired");
    }
}

fn native_color(color: Color) -> Retained<NSColor> {
    let [red, green, blue] = [color.r, color.g, color.b].map(|c| f64::from(linear_to_srgb(c)));
    unsafe { NSColor::colorWithSRGBRed_green_blue_alpha(red, green, blue, f64::from(color.a)) }
}
