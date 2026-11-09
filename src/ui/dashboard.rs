use super::*;

pub(super) struct Dashboard {
    pub(super) item: Retained<NSStatusItem>,
    pub(super) panel: Retained<NSPanel>,
    pub(super) fields: HashMap<&'static str, Retained<NSTextField>>,
    pub(super) bars: HashMap<&'static str, Retained<NSProgressIndicator>>,
    pub(super) dots: HashMap<&'static str, Retained<NSImageView>>,
    pub(super) preview: bool,
    pub(super) last_status: RefCell<Vec<String>>,
}
impl Dashboard {
    fn paint_status(&self, values: Vec<String>) {
        if *self.last_status.borrow() == values {
            return;
        }
        let small = NSFont::systemFontOfSize(8.0);
        let large = NSFont::monospacedDigitSystemFontOfSize_weight(12.0, 0.15);
        let color = NSColor::blackColor();
        let small_attrs: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
            &[unsafe { NSFontAttributeName }, unsafe {
                NSForegroundColorAttributeName
            }],
            &[&*small, &*color],
        );
        let large_attrs: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
            &[unsafe { NSFontAttributeName }, unsafe {
                NSForegroundColorAttributeName
            }],
            &[&*large, &*color],
        );
        // Fit the text, keep 1pt at the outer edges, and separate metrics by 8pt.
        let titles = ["CPU", "RAM", "INT", "EXT", "UP"];
        let widths: Vec<f64> = titles
            .iter()
            .zip(values.iter())
            .map(|(title, value)| {
                // SAFETY: the dictionaries contain the documented font/color attribute types.
                unsafe {
                    NSString::from_str(value)
                        .sizeWithAttributes(Some(&large_attrs))
                        .width
                        .max(
                            NSString::from_str(title)
                                .sizeWithAttributes(Some(&small_attrs))
                                .width,
                        )
                        .ceil()
                }
            })
            .collect();
        let gap = 8.0;
        let width = 2.0 + widths.iter().sum::<f64>() + gap * 4.0;
        let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(width, 22.0));
        // Render only when values change. Child views in a status button trigger continual
        // AppKit replicant snapshots on macOS 27, which costs substantial idle CPU.
        #[allow(deprecated)]
        image.lockFocus();
        let mut x = 1.0;
        for ((title, value), w) in titles.iter().zip(values.iter()).zip(widths.iter().copied()) {
            // SAFETY: attribute keys map to their documented NSFont and NSColor types.
            unsafe {
                NSString::from_str(title)
                    .drawInRect_withAttributes(rect(x, 13.0, w, 9.0), Some(&small_attrs));
                NSString::from_str(value)
                    .drawInRect_withAttributes(rect(x, -1.0, w, 16.0), Some(&large_attrs));
            }
            x += w + gap;
        }
        #[allow(deprecated)]
        image.unlockFocus();
        image.setTemplate(true);
        self.item.setLength(width);
        if let Some(button) = self.item.button(self.panel.mtm()) {
            button.setImage(Some(&image));
            button.setTitle(&NSString::from_str(&values.join(" ")));
            button.setImagePosition(NSCellImagePosition::ImageOnly);
        }
        *self.last_status.borrow_mut() = values;
    }
    fn text(&self, key: &str, value: &str) {
        if let Some(field) = self.fields.get(key) {
            let value = NSString::from_str(value);
            if field.stringValue() != value {
                field.setStringValue(&value);
            }
        }
    }
    fn color(&self, key: &str, color: &NSColor) {
        if let Some(field) = self.fields.get(key) {
            field.setTextColor(Some(color));
        }
    }
    fn dot(&self, key: &str, state: State) {
        let color = match state {
            State::Healthy => NSColor::systemGreenColor(),
            State::Degraded => NSColor::systemOrangeColor(),
            State::Down => NSColor::systemRedColor(),
            State::Unknown => NSColor::tertiaryLabelColor(),
        };
        if let Some(dot) = self.dots.get(key) {
            dot.setContentTintColor(Some(&color));
        }
    }
    fn render_offline(&self) {
        self.paint_status(vec!["—".into(); 5]);
        self.text("host", "Ministat");
        self.text("live", "OFFLINE");
        self.color("live", &NSColor::systemRedColor());
        self.text("subtitle", "Waiting for the boot collector");
        self.text("cpu_heading", "CPU");
        self.text("ram_heading", "MEMORY");
        self.color("ram_value", &NSColor::labelColor());
        for key in [
            "cpu_value",
            "ram_value",
            "cpu_detail",
            "ram_detail",
            "internal_detail",
            "external_detail",
        ] {
            self.text(key, "—");
        }
        for (key, _) in layout::SERVICES {
            self.text(key, "Waiting for collector");
            self.dot(key, State::Unknown);
        }
        for bar in self.bars.values() {
            bar.setDoubleValue(0.0);
        }
    }
    pub(super) fn render(&self, view: Result<StatusView, std::io::Error>) {
        let Ok(view) = view else {
            self.render_offline();
            return;
        };
        let current = view.freshness == Freshness::Current;
        let s = &view.snapshot;
        let disk_current = current && view.disks_freshness == Freshness::Current;
        let percent = |value: Option<f64>| {
            value
                .filter(|_| current)
                .map(|n| format!("{n:.0}%"))
                .unwrap_or_else(|| "—".into())
        };
        let cpu = percent(s.cpu_percent.map(f64::from));
        let ram = percent(Some(s.memory.used_percent));
        let disk_cell = |disk: &crate::model::Disk| {
            if !disk_current {
                "—".into()
            } else {
                disk.used_percent
                    .map(|n| format!("{n:.0}%"))
                    .unwrap_or_else(|| "OFF".into())
            }
        };
        self.paint_status(vec![
            cpu.clone(),
            ram.clone(),
            disk_cell(&s.disks[0]),
            disk_cell(&s.disks[1]),
            if current {
                compact_duration_label(s.uptime_seconds)
            } else {
                "STALE".into()
            },
        ]);
        if !self.preview && !self.panel.isVisible() {
            return;
        }
        self.text("host", &s.hostname);
        self.text(
            "subtitle",
            &format!(
                "up {} · awake {}",
                duration_label(s.uptime_seconds),
                duration_label(s.awake_seconds)
            ),
        );
        if current {
            self.text("live", "LIVE");
            self.color("live", &NSColor::systemGreenColor());
        } else {
            self.text("live", "STALE");
            self.color("live", &NSColor::systemOrangeColor());
        }
        self.text("cpu_value", &cpu);
        self.text("ram_value", &ram);
        self.text(
            "cpu_heading",
            &match &s.load {
                Some(load) => format!("CPU · {} CORES", load.logical_cpus),
                None => "CPU".into(),
            },
        );
        self.text(
            "cpu_detail",
            &match (&s.load, current) {
                (Some(load), true) => format!(
                    "Load {:.2} · {:.2} · {:.2}",
                    load.one_minute, load.five_minutes, load.fifteen_minutes
                ),
                _ => "Load —".into(),
            },
        );
        let (ram_heading, ram_color) = match s.memory.pressure.as_str() {
            "warning" if current => ("MEMORY · WARNING", NSColor::systemOrangeColor()),
            "critical" if current => ("MEMORY · CRITICAL", NSColor::systemRedColor()),
            _ => ("MEMORY", NSColor::labelColor()),
        };
        self.text("ram_heading", ram_heading);
        self.color("ram_value", &ram_color);
        let swap = s.memory.swap_used_bytes as f64 / 1073741824.0;
        self.text(
            "ram_detail",
            &if current {
                format!(
                    "{:.1} of {:.0} GiB · swap {}",
                    s.memory.used_bytes as f64 / 1073741824.0,
                    s.memory.total_bytes as f64 / 1073741824.0,
                    if swap >= 1.0 {
                        format!("{swap:.1} GiB")
                    } else {
                        format!("{:.0} MiB", swap * 1024.0)
                    }
                )
            } else {
                "—".into()
            },
        );
        for (index, (disk, detail, bar_key)) in [
            (&s.disks[0], "internal_detail", "internal"),
            (&s.disks[1], "external_detail", "external"),
        ]
        .into_iter()
        .enumerate()
        {
            self.text(
                detail,
                &if !disk_current {
                    "Waiting for current data".into()
                } else if disk.mounted {
                    format!(
                        "{:.0}% used · {:.1} GiB free",
                        disk.used_percent.unwrap_or(0.0),
                        disk.available_bytes.unwrap_or(0) as f64 / 1073741824.0
                    )
                } else {
                    "Disconnected".into()
                },
            );
            if let Some(bar) = self.bars.get(bar_key) {
                bar.setDoubleValue(view.disk_gauge_percent(index));
            }
        }
        for (key, service) in [
            ("homekit", &s.health.homekit),
            ("jellyfin", &s.health.jellyfin),
            ("ci", &s.health.ci),
            ("hermes", &s.health.hermes),
            ("tailscale", &s.health.tailscale),
        ] {
            self.text(key, &service.summary);
            self.dot(key, service.state);
        }
        for (key, value) in [
            ("cpu", s.cpu_percent.unwrap_or(0.0) as f64),
            ("ram", s.memory.used_percent),
        ] {
            self.bars[key].setDoubleValue(if current { value } else { 0.0 });
        }
        if let Some(button) = self.item.button(self.panel.mtm()) {
            button.setToolTip(Some(&NSString::from_str(&format!(
                "Ministat · {} · {:?} · click for details",
                s.hostname, view.freshness
            ))));
        }
    }
}
