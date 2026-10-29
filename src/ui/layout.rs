use super::*;

pub(super) fn build(delegate: &Delegate) -> Dashboard {
    let mtm = delegate.mtm();
    let item = NSStatusBar::systemStatusBar().statusItemWithLength(190.0);
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        rect(0.0, 0.0, 360.0, 518.0),
        NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::FullSizeContentView,
        NSBackingStoreType::Buffered,
        false,
    );
    unsafe {
        panel.setReleasedWhenClosed(false);
    }
    panel.setTitle(&NSString::from_str("Ministat"));
    panel.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    panel.setTitlebarAppearsTransparent(true);
    panel.setFloatingPanel(true);
    panel.setHidesOnDeactivate(!delegate.ivars().preview);
    panel.setMovableByWindowBackground(true);
    let content = NSVisualEffectView::initWithFrame(
        NSVisualEffectView::alloc(mtm),
        rect(0.0, 0.0, 360.0, 518.0),
    );
    content.setMaterial(NSVisualEffectMaterial::Popover);
    content.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    content.setState(NSVisualEffectState::Active);
    panel.setContentView(Some(&content));
    let mut fields = HashMap::new();
    let mut bars = HashMap::new();
    if let Some(button) = item.button(mtm) {
        button.setTitle(&NSString::from_str(""));
        unsafe {
            button.setTarget(Some(delegate));
            button.setAction(Some(sel!(toggle:)));
        }
    }
    label(
        &content,
        "Ministat",
        rect(24.0, 464.0, 180.0, 30.0),
        22.0,
        false,
        mtm,
    );
    fields.insert(
        "live",
        label(
            &content,
            "STARTING",
            rect(260.0, 472.0, 82.0, 16.0),
            10.0,
            true,
            mtm,
        ),
    );
    fields.insert(
        "subtitle",
        label(
            &content,
            "Waiting for collector",
            rect(24.0, 444.0, 320.0, 18.0),
            10.0,
            true,
            mtm,
        ),
    );
    for (key, title, x) in [("cpu", "CPU", 24.0), ("memory", "MEMORY", 192.0)] {
        label(
            &content,
            title,
            rect(x, 414.0, 140.0, 16.0),
            10.0,
            true,
            mtm,
        );
        let value_key = if key == "cpu" {
            "cpu_value"
        } else {
            "ram_value"
        };
        fields.insert(
            value_key,
            label(&content, "—", rect(x, 374.0, 140.0, 42.0), 32.0, false, mtm),
        );
        bars.insert(key, gauge(&content, rect(x, 366.0, 140.0, 5.0), mtm));
    }
    fields.insert(
        "memory_detail",
        label(
            &content,
            "",
            rect(192.0, 345.0, 145.0, 16.0),
            10.0,
            true,
            mtm,
        ),
    );
    fields.insert(
        "trend",
        label(&content, "", rect(24.0, 343.0, 155.0, 17.0), 9.0, true, mtm),
    );
    label(
        &content,
        "STORAGE",
        rect(24.0, 313.0, 300.0, 16.0),
        10.0,
        true,
        mtm,
    );
    for (key, title, detail, y) in [
        ("internal", "Internal SSD", "internal_detail", 286.0),
        ("external", "Media SSD", "external_detail", 231.0),
    ] {
        label(
            &content,
            title,
            rect(24.0, y, 120.0, 18.0),
            12.0,
            false,
            mtm,
        );
        fields.insert(
            detail,
            label(&content, "—", rect(144.0, y, 192.0, 18.0), 11.0, true, mtm),
        );
        bars.insert(key, gauge(&content, rect(24.0, y - 13.0, 312.0, 5.0), mtm));
    }
    label(
        &content,
        "SERVICES",
        rect(24.0, 188.0, 300.0, 16.0),
        10.0,
        true,
        mtm,
    );
    for (key, title, y) in [
        ("homekit", "HomeKit", 162.0),
        ("jellyfin", "Jellyfin", 136.0),
        ("ci", "CI", 110.0),
    ] {
        label(&content, title, rect(24.0, y, 76.0, 18.0), 12.0, false, mtm);
        fields.insert(
            key,
            label(
                &content,
                "○  Checking",
                rect(102.0, y, 238.0, 18.0),
                11.0,
                true,
                mtm,
            ),
        );
    }
    label(
        &content,
        "CI checks local runners; GitHub connectivity unverified",
        rect(24.0, 88.0, 320.0, 16.0),
        9.0,
        true,
        mtm,
    );
    fields.insert(
        "pressure",
        label(&content, "", rect(24.0, 62.0, 312.0, 18.0), 11.0, true, mtm),
    );
    for (title, action, x, w) in [
        ("Copy JSON", sel!(copyJSON:), 24.0, 100.0),
        ("Open log", sel!(openLog:), 132.0, 100.0),
        ("Quit", sel!(quit:), 260.0, 76.0),
    ] {
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(title),
                Some(delegate),
                Some(action),
                mtm,
            )
        };
        button.setFrame(rect(x, 18.0, w, 28.0));
        button.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        content.addSubview(&button);
    }
    Dashboard {
        item,
        panel,
        fields,
        bars,
        last_status: RefCell::new(Vec::new()),
    }
}
