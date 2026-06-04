use gpui::actions;

actions!(vassl, [
    OpenInventory,
    OpenQuotations,
    OpenPriceBook,
    OpenAuditLog,
    OpenSettings,
    NewRecord,
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
]);
