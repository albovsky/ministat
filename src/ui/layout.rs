use super::*;

pub(super) const WIDTH: f64 = 360.0;
pub(super) const HEIGHT: f64 = 504.0;
const MARGIN: f64 = 20.0;
const INNER: f64 = WIDTH - 2.0 * MARGIN;
const COLUMN: f64 = 150.0;
const RIGHT_COLUMN: f64 = WIDTH - MARGIN - COLUMN;
const BAR: f64 = 6.0;
const ROW: f64 = 24.0;
pub(super) const SERVICES: [(&str, &str); 5] = [
    ("homekit", "HomeKit"),
    ("jellyfin", "Jellyfin"),
    ("ci", "CI"),
    ("hermes", "Hermes"),
    ("tailscale", "Tailscale"),
];
const CI_NOTE: &str = "Checks the local runners only; GitHub connectivity is not verified.";

/// A frame measured down from the panel's top edge.
fn top(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    rect(x, HEIGHT - y - h, w, h)
}
fn heading(
    view: &NSView,
    title: &str,
    frame: NSRect,
    mtm: MainThreadMarker,
) -> Retained<NSTextField> {
    label(view, title, frame, 10.0, 0.5, Tone::Secondary, mtm)
}
fn separator(view: &NSView, y: f64, mtm: MainThreadMarker) {
    let line = NSBox::initWithFrame(NSBox::alloc(mtm), top(MARGIN, y, INNER, 1.0));
    line.setBoxType(NSBoxType::Separator);
    view.addSubview(&line);
}
fn dot(view: &NSView, frame: NSRect, mtm: MainThreadMarker) -> Retained<NSImageView> {
    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(8.0, 8.0));
    #[allow(deprecated)]
    image.lockFocus();
    NSColor::blackColor().set();
    NSBezierPath::bezierPathWithOvalInRect(rect(0.0, 0.0, 8.0, 8.0)).fill();
    #[allow(deprecated)]
    image.unlockFocus();
    image.setTemplate(true);
    let icon = NSImageView::initWithFrame(NSImageView::alloc(mtm), frame);
    icon.setImage(Some(&image));
    icon.setContentTintColor(Some(&NSColor::tertiaryLabelColor()));
    view.addSubview(&icon);
    icon
}

pub(super) fn build(delegate: &Delegate) -> Dashboard {
    let mtm = delegate.mtm();
    let item = NSStatusBar::systemStatusBar().statusItemWithLength(190.0);
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        rect(0.0, 0.0, WIDTH, HEIGHT),
        NSWindowStyleMask::Titled | NSWindowStyleMask::FullSizeContentView,
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
        rect(0.0, 0.0, WIDTH, HEIGHT),
    );
    content.setMaterial(NSVisualEffectMaterial::Popover);
    content.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    content.setState(NSVisualEffectState::Active);
    panel.setContentView(Some(&content));
    if let Some(button) = item.button(mtm) {
        button.setTitle(&NSString::from_str(""));
        unsafe {
            button.setTarget(Some(delegate));
            button.setAction(Some(sel!(toggle:)));
        }
    }
    let mut fields = HashMap::new();
    let mut bars = HashMap::new();
    let mut dots = HashMap::new();

    // Header: host name, freshness badge, uptime line.
    fields.insert(
        "host",
        label(
            &content,
            "Ministat",
            top(MARGIN, 18.0, 230.0, 20.0),
            15.0,
            0.5,
            Tone::Primary,
            mtm,
        ),
    );
    let live = label(
        &content,
        "STARTING",
        top(250.0, 22.0, 90.0, 14.0),
        10.0,
        0.6,
        Tone::Secondary,
        mtm,
    );
    live.setAlignment(NSTextAlignment::Right);
    fields.insert("live", live);
    fields.insert(
        "subtitle",
        label(
            &content,
            "Waiting for collector",
            top(MARGIN, 40.0, INNER, 16.0),
            11.0,
            0.0,
            Tone::Secondary,
            mtm,
        ),
    );
    separator(&content, 66.0, mtm);

    // CPU and memory gauges, one detail line each.
    for (key, title, x, heading_key, value_key, detail_key) in [
        (
            "cpu",
            "CPU",
            MARGIN,
            "cpu_heading",
            "cpu_value",
            "cpu_detail",
        ),
        (
            "ram",
            "MEMORY",
            RIGHT_COLUMN,
            "ram_heading",
            "ram_value",
            "ram_detail",
        ),
    ] {
        fields.insert(
            heading_key,
            heading(&content, title, top(x, 80.0, COLUMN, 14.0), mtm),
        );
        fields.insert(
            value_key,
            label(
                &content,
                "—",
                top(x, 96.0, COLUMN, 38.0),
                30.0,
                0.4,
                Tone::Primary,
                mtm,
            ),
        );
        bars.insert(key, gauge(&content, top(x, 140.0, COLUMN, BAR), mtm));
        fields.insert(
            detail_key,
            label(
                &content,
                "",
                top(x, 152.0, COLUMN, 15.0),
                11.0,
                0.0,
                Tone::Secondary,
                mtm,
            ),
        );
    }
    separator(&content, 180.0, mtm);

    // Storage: name left, detail right, full-width bar.
    heading(&content, "STORAGE", top(MARGIN, 194.0, INNER, 14.0), mtm);
    for (key, title, detail, y) in [
        ("internal", "Internal SSD", "internal_detail", 214.0),
        ("external", "Media SSD", "external_detail", 250.0),
    ] {
        label(
            &content,
            title,
            top(MARGIN, y, 120.0, 17.0),
            12.0,
            0.0,
            Tone::Primary,
            mtm,
        );
        let field = label(
            &content,
            "—",
            top(140.0, y + 1.0, 200.0, 16.0),
            11.0,
            0.0,
            Tone::Secondary,
            mtm,
        );
        field.setAlignment(NSTextAlignment::Right);
        fields.insert(detail, field);
        bars.insert(key, gauge(&content, top(MARGIN, y + 20.0, INNER, BAR), mtm));
    }
    separator(&content, 290.0, mtm);

    // Services: state dot, name, right-aligned summary.
    heading(&content, "SERVICES", top(MARGIN, 304.0, INNER, 14.0), mtm);
    for (index, (key, title)) in SERVICES.into_iter().enumerate() {
        let y = 324.0 + ROW * index as f64;
        dots.insert(key, dot(&content, top(MARGIN, y + 4.0, 8.0, 8.0), mtm));
        let name = label(
            &content,
            title,
            top(36.0, y, 90.0, 17.0),
            12.0,
            0.0,
            Tone::Primary,
            mtm,
        );
        let summary = label(
            &content,
            "Checking",
            top(126.0, y + 1.0, 214.0, 16.0),
            11.0,
            0.0,
            Tone::Secondary,
            mtm,
        );
        summary.setAlignment(NSTextAlignment::Right);
        summary.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        if key == "ci" {
            name.setToolTip(Some(&NSString::from_str(CI_NOTE)));
            summary.setToolTip(Some(&NSString::from_str(CI_NOTE)));
        }
        fields.insert(key, summary);
    }
    separator(&content, 452.0, mtm);

    // Footer actions.
    for (title, action, x, w) in [
        ("Copy JSON", sel!(copyJSON:), MARGIN, 92.0),
        ("Open Log", sel!(openLog:), MARGIN + 100.0, 84.0),
        ("Quit", sel!(quit:), WIDTH - MARGIN - 64.0, 64.0),
    ] {
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(title),
                Some(delegate),
                Some(action),
                mtm,
            )
        };
        button.setControlSize(NSControlSize::Small);
        button.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        button.setFrame(top(x, 464.0, w, 24.0));
        content.addSubview(&button);
    }
    Dashboard {
        item,
        panel,
        fields,
        bars,
        dots,
        preview: delegate.ivars().preview,
        last_status: RefCell::new(Vec::new()),
    }
}
