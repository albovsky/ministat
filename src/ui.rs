//! The only module that owns AppKit objects. Everything runs on the main thread.
use crate::{
    cli::load_view,
    model::{Freshness, State, StatusView, compact_duration_label, duration_label},
};
use objc2::{
    AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::*;
use objc2_foundation::{
    MainThreadMarker, NSDictionary, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect,
    NSSize, NSString, NSTimer,
};
use std::{cell::RefCell, collections::HashMap, path::PathBuf};

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}
fn label(
    view: &NSView,
    title: &str,
    frame: NSRect,
    size: f64,
    secondary: bool,
    mtm: MainThreadMarker,
) -> Retained<NSTextField> {
    let field = NSTextField::labelWithString(&NSString::from_str(title), mtm);
    field.setFrame(frame);
    field.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
        size, 0.0,
    )));
    let color = if secondary {
        NSColor::secondaryLabelColor()
    } else {
        NSColor::labelColor()
    };
    field.setTextColor(Some(&color));
    // SAFETY: all views are main-thread AppKit objects owned throughout their use.
    view.addSubview(&field);
    field
}
fn gauge(view: &NSView, frame: NSRect, mtm: MainThreadMarker) -> Retained<NSProgressIndicator> {
    let bar = NSProgressIndicator::initWithFrame(NSProgressIndicator::alloc(mtm), frame);
    bar.setIndeterminate(false);
    bar.setMinValue(0.0);
    bar.setMaxValue(100.0);
    view.addSubview(&bar);
    bar
}

mod dashboard;
mod layout;
use dashboard::Dashboard;

struct Ivars {
    state_dir: PathBuf,
    preview: bool,
    dashboard: RefCell<Option<Dashboard>>,
}
define_class!(
    // SAFETY: NSObject permits subclassing; all AppKit state stays on the main thread.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    struct Delegate;
    unsafe impl NSObjectProtocol for Delegate {}
    unsafe impl NSApplicationDelegate for Delegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn launched(&self, _note: &NSNotification) {
            if self.ivars().preview {
                NSApplication::sharedApplication(self.mtm()).setActivationPolicy(NSApplicationActivationPolicy::Regular);
            }
            self.setup(); self.refresh();
            // SAFETY: tick: has the NSTimer callback signature and the timer retains its target.
            unsafe { NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                2.0, self, sel!(tick:), None, true).setTolerance(0.3); }
            if self.ivars().preview { self.show(); }
        }
    }
    impl Delegate {
        #[unsafe(method(tick:))]
        fn tick(&self, _timer: &NSTimer) { self.refresh(); }
        #[unsafe(method(toggle:))]
        fn toggle(&self, _sender: &NSObject) {
            if let Some(ui) = self.ivars().dashboard.borrow().as_ref()
                && ui.panel.isVisible() { ui.panel.orderOut(None); return; }
            self.refresh(); self.show();
        }
        #[unsafe(method(copyJSON:))]
        fn copy_json(&self, _sender: &NSObject) {
            if let Ok(view) = load_view(&self.ivars().state_dir)
                && let Ok(json) = serde_json::to_string_pretty(&view) {
                    let board = NSPasteboard::generalPasteboard();
                    board.clearContents();
                    unsafe { board.setString_forType(&NSString::from_str(&json), NSPasteboardTypeString); }
            }
        }
        #[unsafe(method(openLog:))]
        fn open_log(&self, _sender: &NSObject) {
            let _ = std::process::Command::new("/usr/bin/open").arg(self.ivars().state_dir.join("collector.log")).spawn();
        }
        #[unsafe(method(quit:))]
        fn quit(&self, _sender: &NSObject) {
            // SAFETY: normal AppKit termination on its main thread; collector remains independent.
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }
    }
);

impl Delegate {
    fn refresh(&self) {
        if let Some(ui) = self.ivars().dashboard.borrow().as_ref() {
            ui.render(load_view(&self.ivars().state_dir));
        }
    }
    fn show(&self) {
        if let Some(ui) = self.ivars().dashboard.borrow().as_ref() {
            let mtm = self.mtm();
            if let Some(button) = ui.item.button(mtm)
                && let Some(window) = button.window()
            {
                let anchor = window.frame();
                let screen = window
                    .screen()
                    .map(|s| s.visibleFrame())
                    .unwrap_or(rect(0.0, 0.0, 1440.0, 900.0));
                let x = (anchor.origin.x + anchor.size.width - 360.0).clamp(
                    screen.origin.x,
                    (screen.origin.x + screen.size.width - 360.0).max(screen.origin.x),
                );
                ui.panel
                    .setFrame_display(rect(x, anchor.origin.y - 546.0, 360.0, 540.0), true);
            }
            #[allow(deprecated)]
            NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
            ui.panel.makeKeyAndOrderFront(None);
        }
        self.refresh();
    }
    fn setup(&self) {
        *self.ivars().dashboard.borrow_mut() = Some(layout::build(self));
    }
}

pub fn run(state_dir: PathBuf, preview: bool) {
    let mtm = MainThreadMarker::new().expect("AppKit must run on main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let this = Delegate::alloc(mtm).set_ivars(Ivars {
        state_dir,
        preview,
        dashboard: RefCell::new(None),
    });
    let delegate: Retained<Delegate> = unsafe { msg_send![super(this), init] };
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();
}
