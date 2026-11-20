use resvg::{tiny_skia, usvg};

const ICONS: &[(&str, &str)] = &[
    ("bulb", r#"<path d="M9 18h6M10 21h4"/><path d="M12 3a6 6 0 0 0-3.6 10.8c.7.6 1.1 1.3 1.1 2.2h5c0-.9.4-1.6 1.1-2.2A6 6 0 0 0 12 3z"/>"#),
    ("keyboard", r#"<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M6 14h.01M18 14h.01M9 14h6"/>"#),
    ("screens", r#"<rect x="3" y="3" width="8" height="8" rx="1.5"/><rect x="13" y="3" width="8" height="8" rx="1.5"/><rect x="3" y="13" width="8" height="8" rx="1.5"/><rect x="13" y="13" width="8" height="8" rx="1.5"/>"#),
    ("dial", r#"<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="3"/><path d="M12 3.5v3.5"/>"#),
    ("settings", r#"<circle cx="12" cy="12" r="3"/><path d="M10.3 2.5h3.4l.5 2.6 1.8.8 2.2-1.5 2.4 2.4-1.5 2.2.8 1.8 2.6.5v3.4l-2.6.5-.8 1.8 1.5 2.2-2.4 2.4-2.2-1.5-1.8.8-.5 2.6h-3.4l-.5-2.6-1.8-.8-2.2 1.5-2.4-2.4 1.5-2.2-.8-1.8-2.6-.5v-3.4l2.6-.5.8-1.8-1.5-2.2 2.4-2.4 2.2 1.5 1.8-.8z"/>"#),
    ("check", r#"<path d="M5 12.5l4.5 4.5L19 7"/>"#),
    ("warn", r#"<path d="M12 3.5l9.5 16.5h-19z"/><path d="M12 10v4.5M12 17.5h.01"/>"#),
    ("x", r#"<path d="M6 6l12 12M18 6L6 18"/>"#),
    ("plus", r#"<path d="M12 5v14M5 12h14"/>"#),
    ("minus", r#"<path d="M5 12h14"/>"#),
    ("upload", r#"<path d="M12 16V4M7 9l5-5 5 5M4 20h16"/>"#),
    ("download", r#"<path d="M12 4v12M7 11l5 5 5-5M4 20h16"/>"#),
    ("send", r#"<path d="M21 3L10 14M21 3l-7 18-4-7-7-4z"/>"#),
    ("image", r#"<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="M21 17l-5-5-9 8"/>"#),
    ("text", r#"<path d="M5 7V4h14v3M12 4v16M9 20h6"/>"#),
    ("brush", r#"<path d="M18 3l3 3-9.5 9.5-3-3z"/><path d="M8.5 12.5C6 12.5 5 14.5 5 16s-1 2.5-2 3c2 1 6 1 7.5-1.5.8-1.3.5-2.8-.5-3.5"/>"#),
    ("eraser", r#"<path d="M7 21h13M5.5 15.5l9-9a2 2 0 0 1 2.8 0l2.2 2.2a2 2 0 0 1 0 2.8L12 19H8z"/>"#),
    ("fill", r#"<path d="M5 11l7-7 8 8-7 7a2 2 0 0 1-2.8 0L5 13.8a2 2 0 0 1 0-2.8z"/><path d="M5 12h15M20 17c0 1.2-.9 2-2 2s-2-.8-2-2c0-1.3 2-3.5 2-3.5s2 2.2 2 3.5z"/>"#),
    ("save", r#"<path d="M5 3h11l3 3v13a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path d="M7 3v5h8V3M7 21v-7h10v7"/>"#),
    ("bolt", r#"<path d="M13 2L4 14h7l-1 8 9-12h-7z"/>"#),
    ("layers", r#"<path d="M12 3l9 5-9 5-9-5z"/><path d="M3 13l9 5 9-5"/>"#),
    ("link", r#"<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>"#),
    ("shield", r#"<path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z"/><path d="M9 12l2 2 4-4"/>"#),
    ("desktop", r#"<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>"#),
    ("usb", r#"<path d="M12 2v14M9 5l3-3 3 3"/><circle cx="12" cy="19" r="2.5"/><path d="M12 12L7 9V7M12 14l5-3V9"/><circle cx="17" cy="8" r="1.2"/>"#),
    ("palette", r##"<path d="M12 3a9 9 0 0 0 0 18c1.1 0 1.7-.8 1.7-1.6 0-.9-.7-1.4-.7-2.2 0-.9.7-1.7 1.7-1.7H17a4 4 0 0 0 4-4c0-4.7-4-8.5-9-8.5z"/><circle cx="7.5" cy="11" r="1.2" fill="#FFFFFF"/><circle cx="10" cy="7" r="1.2" fill="#FFFFFF"/><circle cx="15" cy="7.5" r="1.2" fill="#FFFFFF"/>"##),
    ("fx-static", r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>"#),
    ("fx-wave", r#"<path d="M2 9c2.5-3 5-3 7.5 0s5 3 7.5 0 3.5-2 5-1.5M2 15c2.5-3 5-3 7.5 0s5 3 7.5 0 3.5-2 5-1.5"/>"#),
    ("fx-tornado", r#"<path d="M3 4h18M5 8h14M7 12h10M9 16h6M11 20h2"/>"#),
    ("fx-breath", r#"<circle cx="12" cy="12" r="3"/><circle cx="12" cy="12" r="6.5" stroke-opacity=".6"/><circle cx="12" cy="12" r="10" stroke-opacity=".3"/>"#),
    ("fx-reactive", r##"<circle cx="12" cy="12" r="2" fill="#FFFFFF"/><path d="M12 4.5v2M12 17.5v2M4.5 12h2M17.5 12h2M6.7 6.7l1.4 1.4M15.9 15.9l1.4 1.4M6.7 17.3l1.4-1.4M15.9 8.1l1.4-1.4"/>"##),
    ("fx-matrix", r#"<path d="M5 3v4M5 10v2M5 15v6M10 3v2M10 8v7M10 18v3M15 3v6M15 12v2M15 17v4M20 3v3M20 9v5M20 17v4"/>"#),
    ("fx-yeti", r#"<path d="M12 2v20M3.3 7l17.4 10M3.3 17L20.7 7M9 4l3 2 3-2M9 20l3-2 3 2"/>"#),
    ("fx-gradient", r#"<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M7.5 5v14"/><path d="M12 5v14" stroke-opacity=".6"/><path d="M16.5 5v14" stroke-opacity=".3"/>"#),
    ("fx-rainbow", r#"<path d="M3 18a9 9 0 0 1 18 0"/><path d="M6.5 18a5.5 5.5 0 0 1 11 0"/><path d="M10 18a2 2 0 0 1 4 0"/>"#),
    ("fx-aurora", r#"<path d="M3 14c3-6 5-6 8-1s6 4 10-5"/><path d="M3 19c3-4 6-4 9-1s5 2 9-3" stroke-opacity=".55"/>"#),
    ("fx-rain", r#"<path d="M7 4v5M12 8v6M17 3v5M7 14v2M17 13v6M12 18v2"/>"#),
    ("fx-spectrum", r#"<path d="M4 20v-5M8 20V9M12 20V5M16 20v-8M20 20v-4"/>"#),
    ("gpu", r#"<rect x="2" y="6" width="17" height="11" rx="1.5"/><circle cx="10.5" cy="11.5" r="2.8"/><path d="M19 9h3M19 14h3M5 17v3M9 17v3"/>"#),
    ("ram", r#"<rect x="3" y="6" width="18" height="10" rx="1"/><path d="M7 9v4M11 9v4M15 9v4M6 16v3M10 16v3M14 16v3M18 16v3"/>"#),
    ("profile", r#"<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>"#),
    ("play", r##"<path d="M8 5v14l11-7z" fill="#FFFFFF"/>"##),
    ("pause", r##"<rect x="6.5" y="5" width="3.5" height="14" rx="1" fill="#FFFFFF"/><rect x="14" y="5" width="3.5" height="14" rx="1" fill="#FFFFFF"/>"##),
    ("playpause", r##"<path d="M3 5v14l9-7z" fill="#FFFFFF"/><path d="M15.5 5v14M20 5v14"/>"##),
    ("next", r##"<path d="M5 5v14l10-7z" fill="#FFFFFF"/><path d="M19 5v14"/>"##),
    ("prev", r##"<path d="M19 5v14L9 12z" fill="#FFFFFF"/><path d="M5 5v14"/>"##),
    ("stop", r##"<rect x="6" y="6" width="12" height="12" rx="1.5" fill="#FFFFFF"/>"##),
    ("record", r##"<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="4" fill="#FFFFFF"/>"##),
    ("vol-up", r#"<path d="M4 9h4l5-4v14l-5-4H4z"/><path d="M16.5 8.5a5 5 0 0 1 0 7M19.5 5.5a9 9 0 0 1 0 13"/>"#),
    ("vol-down", r#"<path d="M4 9h4l5-4v14l-5-4H4z"/><path d="M16.5 8.5a5 5 0 0 1 0 7"/>"#),
    ("mute", r#"<path d="M4 9h4l5-4v14l-5-4H4z"/><path d="M16 9l5 6M21 9l-5 6"/>"#),
    ("mic", r#"<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6"/>"#),
    ("mic-off", r#"<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6M3 3l18 18"/>"#),
    ("headphones", r#"<path d="M4 15v-3a8 8 0 0 1 16 0v3"/><rect x="3" y="14" width="4" height="7" rx="1.5"/><rect x="17" y="14" width="4" height="7" rx="1.5"/>"#),
    ("music", r#"<path d="M9 18V5l11-2v13"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="17.5" cy="16" r="2.5"/>"#),
    ("video", r#"<rect x="3" y="6" width="13" height="12" rx="2"/><path d="M16 10l5-3v10l-5-3"/>"#),
    ("video-off", r#"<rect x="3" y="6" width="13" height="12" rx="2"/><path d="M16 10l5-3v10l-5-3M2 3l19 18"/>"#),
    ("stream", r##"<circle cx="12" cy="12" r="2" fill="#FFFFFF"/><path d="M8.5 15.5a5 5 0 0 1 0-7M15.5 8.5a5 5 0 0 1 0 7M5.6 18.4a9 9 0 0 1 0-12.8M18.4 5.6a9 9 0 0 1 0 12.8"/>"##),
    ("camera", r#"<path d="M4 8h3l2-3h6l2 3h3a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V9a1 1 0 0 1 1-1z"/><circle cx="12" cy="13" r="3.5"/>"#),
    ("screenshot", r#"<path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"/><circle cx="12" cy="12" r="3"/>"#),
    ("lock", r#"<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4M12 15v2"/>"#),
    ("power", r#"<path d="M12 3v9"/><path d="M6.3 6.3a8 8 0 1 0 11.4 0"/>"#),
    ("reboot", r#"<path d="M20 11a8 8 0 1 1-2.3-5.7"/><path d="M20 4v5h-5"/>"#),
    ("moon", r#"<path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5z"/>"#),
    ("logout", r#"<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"/>"#),
    ("terminal", r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 9l3 3-3 3M13 15h4"/>"#),
    ("globe", r#"<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.5 2.7 3.8 5.7 3.8 9s-1.3 6.3-3.8 9c-2.5-2.7-3.8-5.7-3.8-9S9.5 5.7 12 3z"/>"#),
    ("folder", r#"<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>"#),
    ("home", r#"<path d="M3 11l9-7 9 7"/><path d="M5 10v10h5v-6h4v6h5V10"/>"#),
    ("mail", r#"<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 7l9 6 9-6"/>"#),
    ("chat", r#"<path d="M4 5h16v11H9l-5 4z"/><path d="M8 9.5h8M8 12.5h5"/>"#),
    ("code", r#"<path d="M8 7l-5 5 5 5M16 7l5 5-5 5M14 4l-4 16"/>"#),
    ("calc", r#"<rect x="5" y="3" width="14" height="18" rx="2"/><rect x="8" y="6" width="8" height="3.5" rx=".5"/><path d="M8.5 13h.01M12 13h.01M15.5 13h.01M8.5 17h.01M12 17h.01M15.5 17h.01"/>"#),
    ("gamepad", r#"<path d="M7 7h10a5 5 0 0 1 4.9 6l-.6 3a2.5 2.5 0 0 1-4.3 1.2L15 15H9l-2 2.2A2.5 2.5 0 0 1 2.7 16l-.6-3A5 5 0 0 1 7 7z"/><path d="M8 10v3M6.5 11.5h3M15.5 11h.01M17.5 12.5h.01"/>"#),
    ("mouse", r#"<rect x="6" y="3" width="12" height="18" rx="6"/><path d="M12 7v4"/>"#),
    ("cpu", r#"<rect x="6" y="6" width="12" height="12" rx="1.5"/><rect x="9.5" y="9.5" width="5" height="5"/><path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4"/>"#),
    ("clock", r#"<circle cx="12" cy="13" r="8"/><path d="M12 9v4l2.5 2.5M10 2h4"/>"#),
    ("copy", r#"<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>"#),
    ("paste", r#"<rect x="5" y="4" width="14" height="17" rx="2"/><rect x="9" y="2" width="6" height="4" rx="1"/><path d="M9 12h6M9 16h4"/>"#),
    ("cut", r#"<circle cx="6" cy="18" r="3"/><circle cx="18" cy="18" r="3"/><path d="M8.1 15.9L19 4M15.9 15.9L5 4"/>"#),
    ("undo", r#"<path d="M9 14L4 9l5-5"/><path d="M4 9h10a6 6 0 0 1 0 12h-3"/>"#),
    ("redo", r#"<path d="M15 14l5-5-5-5"/><path d="M20 9H10a6 6 0 0 0 0 12h3"/>"#),
    ("search", r#"<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>"#),
    ("star", r#"<path d="M12 3l2.8 5.7 6.2.9-4.5 4.4 1.1 6.2L12 17.3 6.4 20.2l1.1-6.2L3 9.6l6.2-.9z"/>"#),
    ("heart", r#"<path d="M12 20s-7.5-4.6-9.2-9.2A4.8 4.8 0 0 1 12 6.6a4.8 4.8 0 0 1 9.2 4.2C19.5 15.4 12 20 12 20z"/>"#),
    ("bell", r#"<path d="M6 16v-5a6 6 0 0 1 12 0v5l2 2H4z"/><path d="M10 20a2 2 0 0 0 4 0"/>"#),
    ("trash", r#"<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>"#),
    ("sun", r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>"#),
    ("wifi", r#"<path d="M2 9a15 15 0 0 1 20 0M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0"/><path d="M12 19.5h.01"/>"#),
    ("bluetooth", r#"<path d="M7 7l10 10-5 4V3l5 4L7 17"/>"#),
    ("cloud", r#"<path d="M7 18a4.5 4.5 0 0 1-.5-9A6 6 0 0 1 18 9.5a4 4 0 0 1-.5 8.5z"/>"#),
    ("coffee", r#"<path d="M4 9h13v6a5 5 0 0 1-5 5H9a5 5 0 0 1-5-5z"/><path d="M17 11h1.5a2.5 2.5 0 0 1 0 5H17M8 3v3M12 3v3"/>"#),
    ("fire", r#"<path d="M12 21c-4 0-7-2.8-7-6.5 0-3 2-5 3.5-6.5.3 1.6 1.2 2.8 2.5 3.3C11 8 12 5 14.5 3c.4 3 4.5 5.5 4.5 11.5 0 3.7-3 6.5-7 6.5z"/>"#),
    ("smile", r#"<circle cx="12" cy="12" r="9"/><path d="M8.5 14.5a4.5 4.5 0 0 0 7 0M9 9.5h.01M15 9.5h.01"/>"#),
    ("eye", r#"<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>"#),
    ("arrow-up", r#"<path d="M12 20V4M5 11l7-7 7 7"/>"#),
    ("arrow-down", r#"<path d="M12 4v16M5 13l7 7 7-7"/>"#),
    ("arrow-left", r#"<path d="M20 12H4M11 5l-7 7 7 7"/>"#),
    ("arrow-right", r#"<path d="M4 12h16M13 5l7 7-7 7"/>"#),
    ("zoom-in", r#"<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4M8 11h6M11 8v6"/>"#),
    ("zoom-out", r#"<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4M8 11h6"/>"#),
    ("timer", r#"<circle cx="12" cy="13" r="8"/><path d="M12 13V9M9 2h6M19 6l-1.5 1.5"/>"#),
    ("target", r#"<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="5"/><circle cx="12" cy="12" r="1"/>"#),
    ("flag", r#"<path d="M5 21V4M5 4h11l-2 4 2 4H5"/>"#),
    ("pin", r#"<path d="M12 21s-6-5.5-6-11a6 6 0 0 1 12 0c0 5.5-6 11-6 11z"/><circle cx="12" cy="10" r="2.2"/>"#),
    ("battery", r#"<rect x="2" y="7" width="17" height="10" rx="2"/><path d="M22 10.5v3M5.5 10.5v3M9 10.5v3"/>"#),
];

pub const LIBRARY: &[(&str, &str)] = &[
    ("playpause", "Lecture/Pause"), ("play", "Lecture"), ("pause", "Pause"), ("stop", "Stop"), ("next", "Suivant"), ("prev", "Précédent"),
    ("vol-up", "Volume +"), ("vol-down", "Volume −"), ("mute", "Muet"), ("mic", "Micro"), ("mic-off", "Micro coupé"), ("headphones", "Casque"),
    ("music", "Musique"), ("record", "Enregistrer"), ("stream", "Live"), ("video", "Caméra"), ("video-off", "Caméra coupée"), ("camera", "Photo"),
    ("screenshot", "Capture"), ("lock", "Verrouiller"), ("power", "Éteindre"), ("reboot", "Redémarrer"), ("moon", "Veille"), ("logout", "Déconnexion"),
    ("terminal", "Terminal"), ("globe", "Navigateur"), ("folder", "Dossier"), ("home", "Accueil"), ("mail", "Mail"), ("chat", "Discussion"),
    ("code", "Code"), ("calc", "Calculatrice"), ("gamepad", "Jeu"), ("mouse", "Souris"), ("keyboard", "Clavier"), ("cpu", "Système"),
    ("settings", "Réglages"), ("clock", "Horloge"), ("timer", "Minuteur"), ("copy", "Copier"), ("paste", "Coller"), ("cut", "Couper"),
    ("undo", "Annuler"), ("redo", "Rétablir"), ("search", "Rechercher"), ("zoom-in", "Zoom +"), ("zoom-out", "Zoom −"), ("save", "Enregistrer fichier"),
    ("star", "Favori"), ("heart", "Cœur"), ("bell", "Notification"), ("trash", "Corbeille"), ("sun", "Luminosité"), ("bulb", "Lumière"),
    ("wifi", "Wi-Fi"), ("bluetooth", "Bluetooth"), ("cloud", "Cloud"), ("download", "Télécharger"), ("upload", "Envoyer"), ("link", "Lien"),
    ("coffee", "Pause café"), ("fire", "Feu"), ("smile", "Sourire"), ("eye", "Vue"), ("target", "Cible"), ("flag", "Drapeau"),
    ("pin", "Lieu"), ("battery", "Batterie"), ("bolt", "Macro"), ("layers", "Multi"), ("image", "Image"), ("desktop", "Écran"),
    ("arrow-up", "Haut"), ("arrow-down", "Bas"), ("arrow-left", "Gauche"), ("arrow-right", "Droite"),
];

pub const APP_ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256">
<defs><pattern id="g" width="16" height="16" patternUnits="userSpaceOnUse"><path d="M16 0H0V16" fill="none" stroke="#5FB4FF" stroke-opacity=".18" stroke-width="1"/></pattern></defs>
<rect x="8" y="8" width="240" height="240" rx="44" fill="#0A2540"/><rect x="8" y="8" width="240" height="240" rx="44" fill="url(#g)"/>
<rect x="8" y="8" width="240" height="240" rx="44" fill="none" stroke="#5FB4FF" stroke-width="6"/>
<path d="M40 186l58-90 34 50 22-30 62 70z" fill="#5FB4FF" fill-opacity=".18" stroke="#9AD4FF" stroke-width="10" stroke-linejoin="round"/>
<path d="M98 96l14 22 12-10" fill="none" stroke="#FFFFFF" stroke-width="10" stroke-linejoin="round" stroke-linecap="round"/>
</svg>"##;

pub fn svg(name: &str) -> Option<String> {
    let body = ICONS.iter().find(|(n, _)| *n == name)?.1;
    Some(format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"##))
}

pub fn rasterize_svg(svg: &str, w: u32, h: u32) -> Option<tiny_skia::Pixmap> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pm = tiny_skia::Pixmap::new(w, h)?;
    let s = tree.size();
    let scale = (w as f32 / s.width()).min(h as f32 / s.height());
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    Some(pm)
}

pub fn rasterize(name: &str, px: u32, color: [u8; 3]) -> Option<tiny_skia::Pixmap> {
    let hex = format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
    rasterize_svg(&svg(name)?.replace("#FFFFFF", &hex), px, px)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_icon_renders() {
        for (name, _) in super::ICONS {
            let pm = super::rasterize(name, 48, [255, 255, 255]).unwrap_or_else(|| panic!("{name}"));
            assert!(pm.data().chunks(4).any(|p| p[3] > 0), "{name} is empty");
        }
        for (name, _) in super::LIBRARY {
            assert!(super::svg(name).is_some(), "{name}");
        }
        assert!(super::rasterize_svg(super::APP_ICON, 64, 64).is_some());
    }
}
