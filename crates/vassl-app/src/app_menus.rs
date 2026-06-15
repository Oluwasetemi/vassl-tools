use gpui::{Menu, MenuItem, OsAction, SystemMenuType};

use crate::actions::{
    About, DecreaseFontSize, FocusSearch, Hide, HideOthers, IncreaseFontSize, Minimize,
    OpenAuditLog, OpenChangelog, OpenDocumentation, OpenGlobalSearch, OpenInventory, OpenPriceBook,
    OpenQuotations, OpenSettings, Quit, ShowAll, Zoom,
};
use vassl_ui::NewRecord;

pub fn app_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "VASSL".into(),
            disabled: false,
            items: vec![
                MenuItem::action("About VASSL", About),
                MenuItem::separator(),
                MenuItem::action("Settings", OpenSettings),
                MenuItem::separator(),
                #[cfg(target_os = "macos")]
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                #[cfg(target_os = "macos")]
                MenuItem::separator(),
                #[cfg(target_os = "macos")]
                MenuItem::action("Hide VASSL", Hide),
                #[cfg(target_os = "macos")]
                MenuItem::action("Hide Others", HideOthers),
                #[cfg(target_os = "macos")]
                MenuItem::action("Show All", ShowAll),
                MenuItem::separator(),
                MenuItem::action("Quit VASSL", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            disabled: false,
            items: vec![
                MenuItem::action("New Record", NewRecord),
                MenuItem::separator(),
                MenuItem::action("Inventory", OpenInventory),
                MenuItem::action("Quotations", OpenQuotations),
                MenuItem::action("Price Book", OpenPriceBook),
                MenuItem::separator(),
                MenuItem::action("Audit Log", OpenAuditLog),
            ],
        },
        Menu {
            name: "Edit".into(),
            disabled: false,
            items: vec![
                MenuItem::os_action("Cut", vassl_ui::text_input::Cut, OsAction::Cut),
                MenuItem::os_action("Copy", vassl_ui::text_input::Copy, OsAction::Copy),
                MenuItem::os_action("Paste", vassl_ui::text_input::Paste, OsAction::Paste),
                MenuItem::os_action(
                    "Select All",
                    vassl_ui::text_input::SelectAll,
                    OsAction::SelectAll,
                ),
            ],
        },
        Menu {
            name: "View".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Zoom In", IncreaseFontSize),
                MenuItem::action("Zoom Out", DecreaseFontSize),
                MenuItem::separator(),
                MenuItem::action("Search", FocusSearch),
                MenuItem::action("Global Search", OpenGlobalSearch),
            ],
        },
        Menu {
            name: "Window".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", Zoom),
            ],
        },
        Menu {
            name: "Help".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Documentation", OpenDocumentation),
                MenuItem::action("Changelog", OpenChangelog),
                MenuItem::separator(),
                MenuItem::action("About VASSL", About),
            ],
        },
    ]
}
