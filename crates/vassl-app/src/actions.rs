use gpui::actions;

actions!(vassl, [
    Logout,
    OpenInventory,
    OpenQuotations,
    OpenPriceBook,
    OpenSuppliers,
    OpenProjects,
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
