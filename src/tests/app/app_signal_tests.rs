    use super::*;
    use crate::core::event::WindowId;
    use crate::core::lifecycle::LifecycleRegistry;
    use crate::core::signal_bus::{SignalEnvelope, SignalPayload};
    use crate::core::widget::Widget;
    use std::cell::Cell;
    use std::rc::Rc;

    struct SignalScreen {
        handled_count: Rc<Cell<u32>>,
        host: ScreenHost,
    }

    impl SignalScreen {
        fn new(handled_count: Rc<Cell<u32>>) -> Self {
            Self {
                handled_count,
                host: ScreenHost::new(),
            }
        }
    }

    impl Screen for SignalScreen {
        fn build<'a>(&'a mut self, _cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            &mut self.host
        }

        fn handle_signal(
            &mut self,
            signal: &crate::core::signal_bus::SignalEnvelope,
            _cx: &mut ScreenContext<'_>,
        ) -> bool {
            if signal.payload == SignalPayload::SettingsChanged {
                self.handled_count.set(self.handled_count.get() + 1);
                return true;
            }
            false
        }
    }

    #[test]
    fn app_context_signal_bus_delivers_for_alive_primary_window() {
        let mut context = AppContext::new(Theme::dark());
        let mut lifecycle = LifecycleRegistry::new();
        assert!(lifecycle.register_window(WindowId::PRIMARY));
        assert!(lifecycle.activate_window(WindowId::PRIMARY));

        context.signal_bus.enqueue(SignalEnvelope::system_to_window(
            WindowId::PRIMARY,
            SignalPayload::SettingsChanged,
        ));

        let mut delivered = Vec::new();
        deliver_signals(&mut context.signal_bus, &lifecycle, |envelope| {
            delivered.push(envelope.payload);
        });

        assert_eq!(delivered, vec![SignalPayload::SettingsChanged]);
        assert!(context.signal_bus.is_empty());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "INV-XWIN-003")]
    fn inv_xwin_003_signal_delivery_forbidden_inside_frame_critical_section() {
        let mut context = AppContext::new(Theme::dark());
        let mut lifecycle = LifecycleRegistry::new();
        assert!(lifecycle.register_window(WindowId::PRIMARY));
        assert!(lifecycle.activate_window(WindowId::PRIMARY));
        context.signal_bus.enqueue(SignalEnvelope::system_to_window(
            WindowId::PRIMARY,
            SignalPayload::SettingsChanged,
        ));

        let _frame_critical_section = FrameCriticalSectionGuard::enter();
        deliver_signals(&mut context.signal_bus, &lifecycle, |_| {});
    }

    #[test]
    fn app_root_delivers_signal_to_base_screen() {
        let handled_count = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut app_root = AppRoot::new(AppContext::new(Theme::dark()), root);
        app_root.push_screen(Box::new(SignalScreen::new(Rc::clone(&handled_count))));

        let signal =
            SignalEnvelope::system_to_window(WindowId::PRIMARY, SignalPayload::SettingsChanged);

        assert!(app_root.deliver_signal_modal_first(&signal, &mut tree));
        assert_eq!(handled_count.get(), 1);
    }
