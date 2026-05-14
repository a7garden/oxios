//! SVG icon components for Oxios UI.
//!
//! All icons use Lucide-style stroke-based SVG (24×24 viewBox).
//! Icons inherit `currentColor` for seamless theming.

use dioxus::prelude::*;

/// Shared SVG wrapper attributes.
macro_rules! icon {
    ($name:ident, $($child:tt)*) => {
        #[component]
        pub fn $name(class: Option<String>, size: Option<u32>) -> Element {
            let cls = class.unwrap_or_else(|| "icon".to_string());
            let s = size.unwrap_or(20);
            rsx! {
                svg {
                    class: "{cls}",
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "{s}",
                    height: "{s}",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    $($child)*
                }
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Navigation icons
// ---------------------------------------------------------------------------

icon!(IconChat,
    path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
);

icon!(IconDashboard,
    rect { x: "3", y: "3", width: "7", height: "7", rx: "1" }
    rect { x: "14", y: "3", width: "7", height: "7", rx: "1" }
    rect { x: "14", y: "14", width: "7", height: "7", rx: "1" }
    rect { x: "3", y: "14", width: "7", height: "7", rx: "1" }
);

icon!(IconProtocol,
    path { d: "M23 4v6h-6" }
    path { d: "M1 20v-6h6" }
    path { d: "M3.51 9a9 9 0 0 1 14.85-3.36L23 10" }
    path { d: "M20.49 15a9 9 0 0 1-14.85 3.36L1 14" }
);

icon!(IconAgents,
    rect { x: "4", y: "4", width: "16", height: "16", rx: "2" }
    rect { x: "9", y: "9", width: "6", height: "6" }
    path { d: "M9 1v3M15 1v3M9 20v3M15 20v3M20 9h3M20 14h3M1 9h3M1 14h3" }
);

icon!(IconSeeds,
    path { d: "M7 20h10" }
    path { d: "M10 20c5.5-2.5.8-6.4 3-10" }
    path { d: "M9.5 9.4c1.1.8 1.8 2.2 2.3 3.7-2 .4-3.5.4-4.8-.3-1.2-.6-2.3-1.9-3-4.2 2.8-.5 4.4 0 5.5.8z" }
    path { d: "M14.1 6a7 7 0 0 0-1.1 4c0 1 .3 2.2 1 3.5" }
);

icon!(IconFolder,
    path { d: "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" }
);

icon!(IconSkills,
    path { d: "M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" }
);

icon!(IconPackage,
    path { d: "M21 16V8l-9-5-9 5v8l9 5 9-5z" }
    path { d: "M12 22V12" }
    path { d: "M3 8l9 4 9-4" }
);

icon!(IconMemory,
    path { d: "M12 2L2 7l10 5 10-5-10-5z" }
    path { d: "M2 17l10 5 10-5" }
    path { d: "M2 12l10 5 10-5" }
);

icon!(IconClock,
    circle { cx: "12", cy: "12", r: "10" }
    path { d: "M12 6v6l4 2" }
);

icon!(IconShield,
    path { d: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" }
);

icon!(IconCheckSquare,
    path { d: "M9 11l3 3L22 4" }
    path { d: "M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" }
);

icon!(IconSettings,
    circle { cx: "12", cy: "12", r: "3" }
    path { d: "M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" }
);

icon!(IconActivity,
    path { d: "M22 12h-4l-3 9L9 3l-3 9H2" }
);

icon!(IconUsers,
    path { d: "M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" }
    circle { cx: "9", cy: "7", r: "4" }
    path { d: "M23 21v-2a4 4 0 0 0-3-3.87" }
    path { d: "M16 3.13a4 4 0 0 1 0 7.75" }
);

icon!(IconWrench,
    path { d: "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" }
);

// ---------------------------------------------------------------------------
// Action / UI icons
// ---------------------------------------------------------------------------

icon!(IconSend,
    path { d: "M22 2L11 13" }
    path { d: "M22 2l-7 20-4-9-9-4 20-7z" }
);

icon!(IconRefresh,
    path { d: "M23 4v6h-6" }
    path { d: "M1 20v-6h6" }
    path { d: "M3.51 9a9 9 0 0 1 14.85-3.36L23 10" }
    path { d: "M20.49 15a9 9 0 0 1-14.85 3.36L1 14" }
);

icon!(IconMenu,
    path { d: "M3 6h18M3 12h18M3 18h18" }
);

icon!(IconX,
    path { d: "M18 6L6 18M6 6l12 12" }
);

icon!(IconChevronLeft,
    path { d: "M15 18l-6-6 6-6" }
);

icon!(IconChevronRight,
    path { d: "M9 18l6-6-6-6" }
);

icon!(IconSun,
    circle { cx: "12", cy: "12", r: "5" }
    path { d: "M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" }
);

icon!(IconMoon,
    path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
);

icon!(IconArrowUp,
    path { d: "M12 19V5M5 12l7-7 7 7" }
);

icon!(IconFile,
    path { d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" }
    path { d: "M14 2v6h6" }
);

icon!(IconCheck,
    path { d: "M20 6L9 17l-5-5" }
);

icon!(IconAlertTriangle,
    path { d: "M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" }
    path { d: "M12 9v4" }
    path { d: "M12 17h.01" }
);

icon!(IconPlus,
    path { d: "M12 5v14M5 12h14" }
);

icon!(IconTrash,
    path { d: "M3 6h18" }
    path { d: "M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" }
);

icon!(IconLoading,
    path { d: "M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83" }
);

icon!(IconEye,
    path { d: "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" }
    circle { cx: "12", cy: "12", r: "3" }
);

icon!(IconChevronDown,
    path { d: "M6 9l6 6 6-6" }
);

icon!(IconLogOut,
    path { d: "M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" }
    path { d: "M16 17l5-5-5-5" }
    path { d: "M21 12H9" }
);

icon!(IconZap,
    path { d: "M13 2L3 14h9l-1 8 10-12h-9l1-8z" }
);

icon!(IconSearch,
    circle { cx: "11", cy: "11", r: "8" }
    path { d: "M21 21l-4.35-4.35" }
);

icon!(IconCopy,
    rect { x: "9", y: "9", width: "13", height: "13", rx: "2", ry: "2" }
    path { d: "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" }
);

icon!(IconExternalLink,
    path { d: "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" }
    path { d: "M15 3h6v6" }
    path { d: "M10 14L21 3" }
);

icon!(IconCircleCheck,
    path { d: "M22 11.08V12a10 10 0 1 1-5.93-9.14" }
    path { d: "M22 4L12 14.01l-3-3" }
);

icon!(IconCircleX,
    circle { cx: "12", cy: "12", r: "10" }
    path { d: "M15 9l-6 6M9 9l6 6" }
);

icon!(IconInfo,
    circle { cx: "12", cy: "12", r: "10" }
    path { d: "M12 16v-4" }
    path { d: "M12 8h.01" }
);

icon!(IconDatabase,
    ellipse { cx: "12", cy: "5", rx: "9", ry: "3" }
    path { d: "M21 12c0 1.66-4 3-9 3s-9-1.34-9-3" }
    path { d: "M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5" }
);

icon!(IconPlay,
    circle { cx: "12", cy: "12", r: "10" }
    path { d: "M10 8l6 4-6 4V8z" }
);
