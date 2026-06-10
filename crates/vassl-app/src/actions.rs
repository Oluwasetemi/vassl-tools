use gpui::actions;

actions!(vassl, [
    OpenInventory,
    OpenQuotations,
    OpenPriceBook,
    OpenSuppliers,
    OpenAuditLog,
    OpenSettings,
    FocusSearch,
    EscapeModal,
    SelectNext,
    SelectPrev,
    ConfirmSelection,
    OpenGlobalSearch,
    IncreaseFontSize,
    DecreaseFontSize,
    // App-level menu actions
    Quit,
    About,
    Hide,
    HideOthers,
    ShowAll,
    Minimize,
    Zoom,
    // Auto-update
    CheckForUpdates,
    InstallUpdate,
    // Help
    OpenDocumentation,
    OpenChangelog,
]);
