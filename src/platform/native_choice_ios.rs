//! UIKit presentation for an already qualified HAL-400 lease.

use crate::core::event_router::host_choice::{ChoiceLease, ChoiceReply, ChoiceRequest};
use crate::widgets::combo_box::ChoiceDisplayRow;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject, Sel};
use objc2::{ClassType, DeclaredClass, declare_class, msg_send, msg_send_id, mutability, sel};
use objc2_foundation::{MainThreadMarker, NSAttributedString, NSDictionary, NSString};
use objc2_ui_kit::{
    NSForegroundColorAttributeName, UIBarButtonItem, UIBarButtonSystemItem, UIColor,
    UIModalPresentationStyle, UINavigationController, UIPickerView, UIPickerViewDataSource,
    UIPickerViewDelegate, UIView, UIViewController,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use winit::event_loop::EventLoopProxy;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

struct PickerData {
    picker: Retained<UIPickerView>,
    request: ChoiceRequest,
    reply: Rc<RefCell<Option<ChoiceReply>>>,
    wake: EventLoopProxy<()>,
    terminal: Cell<bool>,
}

declare_class!(
    struct HalcyonChoiceTarget;

    unsafe impl ClassType for HalcyonChoiceTarget {
        type Super = NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "HalcyonChoiceTarget";
    }

    impl DeclaredClass for HalcyonChoiceTarget {
        type Ivars = PickerData;
    }

    unsafe impl NSObjectProtocol for HalcyonChoiceTarget {
        #[allow(non_snake_case, reason = "Required objc2 Objective-C protocol method spelling")]
        #[method(respondsToSelector:)]
        fn respondsToSelector(&self, selector: Sel) -> bool {
            if selector == sel!(pickerView:attributedTitleForRow:forComponent:) {
                // Flat pickers still use UIKit's plain title delegate.
                self.ivars().request.groups.is_some()
            } else {
                unsafe { msg_send![super(self), respondsToSelector: selector] }
            }
        }
    }

    unsafe impl UIPickerViewDataSource for HalcyonChoiceTarget {
        #[allow(non_snake_case, reason = "Required objc2 Objective-C protocol method spelling")]
        #[method(numberOfComponentsInPickerView:)]
        fn numberOfComponentsInPickerView(&self, _picker: &UIPickerView) -> isize { 1 }

        #[allow(non_snake_case, reason = "Required objc2 Objective-C protocol method spelling")]
        #[method(pickerView:numberOfRowsInComponent:)]
        fn pickerView_numberOfRowsInComponent(&self, _picker: &UIPickerView, _component: isize) -> isize {
            self.ivars().request.picker_row_count() as isize
        }
    }

    unsafe impl UIPickerViewDelegate for HalcyonChoiceTarget {
        #[allow(non_snake_case, reason = "Required objc2 Objective-C protocol method spelling")]
        #[method_id(pickerView:titleForRow:forComponent:)]
        fn pickerView_titleForRow_forComponent(
            &self, _picker: &UIPickerView, row: isize, _component: isize,
        ) -> Option<Retained<NSString>> {
            let request = &self.ivars().request;
            request.picker_row(row).map(|row| NSString::from_str(request.picker_row_title(row)))
        }

        #[allow(non_snake_case, reason = "Required objc2 Objective-C protocol method spelling")]
        #[method_id(pickerView:attributedTitleForRow:forComponent:)]
        fn pickerView_attributedTitleForRow_forComponent(
            &self, _picker: &UIPickerView, row: isize, _component: isize,
        ) -> Option<Retained<NSAttributedString>> {
            let request = &self.ivars().request;
            request.picker_row(row).map(|row| {
                let title = NSString::from_str(request.picker_row_title(row));
                if matches!(row, ChoiceDisplayRow::Header(_)) {
                    let color = unsafe { UIColor::grayColor() };
                    let color = Retained::into_super(Retained::into_super(color));
                    let attrs = NSDictionary::from_id_slice(
                        &[unsafe { NSForegroundColorAttributeName }],
                        &[color],
                    );
                    unsafe {
                        NSAttributedString::initWithString_attributes(
                            NSAttributedString::alloc(), &title, Some(&attrs),
                        )
                    }
                } else {
                    NSAttributedString::initWithString(NSAttributedString::alloc(), &title)
                }
            })
        }
    }

    unsafe impl HalcyonChoiceTarget {
        #[method(halcyonChoiceDone:)]
        fn done(&self, _sender: &UIBarButtonItem) {
            let row = unsafe { self.ivars().picker.selectedRowInComponent(0) };
            self.send_reply(Some(row));
        }

        #[method(halcyonChoiceCancel:)]
        fn cancel(&self, _sender: &UIBarButtonItem) {
            self.send_reply(None);
        }
    }
);

impl HalcyonChoiceTarget {
    fn send_reply(&self, row: Option<isize>) {
        let data = self.ivars();
        if !data.terminal.replace(true) {
            data.reply.replace(Some(data.request.picker_reply(row)));
            let _ = data.wake.send_event(());
        }
    }
}

struct Presentation {
    controller: Retained<UINavigationController>,
    target: Retained<HalcyonChoiceTarget>,
}

pub(crate) struct NativeChoiceHost {
    active: Option<Presentation>,
    reply: Rc<RefCell<Option<ChoiceReply>>>,
    wake: EventLoopProxy<()>,
}

impl NativeChoiceHost {
    pub(crate) fn new(wake: EventLoopProxy<()>) -> Self {
        Self {
            active: None,
            reply: Rc::default(),
            wake,
        }
    }

    fn presenter(window: &Window) -> Option<Retained<UIViewController>> {
        let RawWindowHandle::UiKit(handle) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // Winit retains this UIView for the lifetime of its window handle.
        let view = unsafe { &*handle.ui_view.as_ptr().cast::<UIView>() };
        let controller = view.window()?.rootViewController()?;
        unsafe { controller.presentedViewController() }
            .is_none()
            .then_some(controller)
    }

    pub(crate) fn available(&self, window: &Window) -> bool {
        UIViewController::class().responds_to(sel!(setModalInPresentation:))
            && MainThreadMarker::new().is_some()
            && self.active.is_none()
            && Self::presenter(window).is_some()
    }

    pub(crate) fn present(&mut self, window: &Window, request: ChoiceRequest) -> bool {
        let Some(mtm) = MainThreadMarker::new() else {
            return false;
        };
        let Some(presenter) = Self::presenter(window) else {
            return false;
        };
        let Some(view) = presenter.view() else {
            return false;
        };
        let initial_row = request.initial_picker_row();
        let picker = unsafe { UIPickerView::initWithFrame(mtm.alloc(), view.bounds()) };
        let target = mtm.alloc::<HalcyonChoiceTarget>().set_ivars(PickerData {
            picker: picker.clone(),
            request,
            reply: self.reply.clone(),
            wake: self.wake.clone(),
            terminal: Cell::new(false),
        });
        let target: Retained<HalcyonChoiceTarget> = unsafe { msg_send_id![super(target), init] };
        let content: Retained<UIViewController> = unsafe { UIViewController::init(mtm.alloc()) };
        content.setView(Some(&picker));
        if UIColor::class().metaclass().responds_to(sel!(systemBackgroundColor)) {
            let background: Retained<UIColor> = unsafe { msg_send_id![UIColor::class(), systemBackgroundColor] };
            picker.setBackgroundColor(Some(&background));
        }
        let controller = unsafe {
            picker.setDataSource(Some(ProtocolObject::from_ref(&*target)));
            picker.setDelegate(Some(ProtocolObject::from_ref(&*target)));
            picker.selectRow_inComponent_animated(initial_row as isize, 0, false);
            let cancel = UIBarButtonItem::initWithBarButtonSystemItem_target_action(
                mtm.alloc(),
                UIBarButtonSystemItem::Cancel,
                Some(&target),
                Some(sel!(halcyonChoiceCancel:)),
            );
            let done = UIBarButtonItem::initWithBarButtonSystemItem_target_action(
                mtm.alloc(),
                UIBarButtonSystemItem::Done,
                Some(&target),
                Some(sel!(halcyonChoiceDone:)),
            );
            let navigation_item = content.navigationItem();
            navigation_item.setLeftBarButtonItem(Some(&cancel));
            navigation_item.setRightBarButtonItem(Some(&done));
            let controller =
                UINavigationController::initWithRootViewController(mtm.alloc(), &content);
            controller.setModalPresentationStyle(UIModalPresentationStyle::UIModalPresentationPageSheet);
            controller.setModalInPresentation(true);
            controller
        };
        self.active = Some(Presentation {
            controller: controller.clone(),
            target,
        });
        unsafe { presenter.presentViewController_animated_completion(&controller, true, None) };
        true
    }

    pub(crate) fn take_reply(&self) -> Option<ChoiceReply> {
        self.reply.borrow_mut().take()
    }

    pub(crate) fn retain(&mut self, live: Option<&ChoiceLease>) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| live != Some(&active.target.ivars().request.lease))
        {
            let active = self.active.take().unwrap();
            let data = active.target.ivars();
            data.terminal.set(true);
            unsafe {
                data.picker.setDataSource(None);
                data.picker.setDelegate(None);
                active
                    .controller
                    .dismissViewControllerAnimated_completion(true, None);
            }
        }
    }
}

impl Drop for NativeChoiceHost {
    fn drop(&mut self) {
        self.retain(None);
    }
}
