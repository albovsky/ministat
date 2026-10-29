use super::*;

pub(super) struct Dashboard {
    pub(super) item: Retained<NSStatusItem>,
    pub(super) panel: Retained<NSPanel>,
    pub(super) fields: HashMap<&'static str, Retained<NSTextField>>,
    pub(super) bars: HashMap<&'static str, Retained<NSProgressIndicator>>,
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
    pub(super) fn render(&self, view: Result<StatusView, std::io::Error>) {
        let Ok(view) = view else {
            self.paint_status(vec!["—".into(); 5]);
            for key in [
                "cpu_value",
                "ram_value",
                "memory_detail",
                "internal_detail",
                "external_detail",
                "pressure",
                "trend",
            ] {
                self.text(key, "—");
            }
            self.text("live", "OFFLINE");
            self.text("subtitle", "Waiting for the boot collector");
            for key in ["homekit", "jellyfin", "ci"] {
                self.text(key, "○  Waiting for collector");
            }
            for bar in self.bars.values() {
                bar.setDoubleValue(0.0);
            }
            return;
        };
        let current = view.freshness == Freshness::Current;
        let s = &view.snapshot;
        let disk_current = current && view.disks_freshness == Freshness::Current;
        self.paint_status(vec![
            if current {
                s.cpu_percent
                    .map(|n| format!("{n:.0}%"))
                    .unwrap_or_else(|| "—".into())
            } else {
                "—".into()
            },
            if current {
                format!("{:.0}%", s.memory.used_percent)
            } else {
                "—".into()
            },
            if disk_current {
                s.disks[0]
                    .used_percent
                    .map(|n| format!("{n:.0}%"))
                    .unwrap_or_else(|| "OFF".into())
            } else {
                "—".into()
            },
            if disk_current {
                s.disks[1]
                    .used_percent
                    .map(|n| format!("{n:.0}%"))
                    .unwrap_or_else(|| "OFF".into())
            } else {
                "—".into()
            },
            if current {
                compact_duration_label(s.uptime_seconds)
            } else {
                "STALE".into()
            },
        ]);
        if !self.panel.isVisible() {
            return;
        }
        self.text("up", &duration_label(s.uptime_seconds));
        let cpu = s
            .cpu_percent
            .map(|n| format!("{n:.0}%"))
            .unwrap_or_else(|| "—".into());
        self.text("cpu", if current { &cpu } else { "—" });
        self.text(
            "ram",
            &if current {
                format!("{:.0}%", s.memory.used_percent)
            } else {
                "—".into()
            },
        );
        self.text("cpu_value", if current { &cpu } else { "—" });
        self.text(
            "ram_value",
            &if current {
                format!("{:.0}%", s.memory.used_percent)
            } else {
                "—".into()
            },
        );
        self.text(
            "subtitle",
            &format!(
                "{} · up {} · awake {}",
                s.hostname,
                duration_label(s.uptime_seconds),
                duration_label(s.awake_seconds)
            ),
        );
        self.text("live", if current { "LIVE · 2s" } else { "STALE" });
        self.text(
            "memory_detail",
            &format!(
                "{:.1} / {:.0} GiB used",
                s.memory.used_bytes as f64 / 1073741824.0,
                s.memory.total_bytes as f64 / 1073741824.0
            ),
        );
        for (disk, key, detail, bar_key) in [
            (&s.disks[0], "int", "internal_detail", "internal"),
            (&s.disks[1], "ext", "external_detail", "external"),
        ] {
            self.text(
                key,
                &if !current {
                    "—".into()
                } else {
                    disk.used_percent
                        .map(|n| format!("{n:.0}%"))
                        .unwrap_or_else(|| "OFF".into())
                },
            );
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
                bar.setDoubleValue(if current {
                    disk.used_percent.unwrap_or(0.0)
                } else {
                    0.0
                });
            }
        }
        for (key, service) in [
            ("homekit", &s.health.homekit),
            ("jellyfin", &s.health.jellyfin),
            ("ci", &s.health.ci),
        ] {
            let icon = match service.state {
                State::Healthy => "●",
                State::Degraded => "◐",
                State::Down => "●",
                State::Unknown => "○",
            };
            self.text(key, &format!("{icon}  {}", service.summary));
            let color = match service.state {
                State::Healthy => NSColor::secondaryLabelColor(),
                State::Degraded => NSColor::systemOrangeColor(),
                State::Down => NSColor::systemRedColor(),
                State::Unknown => NSColor::tertiaryLabelColor(),
            };
            self.fields[key].setTextColor(Some(&color));
        }
        self.text(
            "pressure",
            &format!(
                "Pressure {} · swap {:.0} MiB",
                s.memory.pressure,
                s.memory.swap_used_bytes as f64 / 1048576.0
            ),
        );
        let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let trend: String = s
            .cpu_history
            .iter()
            .step_by(2)
            .map(|v| chars[((*v as usize * 7) / 100).min(7)])
            .collect();
        self.text("trend", &trend);
        for (key, value) in [
            ("cpu", s.cpu_percent.unwrap_or(0.0) as f64),
            ("memory", s.memory.used_percent),
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
