//! Both palettes have to be readable, not just the one the session happens to use.
//!
//! These are the relationships the colours were chosen for, written down so that
//! changing a value in one palette cannot quietly make something unreadable in the
//! other. The thresholds are per role rather than one global number: an intentionally
//! quiet answer is dimmer than the date beside it, and a border is a hint rather than
//! text.

use glimt_shell::theme::{Palette, Rgb};

/// What the light palette was drawn with before there was a dark one, and what a
/// session without a preference gets.
const PALETTES: [(&str, Palette); 2] = [("light", Palette::LIGHT), ("dark", Palette::DARK)];

fn channel(value: u8) -> f64 {
    let value = f64::from(value) / 255.0;

    if value <= 0.03928 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(colour: Rgb) -> f64 {
    0.2126 * channel(colour.0) + 0.7152 * channel(colour.1) + 0.0722 * channel(colour.2)
}

/// WCAG's contrast ratio: `(L1 + 0.05) / (L2 + 0.05)`, with the lighter first.
fn contrast(one: Rgb, other: Rgb) -> f64 {
    let (lighter, darker) = {
        let (one, other) = (luminance(one), luminance(other));

        if one > other {
            (one, other)
        } else {
            (other, one)
        }
    };

    (lighter + 0.05) / (darker + 0.05)
}

#[test]
fn ink_is_readable_on_every_surface_it_is_drawn_over() {
    for (name, palette) in PALETTES {
        for surface in [
            palette.background,
            palette.bar_fill,
            palette.shade,
            palette.past_fill,
        ] {
            let ratio = contrast(palette.ink, surface);
            assert!(
                ratio >= 7.0,
                "{name}: ink on {surface:?} is {ratio:.2}:1, below the 7:1 body-text floor"
            );
        }

        // Today's number sits on the accent, not on the background.
        let ratio = contrast(palette.today_ink, palette.accent);
        assert!(
            ratio >= 4.5,
            "{name}: today's ink on the accent is {ratio:.2}:1, below 4.5:1"
        );
    }
}

#[test]
fn the_quieter_roles_stay_visible() {
    for (name, palette) in PALETTES {
        // Holidays are secondary information, and were dimmer than body text before
        // there was a dark palette; 2.3 is what the light one already does at its
        // worst, over a shaded weekend.
        for surface in [palette.background, palette.bar_fill, palette.shade] {
            let ratio = contrast(palette.muted, surface);
            assert!(
                ratio >= 2.3,
                "{name}: muted text on {surface:?} is {ratio:.2}:1"
            );
        }

        let ratio = contrast(palette.past_ink, palette.past_fill);
        assert!(
            ratio >= 2.3,
            "{name}: a past day is {ratio:.2}:1 against its fill"
        );

        // A faded answer is meant to recede, not to disappear.
        let ratio = contrast(palette.faded, palette.bar_fill);
        assert!(
            ratio >= 1.8,
            "{name}: a faded answer is {ratio:.2}:1 against the calculator"
        );
    }
}

#[test]
fn today_and_the_selection_can_be_told_apart() {
    for (name, palette) in PALETTES {
        // Both are drawn over the calendar, including over a shaded weekend.
        for surface in [palette.background, palette.shade] {
            let accent = contrast(palette.accent, surface);
            assert!(
                accent >= 3.0,
                "{name}: the accent on {surface:?} is {accent:.2}:1"
            );

            let selection = contrast(palette.selection, surface);
            assert!(
                selection >= 3.0,
                "{name}: the selection on {surface:?} is {selection:.2}:1"
            );
        }

        // Different hues, not only different brightness.
        assert_ne!(palette.accent, palette.selection);
    }
}

#[test]
fn every_surface_is_visible_against_the_page() {
    for (name, palette) in PALETTES {
        // Past days, weekends and the calculator are tints of the background rather
        // than separate colours, so the threshold is low by design.
        for (role, surface) in [
            ("bar", palette.bar_fill),
            ("shade", palette.shade),
            ("past", palette.past_fill),
            ("future border", palette.future_border),
        ] {
            let ratio = contrast(surface, palette.background);
            assert!(
                ratio >= 1.05,
                "{name}: the {role} is {ratio:.2}:1 against the page"
            );
        }

        let rule = contrast(palette.rule, palette.background);
        assert!(rule >= 1.2, "{name}: the rule is {rule:.2}:1");
    }
}
